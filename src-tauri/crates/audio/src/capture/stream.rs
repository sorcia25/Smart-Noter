//! Opens audio input streams per `CaptureMode` and pushes samples to a channel.
//!
//! - `System`: WASAPI loopback on the selected render endpoint.
//! - `Mic`: cpal default-host input device matched by name (the device id
//!   stored in our `AudioDevice.name` field).
//! - `Mix`: both of the above, each feeding a separate channel that the
//!   mixer thread consumes.
//!
//! The audio callback does ONE allocation (`buf.to_vec()`) and then
//! `try_send(...)` on a bounded channel. Drops are counted and surfaced via
//! `audio:error` event when they exceed the threshold (see meter thread).

use crate::capture::mixer::TARGET_SAMPLE_RATE;
use crate::capture::session::CaptureMode;
use crate::devices::{enumerate, AudioDeviceKind};
use crate::error::AudioError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Sender;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

/// Callback invoked when the loopback follows a default-render-endpoint switch,
/// receiving the friendly name of the new output device.
///
/// Kept as a plain boxed closure (NOT a `tauri::AppHandle`) so this audio crate
/// stays decoupled from the GUI runtime: `stream.rs` must not reference any
/// `tauri` type, otherwise its object file drags `tao`/`tauri-runtime-wry` (and
/// the `TaskDialogIndirect` import from a `comctl32` v6 the test binary can't
/// resolve without an app manifest) into the crate's test executable, which
/// then fails to start. The recorder supplies a closure that emits the Tauri
/// `audio:output-device-changed` event.
///
/// `Send + Sync` because the closure is moved into the WASAPI capture thread.
pub type DeviceChangeCallback = Box<dyn Fn(String) + Send + Sync>;

/// Sentinel device id: resolve the CURRENT default render endpoint at open time.
/// The Mix card means "record whatever the PC is playing" — pinning a concrete
/// endpoint breaks when the user switches output (e.g. speakers → headphones)
/// between page load and recording start.
pub const DEFAULT_RENDER_LOOPBACK: &str = "__default_render__";

pub struct StreamHandle {
    pub sample_rate: u32,
    pub channels: u16,
    /// Cumulative drop counter shared across all streams for this handle.
    ///
    /// **Unit asymmetry (resolved in v1.0.1, Fix F1):** stream callbacks
    /// (System/Mic mode) increment this by 1 per *dropped buffer* (~480 frames
    /// / ~10 ms each). The Mix path's mixer thread computes drops as *sample
    /// counts* (`Mixer::dropped_frames()`), so the recorder's mixer thread now
    /// converts samples → buffer-equivalents (`÷ 480`, remainder carried
    /// forward) before adding to this counter — the ≥ 100 overflow-toast
    /// threshold means the same ~1 s of loss in both modes.
    pub drops: Arc<AtomicU32>,
    /// Populated only in Mix mode: the actual sample rate of the mic input.
    ///
    /// The mixer's b-side resampler must be configured with this rate.
    /// `None` in System and Mic modes (no resampling needed).
    pub mic_sample_rate: Option<u32>,
    /// Native rate/channels of the Mix sources (None outside Mix mode).
    pub loop_sample_rate: Option<u32>,
    pub loop_channels: Option<u16>,
    pub mic_channels: Option<u16>,
    /// Keep handles alive so the OS doesn't drop the stream.
    pub(crate) _streams: Vec<Box<dyn KeepAlive>>,
}

/// Marker trait so we can put cpal::Stream and wasapi handles in the same Vec.
pub(crate) trait KeepAlive: Send {}

struct CpalStream(#[allow(dead_code)] cpal::Stream);
impl KeepAlive for CpalStream {}
// SAFETY: cpal::Stream is `!Send` by default; we erase via Box<dyn KeepAlive>.
// In practice we keep it on the thread that opened it; do NOT move handles across threads.
unsafe impl Send for CpalStream {}

/// Holds the WASAPI loopback background thread and the flag to request its shutdown.
///
/// We store the stop flag alongside the JoinHandle so that Drop can signal the
/// thread to exit and then join it cleanly. Simply dropping a JoinHandle does
/// NOT join the thread — it just detaches it — which would leave the WASAPI
/// stream running and potentially cause COM teardown issues on process exit.
pub(crate) struct WasapiStreamThread {
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) handle: Option<std::thread::JoinHandle<()>>,
}

impl KeepAlive for WasapiStreamThread {}

