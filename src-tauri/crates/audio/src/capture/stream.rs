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

use crate::capture::session::CaptureMode;
use crate::devices::{enumerate, AudioDeviceKind};
use crate::error::AudioError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Sender;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};

pub struct StreamHandle {
    pub sample_rate: u32,
    pub channels: u16,
    pub drops: Arc<AtomicU32>,
    /// Keep handles alive so the OS doesn't drop the stream.
    _streams: Vec<Box<dyn KeepAlive>>,
}

/// Marker trait so we can put cpal::Stream and wasapi handles in the same Vec.
trait KeepAlive: Send {}

struct CpalStream(#[allow(dead_code)] cpal::Stream);
impl KeepAlive for CpalStream {}
// SAFETY: cpal::Stream is `!Send` by default; we erase via Box<dyn KeepAlive>.
// In practice we keep it on the thread that opened it; do NOT move handles across threads.
unsafe impl Send for CpalStream {}

/// Newtype wrapper that makes `wasapi::Device` sendable across threads.
///
/// SAFETY: `wasapi::Device` wraps an `IMMDevice` COM pointer. COM objects in the
/// MTA (Multi-Threaded Apartment, which we use for the loopback thread) can be
/// safely accessed from any thread in that MTA. The loopback thread calls
/// `initialize_mta()` before touching the device, ensuring it is in the MTA.
/// We never share the raw pointer between threads simultaneously.
struct SendDevice(wasapi::Device);
unsafe impl Send for SendDevice {}

/// Holds the WASAPI loopback background thread and the flag to request its shutdown.
///
/// We store the stop flag alongside the JoinHandle so that Drop can signal the
/// thread to exit and then join it cleanly. Simply dropping a JoinHandle does
/// NOT join the thread — it just detaches it — which would leave the WASAPI
/// stream running and potentially cause COM teardown issues on process exit.
struct WasapiStreamThread {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
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
pub fn open(
    mode: CaptureMode,
    device_id: &str,
    tx_a: Sender<Vec<f32>>,
    tx_b: Option<Sender<Vec<f32>>>,
) -> Result<StreamHandle, AudioError> {
    match mode {
        CaptureMode::System => open_loopback(device_id, tx_a),
        CaptureMode::Mic => open_mic(device_id, tx_a),
        CaptureMode::Mix => {
            let tx_b = tx_b.ok_or_else(|| {
                AudioError::Other("Mix mode requires a second channel sender".into())
            })?;
            // device_id is the loopback id; mic picks the system default for now
            let loop_handle = open_loopback(device_id, tx_a)?;
            let mic_handle = open_mic_default(tx_b)?;
            // Combine streams' keepalive boxes
            let mut streams = loop_handle._streams;
            streams.extend(mic_handle._streams);
            Ok(StreamHandle {
                sample_rate: loop_handle.sample_rate,
                channels: 1, // mixed output is mono
                drops: loop_handle.drops,
                _streams: streams,
            })
        }
    }
}

fn open_loopback(device_id: &str, tx: Sender<Vec<f32>>) -> Result<StreamHandle, AudioError> {
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
    let device = wasapi_dev.ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))?;

    let drops = Arc::new(AtomicU32::new(0));
    let drops_clone = drops.clone();
    let sample_rate = target.sample_rate;
    let channels = target.channels;

    let stop = Arc::new(AtomicBool::new(false));
    let handle = spawn_wasapi_loopback_thread(
        SendDevice(device),
        sample_rate,
        channels,
        tx,
        drops_clone,
        stop.clone(),
    )?;

    Ok(StreamHandle {
        sample_rate,
        channels,
        drops,
        _streams: vec![Box::new(WasapiStreamThread {
            stop,
            handle: Some(handle),
        })],
    })
}

fn open_mic(device_id: &str, tx: Sender<Vec<f32>>) -> Result<StreamHandle, AudioError> {
    let host = cpal::default_host();
    let devices = enumerate()?;
    let target = devices
        .iter()
        .find(|d| d.id == device_id && d.kind == AudioDeviceKind::Input)
        .ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))?;

    let device = host
        .input_devices()
        .map_err(|e| AudioError::Other(format!("cpal input_devices: {e}")))?
        .find(|d| d.name().ok().as_deref() == Some(target.name.as_str()))
        .ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))?;

    build_cpal_input_stream(device, tx)
}