impl Drop for WasapiStreamThread {
    fn drop(&mut self) {
        // Signal the loopback thread to exit.
        self.stop.store(true, Ordering::Relaxed);
        // Join so we don't leak the thread or COM objects.
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Open one or two streams depending on the mode.
///
/// Returns a handle whose drop closes the streams.
///
/// `on_device_change` is only consulted for loopback (System/Mix) capture, where
/// it is threaded down to the WASAPI capture thread and invoked with the new
/// device's friendly name when following a default-render switch (see
/// `wasapi_loopback_loop`). Mic-only capture never has a loopback thread, so it
/// ignores the callback — pass `None` from non-Tauri contexts (e.g. tests).
pub fn open(
    mode: CaptureMode,
    device_id: &str,
    mic_device_id: Option<&str>,
    tx_a: Sender<Vec<f32>>,
    tx_b: Option<Sender<Vec<f32>>>,
    on_device_change: Option<DeviceChangeCallback>,
) -> Result<StreamHandle, AudioError> {
    match mode {
        CaptureMode::System => open_loopback(device_id, tx_a, on_device_change),
        CaptureMode::Mic => open_mic(device_id, tx_a),
        CaptureMode::Mix => {
            let tx_b = tx_b.ok_or_else(|| {
                AudioError::Other("Mix mode requires a second channel sender".into())
            })?;
            // device_id is the loopback id; the mic is the explicitly chosen input
            // device, falling back to the system default when none was chosen.
            // Share a single drops counter across both streams so Task 3.2's recorder
            // sees aggregate pipeline drops from the whole Mix pipeline.
            let shared_drops = Arc::new(AtomicU32::new(0));
            let loop_handle =
                open_loopback_with_drops(device_id, tx_a, shared_drops.clone(), on_device_change)?;
            let mic_handle = match mic_device_id {
                Some(mic_id) => {
                    let device = resolve_input_device(mic_id)?;
                    build_cpal_input_stream(device, tx_b, shared_drops.clone())?
                }
                None => open_mic_default_with_drops(tx_b, shared_drops.clone())?,
            };
            // Capture the Mix source metadata before the handles are consumed.
            let mic_sample_rate = mic_handle.sample_rate;
            let loop_sample_rate = loop_handle.sample_rate;
            let loop_channels = loop_handle.channels;
            let mic_channels = mic_handle.channels;
            // Combine streams' keepalive boxes
            let mut streams = loop_handle._streams;
            streams.extend(mic_handle._streams);
            Ok(StreamHandle {
                // The recorder's Mixer thread always outputs at TARGET_SAMPLE_RATE
                // (48 kHz) regardless of the loopback device's native rate.  The
                // writer must be configured for that output rate; using
                // loop_handle.sample_rate would tag 48 kHz audio with the wrong
                // rate, causing wrong-pitch/tempo playback on any device whose
                // native loopback rate ≠ 48 kHz.
                sample_rate: TARGET_SAMPLE_RATE,
                channels: 1, // mixed output is mono
                drops: shared_drops,
                mic_sample_rate: Some(mic_sample_rate),
                loop_sample_rate: Some(loop_sample_rate),
                loop_channels: Some(loop_channels),
                mic_channels: Some(mic_channels),
                _streams: streams,
            })
        }
    }
}

fn open_loopback(
    device_id: &str,
    tx: Sender<Vec<f32>>,
    on_device_change: Option<DeviceChangeCallback>,
) -> Result<StreamHandle, AudioError> {
    let drops = Arc::new(AtomicU32::new(0));
    open_loopback_with_drops(device_id, tx, drops, on_device_change)
}

/// Resolve a loopback `device_id` to a concrete `wasapi::Device` plus a stable id
/// string for that endpoint.
///
/// Two resolution modes, mirroring the caller's original inline logic:
///  - `DEFAULT_RENDER_LOOPBACK` sentinel: resolve the CURRENT default render
///    endpoint (`wasapi::get_default_device`). This is Mix's "record whatever the
///    PC is playing" semantics — never fall through to the by-id path, which would
///    look up a device literally named `__default_render__` and always fail.
///  - any other id: enumerate our devices, find the matching Loopback entry, then
///    match it back to a `wasapi::Device` by friendly name (the name we store in
///    `AudioDevice`).
///
/// The returned id string comes from `wasapi::Device::get_id()` (wraps
/// `IMMDevice::GetId` — the persistent endpoint id), so it is stable and
/// comparable for the same endpoint across calls. Task B2 uses it to detect when
/// the default render endpoint has switched and a re-open is required. If
/// `get_id()` fails we fall back to the device's friendly name so the caller
/// always gets *some* comparable id rather than an error (the id is advisory, not
/// load-bearing for capture).
fn resolve_render_device(device_id: &str) -> Result<(wasapi::Device, String), AudioError> {
    let device = if device_id == DEFAULT_RENDER_LOOPBACK {
        wasapi::get_default_device(&wasapi::Direction::Render).map_err(|e| match e {
            wasapi::WasapiError::Windows(inner) => AudioError::WasapiInit {
                hresult: inner.code().0,
            },
            other => AudioError::Other(format!("WASAPI get_default_device: {other}")),
        })?
    } else {
        let devices = enumerate()?;
        let target = devices
            .iter()
            .find(|d| d.id == device_id && d.kind == AudioDeviceKind::Loopback)
            .ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))?;

        // Resolve back to a wasapi::Device by enumerating render endpoints and
        // matching by friendly name (the name stored in our AudioDevice).
        use wasapi::{DeviceCollection, Direction};
        let coll = DeviceCollection::new(&Direction::Render).map_err(|e| match e {
            wasapi::WasapiError::Windows(inner) => AudioError::WasapiInit {
                hresult: inner.code().0,
            },
            other => AudioError::Other(format!("WASAPI: {other}")),
        })?;

        let count = coll.get_nbr_devices().unwrap_or(0);
        let mut wasapi_dev = None;
        for i in 0..count {
            if let Ok(d) = coll.get_device_at_index(i) {
                if d.get_friendlyname().ok().as_deref() == Some(target.name.as_str()) {
                    wasapi_dev = Some(d);
                    break;
                }
            }
        }
        wasapi_dev.ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))?
    };

    // Prefer the persistent endpoint id (IMMDevice::GetId); fall back to the
    // friendly name if GetId fails. Either is stable/comparable for the same
    // endpoint, which is all B2's change detection needs.
    let open_id = device
        .get_id()
        .or_else(|_| device.get_friendlyname())
        .unwrap_or_default();

    Ok((device, open_id))
}

/// True when following is on and the current default differs from the open one.
fn should_reopen(follow: bool, open_id: &str, current_id: &str) -> bool {
    follow && open_id != current_id
}

fn open_loopback_with_drops(
    device_id: &str,
    tx: Sender<Vec<f32>>,
    drops: Arc<AtomicU32>,
    on_device_change: Option<DeviceChangeCallback>,
) -> Result<StreamHandle, AudioError> {
    // We need (sample_rate, channels) up front to configure the returned
    // StreamHandle's writer. Resolve once here to read the format; the capture
    // thread re-resolves the device itself from `device_id` (so it can later
    // follow a default-device switch). Resolving twice at startup is a cheap,
    // acceptable cost and keeps the thread self-contained.
    let (device, sample_rate, channels) = if device_id == DEFAULT_RENDER_LOOPBACK {
        let (device, _open_id) = resolve_render_device(device_id)?;
        let (sample_rate, channels) = crate::devices::render_format(&device);
        (device, sample_rate, channels)
    } else {
        // For the by-id path the rate/channels are the values we enumerated and
        // stored on the AudioDevice; look them up alongside resolving the device.
        let devices = enumerate()?;
        let target = devices
            .iter()
            .find(|d| d.id == device_id && d.kind == AudioDeviceKind::Loopback)
            .ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))?;
        let (sample_rate, channels) = (target.sample_rate, target.channels);
        let (device, _open_id) = resolve_render_device(device_id)?;
        (device, sample_rate, channels)
    };
    // The resolved `device` is only used to read the format here; the thread
    // re-resolves from `device_id`. Drop it explicitly to make that intent clear.
    drop(device);

    let drops_clone = drops.clone();

    let stop = Arc::new(AtomicBool::new(false));
    let handle = spawn_wasapi_loopback_thread(
        device_id.to_string(),
        sample_rate,
        channels,
        tx,
        drops_clone,
        stop.clone(),
        on_device_change,
    )?;

    Ok(StreamHandle {
        sample_rate,
        channels,
        drops,
        mic_sample_rate: None,
        loop_sample_rate: None,
        loop_channels: None,
        mic_channels: None,
        _streams: vec![Box::new(WasapiStreamThread {
            stop,
            handle: Some(handle),
        })],
    })
}