fn open_mic_default(tx: Sender<Vec<f32>>) -> Result<StreamHandle, AudioError> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| AudioError::DeviceNotFound("system default input".into()))?;

    build_cpal_input_stream(device, tx)
}

/// Shared implementation for both `open_mic` and `open_mic_default`.
fn build_cpal_input_stream(
    device: cpal::Device,
    tx: Sender<Vec<f32>>,
) -> Result<StreamHandle, AudioError> {
    let config = device
        .default_input_config()
        .map_err(|e| AudioError::FormatUnsupported(e.to_string()))?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    let drops = Arc::new(AtomicU32::new(0));
    let drops_clone = drops.clone();

    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                if tx.try_send(data.to_vec()).is_err() {
                    drops_clone.fetch_add(1, Ordering::Relaxed);
                }
            },
            |err| {
                tracing::error!(?err, "cpal input stream error");
            },
            None,
        )
        .map_err(|e| AudioError::Other(format!("cpal build_input_stream: {e}")))?;
    stream
        .play()
        .map_err(|e| AudioError::Other(format!("cpal play: {e}")))?;

    Ok(StreamHandle {
        sample_rate,
        channels,
        drops,
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
    device: SendDevice,
    sample_rate: u32,
    channels: u16,
    tx: Sender<Vec<f32>>,
    drops: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, AudioError> {
    let handle = std::thread::Builder::new()
        .name("wasapi-loopback".into())
        .spawn(move || {
            // device is a SendDevice (unsafe impl Send); unwrap inside the thread.
            if let Err(e) = wasapi_loopback_loop(device, sample_rate, channels, &tx, &drops, &stop)
            {
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
fn wasapi_loopback_loop(
    device: SendDevice,
    sample_rate: u32,
    channels: u16,
    tx: &Sender<Vec<f32>>,
    drops: &Arc<AtomicU32>,
    stop: &Arc<AtomicBool>,
) -> Result<(), AudioError> {
    use wasapi::{Direction, SampleType, ShareMode, WaveFormat};

    // Initialise COM for the MTA so we can use the IMMDevice COM pointer from
    // any thread in this apartment. Returns S_FALSE if already initialised — not an error.
    let _ = wasapi::initialize_mta();

    let device = device.0;
    let mut audio_client = device.get_iaudioclient().map_err(|e| match e {
        wasapi::WasapiError::Windows(inner) => AudioError::WasapiInit {
            hresult: inner.code().0,
        },
        other => AudioError::Other(format!("WASAPI get_iaudioclient: {other}")),
    })?;

    // Request 32-bit IEEE-float samples natively — no i16→f32 conversion needed.
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

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        // Wait up to 100 ms. On timeout (no audio playing / device went silent)
        // we loop back and check the stop flag, then wait again.
        if h_event.wait_for_event(100).is_err() {
            // Timeout is not an error — just no data yet.
            continue;
        }

        // Drain all packets that became available in this event cycle.
        loop {
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
                    // The wire format is 32-bit IEEE-float; reinterpret bytes as f32.
                    // blockalign = channels * 4 bytes/sample, so bytes_read is always
                    // a multiple of 4.
                    let float_count = bytes_read / 4;
                    let mut samples = vec![0.0f32; float_count];
                    // SAFETY: raw_buf is aligned to u8; f32 alignment is 4. We
                    // checked bytes_read % 4 == 0 implicitly via blockalign math.
                    // copy_nonoverlapping from a &[u8] into &mut [f32] via bytes.
                    let raw_slice = &raw_buf[..bytes_read];
                    for (i, chunk) in raw_slice.chunks_exact(4).enumerate() {
                        samples[i] = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    }
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

    if let Err(e) = audio_client.stop_stream() {
        tracing::warn!("WASAPI stop_stream error: {e}");
    }
    tracing::info!("WASAPI loopback stream stopped");
    Ok(())
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
            _streams: vec![],
        };
        assert_eq!(h.sample_rate, 48_000);
        assert_eq!(h.channels, 2);
    }
}