/// Resolve an input device by our enumerated id (matched back to cpal by name).
fn resolve_input_device(device_id: &str) -> Result<cpal::Device, AudioError> {
    let host = cpal::default_host();
    let devices = enumerate()?;
    let target = devices
        .iter()
        .find(|d| d.id == device_id && d.kind == AudioDeviceKind::Input)
        .ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))?;

    host.input_devices()
        .map_err(|e| AudioError::Other(format!("cpal input_devices: {e}")))?
        .find(|d| d.name().ok().as_deref() == Some(target.name.as_str()))
        .ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))
}

fn open_mic(device_id: &str, tx: Sender<Vec<f32>>) -> Result<StreamHandle, AudioError> {
    let device = resolve_input_device(device_id)?;
    let drops = Arc::new(AtomicU32::new(0));
    build_cpal_input_stream(device, tx, drops)
}

fn open_mic_default_with_drops(
    tx: Sender<Vec<f32>>,
    drops: Arc<AtomicU32>,
) -> Result<StreamHandle, AudioError> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| AudioError::DeviceNotFound("system default input".into()))?;

    build_cpal_input_stream(device, tx, drops)
}

/// Shared implementation for `open_mic` and `open_mic_default`.
///
/// [C1] Dispatches on `cpal::SampleFormat` to avoid a panic in the audio
/// callback. `build_input_stream::<T>` panics if the driver's actual format
/// doesn't match `T`; we convert all non-f32 formats to f32 inline here.
fn build_cpal_input_stream(
    device: cpal::Device,
    tx: Sender<Vec<f32>>,
    drops: Arc<AtomicU32>,
) -> Result<StreamHandle, AudioError> {
    let config = device
        .default_input_config()
        .map_err(|e| AudioError::FormatUnsupported(e.to_string()))?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    let sample_format = config.sample_format();
    let stream_cfg: cpal::StreamConfig = config.into();

    let err_fn = |err: cpal::StreamError| {
        tracing::error!(?err, "cpal input stream error");
    };

    // Each arm of the match needs its own clone of tx and drops because each
    // closure is a separate move closure.
    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            let tx = tx.clone();
            let drops_clone = drops.clone();
            device.build_input_stream::<f32, _, _>(
                &stream_cfg,
                move |data: &[f32], _| {
                    if tx.try_send(data.to_vec()).is_err() {
                        drops_clone.fetch_add(1, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I8 => {
            let tx = tx.clone();
            let drops_clone = drops.clone();
            device.build_input_stream::<i8, _, _>(
                &stream_cfg,
                move |data: &[i8], _| {
                    let f: Vec<f32> = data.iter().map(|s| *s as f32 / i8::MAX as f32).collect();
                    if tx.try_send(f).is_err() {
                        drops_clone.fetch_add(1, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let tx = tx.clone();
            let drops_clone = drops.clone();
            device.build_input_stream::<i16, _, _>(
                &stream_cfg,
                move |data: &[i16], _| {
                    let f: Vec<f32> = data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                    if tx.try_send(f).is_err() {
                        drops_clone.fetch_add(1, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I32 => {
            let tx = tx.clone();
            let drops_clone = drops.clone();
            device.build_input_stream::<i32, _, _>(
                &stream_cfg,
                move |data: &[i32], _| {
                    let f: Vec<f32> = data.iter().map(|s| *s as f32 / i32::MAX as f32).collect();
                    if tx.try_send(f).is_err() {
                        drops_clone.fetch_add(1, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I64 => {
            let tx = tx.clone();
            let drops_clone = drops.clone();
            device.build_input_stream::<i64, _, _>(
                &stream_cfg,
                move |data: &[i64], _| {
                    let f: Vec<f32> = data.iter().map(|s| *s as f32 / i64::MAX as f32).collect();
                    if tx.try_send(f).is_err() {
                        drops_clone.fetch_add(1, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U8 => {
            let tx = tx.clone();
            let drops_clone = drops.clone();
            device.build_input_stream::<u8, _, _>(
                &stream_cfg,
                move |data: &[u8], _| {
                    // Unipolar → bipolar: subtract midpoint, divide by half-range.
                    let f: Vec<f32> = data
                        .iter()
                        .map(|s| (*s as f32 - u8::MAX as f32 / 2.0) / (u8::MAX as f32 / 2.0))
                        .collect();
                    if tx.try_send(f).is_err() {
                        drops_clone.fetch_add(1, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let tx = tx.clone();
            let drops_clone = drops.clone();
            device.build_input_stream::<u16, _, _>(
                &stream_cfg,
                move |data: &[u16], _| {
                    // Unipolar → bipolar: subtract midpoint, divide by half-range.
                    let f: Vec<f32> = data
                        .iter()
                        .map(|s| (*s as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0))
                        .collect();
                    if tx.try_send(f).is_err() {
                        drops_clone.fetch_add(1, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U32 => {
            let tx = tx.clone();
            let drops_clone = drops.clone();
            device.build_input_stream::<u32, _, _>(
                &stream_cfg,
                move |data: &[u32], _| {
                    // Unipolar → bipolar: subtract midpoint, divide by half-range.
                    let f: Vec<f32> = data
                        .iter()
                        .map(|s| (*s as f32 - u32::MAX as f32 / 2.0) / (u32::MAX as f32 / 2.0))
                        .collect();
                    if tx.try_send(f).is_err() {
                        drops_clone.fetch_add(1, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U64 => {
            let tx = tx.clone();
            let drops_clone = drops.clone();
            device.build_input_stream::<u64, _, _>(
                &stream_cfg,
                move |data: &[u64], _| {
                    // Unipolar → bipolar: subtract midpoint, divide by half-range.
                    let f: Vec<f32> = data
                        .iter()
                        .map(|s| (*s as f32 - u64::MAX as f32 / 2.0) / (u64::MAX as f32 / 2.0))
                        .collect();
                    if tx.try_send(f).is_err() {
                        drops_clone.fetch_add(1, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::F64 => {
            let tx = tx.clone();
            let drops_clone = drops.clone();
            device.build_input_stream::<f64, _, _>(
                &stream_cfg,
                move |data: &[f64], _| {
                    let f: Vec<f32> = data.iter().map(|s| *s as f32).collect();
                    if tx.try_send(f).is_err() {
                        drops_clone.fetch_add(1, Ordering::Relaxed);
                    }
                },
                err_fn,
                None,
            )
        }
        // `SampleFormat` is `#[non_exhaustive]`; reject any future variants added
        // by cpal rather than silently misinterpreting the bytes.
        other => {
            return Err(AudioError::FormatUnsupported(format!(
                "cpal sample format: {other:?}"
            )))
        }
    }
    .map_err(|e| AudioError::Other(format!("cpal build_input_stream: {e}")))?;

    stream
        .play()
        .map_err(|e| AudioError::Other(format!("cpal play: {e}")))?;

    Ok(StreamHandle {
        sample_rate,
        channels,
        drops,
        mic_sample_rate: None,
        loop_sample_rate: None,
        loop_channels: None,
        mic_channels: None,
        _streams: vec![Box::new(CpalStream(stream))],
    })
}

/// Spawn the WASAPI loopback capture thread.
///
/// The thread initialises an IAudioClient in loopback mode (Render device +
/// Direction::Capture), requests f32 samples natively (no conversion needed),
/// and pushes `Vec<f32>` chunks to `tx` in an event-driven loop.
///
/// Shutdown: the caller sets `stop` to `true`; the event-wait timeout wakes
/// the thread at most every 100 ms so it can check the flag and exit cleanly.
/// The `WasapiStreamThread` Drop impl stores the same Arc and joins the thread.
fn spawn_wasapi_loopback_thread(
    device_id: String,
    sample_rate: u32,
    channels: u16,
    tx: Sender<Vec<f32>>,
    drops: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
    on_device_change: Option<DeviceChangeCallback>,
) -> Result<std::thread::JoinHandle<()>, AudioError> {
    let handle = std::thread::Builder::new()
        .name("wasapi-loopback".into())
        .spawn(move || {
            // The thread resolves the device itself from `device_id` (inside the
            // MTA it initialises below), so nothing non-Send crosses this boundary.
            if let Err(e) = wasapi_loopback_loop(
                device_id,
                sample_rate,
                channels,
                &tx,
                &drops,
                &stop,
                on_device_change,
            ) {
                tracing::error!("WASAPI loopback thread exited with error: {e}");
            }
        })
        .map_err(|e| AudioError::Other(format!("spawn loopback thread: {e}")))?;
    Ok(handle)
}

/// Inner loopback capture loop.  Called from the background thread.
///
/// Steps (mirroring `wasapi-rs` `examples/loopback.rs`):
///  1. Get IAudioClient from the Render endpoint.
///  2. Request f32/32-bit shared-mode capture (loopback flag is set automatically
///     by wasapi-rs when Direction::Capture is passed to a Render-direction device).
///  3. Set an event handle so the wait is CPU-friendly.
///  4. Start the stream.
///  5. Loop: wait on event → drain all available packets → push f32 Vec to channel.
///  6. On exit: stop the stream.
///
/// The "Render device + Capture direction" combination is what wasapi-rs calls
/// WASAPI loopback: `initialize_client` detects the mismatch and sets
/// `AUDCLNT_STREAMFLAGS_LOOPBACK` automatically (see wasapi api.rs line ~835).
///
/// The outer `'reopen` loop re-resolves the device and rebuilds the client
/// whenever the default render endpoint changes (Mix mode only — see `follow`
/// below); every resource is scoped to one iteration, so `continue 'reopen`
/// only runs after the current `audio_client` (and its capture client / event
/// handle) has been stopped and dropped.
fn wasapi_loopback_loop(
    device_id: String,
    sample_rate: u32,
    channels: u16,
    tx: &Sender<Vec<f32>>,
    drops: &Arc<AtomicU32>,
    stop: &Arc<AtomicBool>,
    on_device_change: Option<DeviceChangeCallback>,
) -> Result<(), AudioError> {
    use wasapi::{Direction, SampleType, ShareMode, WaveFormat};

    // [I3] Initialise COM for the MTA. `initialize_mta` returns a raw HRESULT:
    //   S_OK (0)    — success, COM now initialised on this thread.
    //   S_FALSE (1) — already initialised as MTA on this thread; benign.
    //   failure (<0) — real error; subsequent COM calls will produce confusing
    //                  HRESULTs, so log and exit the thread early.
    // Both S_OK and S_FALSE satisfy `is_ok()` (hr.0 >= 0).
    {
        let hr = wasapi::initialize_mta();
        if hr.is_err() {
            tracing::error!(
                hresult = hr.0,
                "initialize_mta failed; loopback thread cannot continue"
            );
            return Ok(()); // exit cleanly; recorder will observe silent channel
        }
        // S_OK or S_FALSE — both acceptable
    }

    // Whether this thread should follow default-render-endpoint changes. Only the
    // sentinel id has "record whatever the PC is playing" semantics; a concrete
    // device id stays pinned. Consumed by the default-device poll below.
    let follow = device_id == DEFAULT_RENDER_LOOPBACK;

    // Outer re-open loop: re-resolves the device and rebuilds the client when the
    // default render endpoint changes (poll below). Every resource below is
    // scoped to one iteration, so the old client fully drops (stop_stream + COM
    // release) before the next open.
    'reopen: loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Re-resolve the device inside the thread (in the MTA). Re-entered via
        // `continue 'reopen` to pick up the new default endpoint. `open_id` is
        // the endpoint id this iteration opened against, used by the poll below
        // to detect a subsequent default-device change.
        let (device, open_id) = match resolve_render_device(&device_id) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(?e, "loopback resolve failed");
                return Ok(()); // exit cleanly; recorder observes silent channel
            }
        };

        let mut audio_client = device.get_iaudioclient().map_err(|e| match e {
            wasapi::WasapiError::Windows(inner) => AudioError::WasapiInit {
                hresult: inner.code().0,
            },
            other => AudioError::Other(format!("WASAPI get_iaudioclient: {other}")),
        })?;

        // Request 32-bit IEEE-float samples natively — no i16→f32 conversion needed.
        // We always request the ORIGINAL sample_rate/channels (the fn params) so
        // downstream (writer/mixer) never sees a format change across a re-open.
        // On re-open (poll-triggered `continue 'reopen`) the new endpoint's native
        // mix format may differ (e.g. speakers @ 48 kHz → headphones @ 44.1 kHz);
        // `convert=true` below asks WASAPI to resample/reformat to this requested
        // format, so this assumption should hold, but it is only truly verified by
        // a manual smoke with two real output devices at different native rates.
        let desired_format = WaveFormat::new(
            32,
            32,
            &SampleType::Float,
            sample_rate as usize,
            channels as usize,
            None,
        );

        let (_, min_time) = audio_client.get_periods().map_err(|e| match e {
            wasapi::WasapiError::Windows(inner) => AudioError::WasapiInit {
                hresult: inner.code().0,
            },
            other => AudioError::Other(format!("WASAPI get_periods: {other}")),
        })?;

        // initialize_client: device direction is Render, requested direction is Capture.
        // wasapi-rs detects this and adds AUDCLNT_STREAMFLAGS_LOOPBACK automatically.
        // convert=true enables format auto-conversion so the driver accepts our f32 format
        // even if its native mix format differs.
        audio_client
            .initialize_client(
                &desired_format,
                min_time,
                &Direction::Capture,
                &ShareMode::Shared,
                true,
            )
            .map_err(|e| match e {
                wasapi::WasapiError::Windows(inner) => AudioError::WasapiInit {
                    hresult: inner.code().0,
                },
                other => AudioError::Other(format!("WASAPI initialize_client: {other}")),
            })?;

        // Event handle: the OS signals this when a new packet is ready, so we don't
        // have to spin-poll — dramatically reduces CPU usage at idle.
        let h_event = audio_client.set_get_eventhandle().map_err(|e| match e {
            wasapi::WasapiError::Windows(inner) => AudioError::WasapiInit {
                hresult: inner.code().0,
            },
            other => AudioError::Other(format!("WASAPI set_get_eventhandle: {other}")),
        })?;

        let capture_client = audio_client.get_audiocaptureclient().map_err(|e| match e {
            wasapi::WasapiError::Windows(inner) => AudioError::WasapiInit {
                hresult: inner.code().0,
            },
            other => AudioError::Other(format!("WASAPI get_audiocaptureclient: {other}")),
        })?;

        let blockalign = desired_format.get_blockalign() as usize;

        audio_client.start_stream().map_err(|e| match e {
            wasapi::WasapiError::Windows(inner) => AudioError::WasapiInit {
                hresult: inner.code().0,
            },
            other => AudioError::Other(format!("WASAPI start_stream: {other}")),
        })?;

        tracing::info!(sample_rate, channels, "WASAPI loopback stream started");

        // One-shot scratch buffer: sized for one packet worth of bytes.
        // We'll grow it as needed below.
        let mut raw_buf: Vec<u8> = Vec::new();

        // Throttles the default-device poll below to ~once per second, so we
        // don't call `get_default_device` on every event wakeup.
        let mut last_poll = Instant::now();

        // Inner capture loop for this open. On stop we stop the stream and return.
        loop {
            if stop.load(Ordering::Relaxed) {
                if let Err(e) = audio_client.stop_stream() {
                    tracing::warn!("WASAPI stop_stream error: {e}");
                }
                tracing::info!("WASAPI loopback stream stopped");
                return Ok(());
            }

            // Poll the default render endpoint (~1 s) and re-open if it changed.
            // Only active when following the sentinel (Mix "whatever is playing");
            // a pinned device id never triggers this. Stopping the stream here
            // (before `continue 'reopen`) ensures the client is fully torn down —
            // `audio_client`/`capture_client`/`h_event` all drop at the top of this
            // block's scope — before the next iteration resolves + opens the new
            // endpoint. The mixer's silence-fill covers the sub-second gap.
            if follow && last_poll.elapsed() >= Duration::from_secs(1) {
                last_poll = Instant::now();
                if let Ok(cur) = wasapi::get_default_device(&wasapi::Direction::Render) {
                    let cur_id = cur.get_id().unwrap_or_default();
                    if should_reopen(follow, &open_id, &cur_id) {
                        tracing::info!(old = %open_id, new = %cur_id, "default render changed; reopening loopback");
                        let _ = audio_client.stop_stream();
                        if let Some(cb) = &on_device_change {
                            cb(cur.get_friendlyname().unwrap_or_default());
                        }
                        continue 'reopen;
                    }
                }
            }

            // Wait up to 100 ms. On timeout (no audio playing / device went silent)
            // we loop back and check the stop flag, then wait again.
            if h_event.wait_for_event(100).is_err() {
                // Timeout is not an error — just no data yet.
                continue;
            }

            // Drain all packets that became available in this event cycle.
            loop {
                // [I2] Check stop flag inside the inner drain loop too, so shutdown
                // latency is bounded at ≤ event-timeout (~100 ms) even under load.
                if stop.load(Ordering::Relaxed) {
                    break;
                }

                let frames = match capture_client.get_next_nbr_frames() {
                    Ok(Some(0)) | Ok(None) => break,
                    Ok(Some(n)) => n as usize,
                    Err(e) => {
                        tracing::warn!("WASAPI get_next_nbr_frames error: {e}");
                        break;
                    }
                };

                let needed_bytes = frames * blockalign;
                if raw_buf.len() < needed_bytes {
                    raw_buf.resize(needed_bytes, 0u8);
                }

                match capture_client.read_from_device(&mut raw_buf[..needed_bytes]) {
                    Ok((frames_read, _flags)) if frames_read > 0 => {
                        let bytes_read = frames_read as usize * blockalign;
                        // Decode little-endian IEEE-754 single-precision floats packet-by-packet.
                        // blockalign = channels * 4 bytes/sample, so bytes_read is always
                        // a multiple of 4.
                        let raw_slice = &raw_buf[..bytes_read];
                        // [M3] Skip zero-init pass: collect directly via iterator.
                        let samples: Vec<f32> = raw_slice
                            .chunks_exact(4)
                            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                            .collect();
                        if tx.try_send(samples).is_err() {
                            drops.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("WASAPI read_from_device error: {e}");
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_handle_struct_is_constructable() {
        let (_tx, _rx) = crossbeam_channel::bounded::<Vec<f32>>(1);
        let drops = Arc::new(AtomicU32::new(0));
        let h = StreamHandle {
            sample_rate: 48_000,
            channels: 2,
            drops: drops.clone(),
            mic_sample_rate: None,
            loop_sample_rate: None,
            loop_channels: None,
            mic_channels: None,
            _streams: vec![],
        };
        assert_eq!(h.sample_rate, 48_000);
        assert_eq!(h.channels, 2);
    }

    /// [M2] Calling `open` in Mix mode without a second tx must return
    /// `AudioError::Other` with a descriptive message. No audio hardware needed.
    #[test]
    fn open_mix_without_second_tx_returns_other() {
        let (tx, _rx) = crossbeam_channel::bounded::<Vec<f32>>(1);
        let result = open(CaptureMode::Mix, "any-id", None, tx, None, None);
        match result {
            Err(AudioError::Other(msg)) => {
                assert!(
                    msg.contains("Mix mode requires a second channel sender"),
                    "unexpected message: {msg}"
                );
            }
            Err(other) => panic!("expected AudioError::Other, got {other:?}"),
            Ok(_) => panic!("expected Err, got Ok"),
        }
    }

    /// [M2] Requesting System capture with an id that cannot be found must
    /// return `DeviceNotFound`. On a machine without a WASAPI subsystem the
    /// call may fail earlier with `WasapiInit`; that is also acceptable.
    ///
    /// Gated to Windows because WASAPI enumeration is Windows-only and would
    /// link-fail on other platforms.
    #[cfg(target_os = "windows")]
    #[test]
    fn open_system_with_unknown_device_returns_device_not_found() {
        let (tx, _rx) = crossbeam_channel::bounded::<Vec<f32>>(1);
        let result = open(
            CaptureMode::System,
            "id-that-does-not-exist",
            None,
            tx,
            None,
            None,
        );
        match result {
            Err(AudioError::DeviceNotFound(id)) => {
                assert_eq!(id, "id-that-does-not-exist");
            }
            // Acceptable: WASAPI subsystem absent on the test runner.
            Err(AudioError::WasapiInit { .. }) | Err(AudioError::Other(_)) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("expected Err, got Ok"),
        }
    }

    /// v1.0.1 F3: the `DEFAULT_RENDER_LOOPBACK` sentinel must resolve the
    /// CURRENT default render endpoint directly (`wasapi::get_default_device`)
    /// and must NEVER fall through to the by-id enumerate/match path — that
    /// path would look up a device literally named `__default_render__`, which
    /// never exists, and incorrectly return `DeviceNotFound`.
    ///
    /// On a machine with a render device this succeeds. On a headless CI
    /// runner without audio hardware `get_default_device` itself may fail
    /// (WasapiInit) or a later WASAPI call may fail (Other) — both acceptable,
    /// mirroring `open_system_with_unknown_device_returns_device_not_found`
    /// above. The one outcome that must NEVER happen is `DeviceNotFound` for
    /// the sentinel id, since that would mean the sentinel leaked into the
    /// by-id lookup.
    #[cfg(target_os = "windows")]
    #[test]
    fn default_render_sentinel_does_not_hit_device_not_found() {
        let (tx, _rx) = crossbeam_channel::bounded::<Vec<f32>>(1);
        let result = open(
            CaptureMode::System,
            DEFAULT_RENDER_LOOPBACK,
            None,
            tx,
            None,
            None,
        );
        match result {
            Ok(_) => {}
            // Acceptable: WASAPI subsystem or default render endpoint absent
            // on the test runner.
            Err(AudioError::WasapiInit { .. }) | Err(AudioError::Other(_)) => {}
            Err(AudioError::DeviceNotFound(id)) => panic!(
                "sentinel must resolve via get_default_device, not the by-id \
                 enumerate/match path; got DeviceNotFound({id:?})"
            ),
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn reopens_only_when_following_and_id_changed() {
        assert!(should_reopen(true, "spk", "hp"));
        assert!(!should_reopen(true, "spk", "spk"));
        assert!(!should_reopen(false, "spk", "hp")); // pinned device never follows
    }
}
