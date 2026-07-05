//! Microphone capture through the Windows-native AEC (Communications signal
//! processing mode). Opens an `IAudioClient` in `AudioCategory_Communications`
//! and sets the loopback render endpoint as the echo-cancellation reference via
//! `IAcousticEchoCancellationControl`. The OS delivers an already-echo-cancelled
//! mic stream and compensates the loopback/mic clock drift internally.
//!
//! The captured f32 samples are pushed to the same `Sender<Vec<f32>>` the cpal
//! mic path uses, so the mixer/writer downstream are agnostic to the source.

use crate::capture::session::CaptureMode;
use crate::capture::stream::{
    KeepAlive, StreamHandle, WasapiStreamThread, DEFAULT_RENDER_LOOPBACK,
};
use crate::error::AudioError;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    AudioCategory_Communications, AudioClientProperties, IAcousticEchoCancellationControl,
    IAudioCaptureClient, IAudioClient, IAudioClient2, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

/// Whether the microphone should be captured through the OS AEC (Communications
/// mode) instead of raw cpal. Only Mix mode has speaker echo to cancel.
pub(crate) fn use_comms_mic(mode: CaptureMode, aec_enabled: bool) -> bool {
    matches!(mode, CaptureMode::Mix) && aec_enabled
}

/// Whether the AEC reference should auto-follow the default render endpoint.
/// True for the "record whatever is playing" sentinel — we pass NULL to
/// `SetEchoCancellationRenderEndpoint` and let Windows track the default render
/// (so no manual re-set is needed when the output device changes). A pinned
/// loopback id resolves to that concrete endpoint instead.
pub(crate) fn aec_reference_is_auto(loopback_device_id: &str) -> bool {
    loopback_device_id == DEFAULT_RENDER_LOOPBACK
}

/// RAII guard that closes an owned event `HANDLE` (from `CreateEventW`) on drop,
/// so the event object is released on every exit path of `mic_comms_loop` —
/// normal return, a `?` early-return after creation, or a panic. Without this the
/// event leaks once per recording session.
struct EventHandle(HANDLE);

impl Drop for EventHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: handle came from CreateEventW; closed exactly once on drop.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// Resolve the capture endpoint id string for the chosen mic (or the default
/// communications capture endpoint when `mic_device_id` is None). Reuses the
/// `wasapi` crate's friendly-name matching (mirrors `stream::resolve_render_device`
/// but for Direction::Capture) and returns the persistent endpoint id
/// (`IMMDevice::GetId`), which `IMMDeviceEnumerator::GetDevice` re-resolves.
fn resolve_capture_endpoint_id(mic_device_id: Option<&str>) -> Result<String, AudioError> {
    use crate::devices::{enumerate, AudioDeviceKind};
    use wasapi::{DeviceCollection, Direction};

    let dev = match mic_device_id {
        None => wasapi::get_default_device(&Direction::Capture).map_err(|e| match e {
            wasapi::WasapiError::Windows(inner) => AudioError::WasapiInit {
                hresult: inner.code().0,
            },
            other => AudioError::Other(format!("WASAPI get_default_device(capture): {other}")),
        })?,
        Some(our_id) => {
            let devices = enumerate()?;
            let target = devices
                .iter()
                .find(|d| d.id == our_id && d.kind == AudioDeviceKind::Input)
                .ok_or_else(|| AudioError::DeviceNotFound(our_id.to_string()))?;
            let coll = DeviceCollection::new(&Direction::Capture).map_err(|e| match e {
                wasapi::WasapiError::Windows(inner) => AudioError::WasapiInit {
                    hresult: inner.code().0,
                },
                other => AudioError::Other(format!("WASAPI capture collection: {other}")),
            })?;
            let count = coll.get_nbr_devices().unwrap_or(0);
            let mut found = None;
            for i in 0..count {
                if let Ok(d) = coll.get_device_at_index(i) {
                    if d.get_friendlyname().ok().as_deref() == Some(target.name.as_str()) {
                        found = Some(d);
                        break;
                    }
                }
            }
            found.ok_or_else(|| AudioError::DeviceNotFound(our_id.to_string()))?
        }
    };
    dev.get_id()
        .map_err(|e| AudioError::Other(format!("capture get_id: {e}")))
}

/// Open the microphone through the OS AEC (Communications mode).
///
/// `reference` is the loopback render endpoint id to use as the echo reference,
/// or None to let Windows auto-follow the default render (the sentinel case).
/// Returns a `StreamHandle` whose `sample_rate`/`channels` are the comms mix
/// format (the mixer resamples to 48k). f32 samples are pushed to `tx`.
pub(crate) fn open_mic_comms(
    mic_device_id: Option<&str>,
    reference: Option<String>,
    tx: Sender<Vec<f32>>,
    drops: Arc<AtomicU32>,
) -> Result<StreamHandle, AudioError> {
    let endpoint_id = resolve_capture_endpoint_id(mic_device_id)?;

    // Read the mix format up-front (activate + GetMixFormat, then release) so the
    // StreamHandle reports the real rate/channels; the capture thread re-activates
    // its own client (mirrors stream.rs loopback: read format up-front, thread
    // re-resolves). The comms mix format is stable for the endpoint.
    let (sample_rate, channels) = read_comms_mix_format(&endpoint_id)?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = stop.clone();
    let drops_thread = drops.clone();
    let handle = std::thread::Builder::new()
        .name("wasapi-mic-aec".into())
        .spawn(move || {
            if let Err(e) =
                mic_comms_loop(&endpoint_id, reference, &tx, &drops_thread, &stop_thread)
            {
                tracing::error!("WASAPI comms-mic thread exited with error: {e}");
            }
        })
        .map_err(|e| AudioError::Other(format!("spawn comms-mic thread: {e}")))?;

    Ok(StreamHandle {
        sample_rate,
        channels,
        drops,
        mic_sample_rate: None,
        loop_sample_rate: None,
        loop_channels: None,
        mic_channels: None,
        aec_fell_back: false,
        _streams: vec![Box::new(WasapiStreamThread {
            stop,
            handle: Some(handle),
        }) as Box<dyn KeepAlive>],
    })
}

/// COM init for the current thread. S_OK: initialised. S_FALSE: already init in
/// the same mode (benign). RPC_E_CHANGED_MODE: the thread is already init in a
/// DIFFERENT apartment (e.g. the Tauri command thread is STA) — also fine, since
/// `IAudioClient` activation works in both STA and MTA. Any other failure is real.
/// This matters because `open_mic_comms` may run on a thread Tauri already
/// CoInitialize'd; only `mic_comms_loop`'s fresh thread is guaranteed clean MTA.
fn ensure_com() -> Result<(), AudioError> {
    const RPC_E_CHANGED_MODE: i32 = 0x8001_0106u32 as i32;
    // SAFETY: standard WASAPI init.
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if hr.is_err() && hr.0 != RPC_E_CHANGED_MODE {
        return Err(AudioError::WasapiInit { hresult: hr.0 });
    }
    Ok(())
}

/// Get an IMMDevice for the endpoint id string via the enumerator.
///
/// # Safety
/// Must be called on a COM-initialised thread (see `ensure_com`).
unsafe fn device_for_id(endpoint_id: &str) -> Result<IMMDevice, AudioError> {
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
        .map_err(|e| AudioError::WasapiInit {
        hresult: e.code().0,
    })?;
    let wide: Vec<u16> = endpoint_id
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    enumerator
        .GetDevice(PCWSTR(wide.as_ptr()))
        .map_err(|e| AudioError::WasapiInit {
            hresult: e.code().0,
        })
}

/// Activate a client and read its shared-mode mix format, then drop it. The
/// shared-mode mix format is category-independent, so this deliberately skips the
/// comms-category call (it would be a wasted QueryInterface here); the category is
/// set in `mic_comms_loop`, before Initialize, where it actually pulls in the AEC.
fn read_comms_mix_format(endpoint_id: &str) -> Result<(u32, u16), AudioError> {
    ensure_com()?;
    unsafe {
        let device = device_for_id(endpoint_id)?;
        let client: IAudioClient =
            device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| AudioError::WasapiInit {
                    hresult: e.code().0,
                })?;
        let fmt = client.GetMixFormat().map_err(|e| AudioError::WasapiInit {
            hresult: e.code().0,
        })?;
        let rate = (*fmt).nSamplesPerSec;
        let ch = (*fmt).nChannels;
        CoTaskMemFree(Some(fmt as *const _));
        Ok((rate, ch))
    }
}

/// Set `eCategory = AudioCategory_Communications` BEFORE Initialize (pulls the
/// AEC/Voice-Clarity APO into the capture chain). Must not use RAW.
///
/// `SetClientProperties` lives on `IAudioClient2` in windows-rs 0.59, so we
/// QueryInterface (`.cast`) the `IAudioClient` to its `IAudioClient2` view (same
/// COM object) and set the category through it.
///
/// # Safety
/// `client` must be a freshly-activated, not-yet-Initialized `IAudioClient`.
unsafe fn set_comms_category(client: &IAudioClient) -> Result<(), AudioError> {
    let client2: IAudioClient2 = client.cast().map_err(|e| AudioError::WasapiInit {
        hresult: e.code().0,
    })?;
    let props = AudioClientProperties {
        cbSize: std::mem::size_of::<AudioClientProperties>() as u32,
        bIsOffload: false.into(),
        eCategory: AudioCategory_Communications,
        Options: Default::default(),
    };
    client2
        .SetClientProperties(&props)
        .map_err(|e| AudioError::WasapiInit {
            hresult: e.code().0,
        })
}

/// Capture loop: activate → comms category → initialize (event-driven) → set AEC
/// reference → start → drain buffers → push f32. Runs on its own thread with its
/// own COM apartment.
fn mic_comms_loop(
    endpoint_id: &str,
    reference: Option<String>,
    tx: &Sender<Vec<f32>>,
    drops: &Arc<AtomicU32>,
    stop: &Arc<AtomicBool>,
) -> Result<(), AudioError> {
    ensure_com()?;
    unsafe {
        let device = device_for_id(endpoint_id)?;
        let client: IAudioClient =
            device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| AudioError::WasapiInit {
                    hresult: e.code().0,
                })?;
        set_comms_category(&client)?;

        let fmt = client.GetMixFormat().map_err(|e| AudioError::WasapiInit {
            hresult: e.code().0,
        })?;
        let channels = (*fmt).nChannels as usize;

        // 200 ms buffer (in 100-ns units), shared, event-driven. Initialize copies
        // the format synchronously, so free `fmt` right after the call regardless of
        // outcome (an Initialize error would otherwise skip the free via `?`), then
        // propagate any error.
        let init = client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            2_000_000,
            0,
            fmt as *const _,
            None,
        );
        CoTaskMemFree(Some(fmt as *const _));
        init.map_err(|e| AudioError::WasapiInit {
            hresult: e.code().0,
        })?;

        // Set the AEC reference (non-fatal: E_NOINTERFACE means AEC still runs,
        // OS auto-picks the reference).
        match client.GetService::<IAcousticEchoCancellationControl>() {
            Ok(aec) => {
                let hr = match &reference {
                    Some(id) => {
                        let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
                        aec.SetEchoCancellationRenderEndpoint(PCWSTR(wide.as_ptr()))
                    }
                    None => aec.SetEchoCancellationRenderEndpoint(PCWSTR::null()),
                };
                if let Err(e) = hr {
                    tracing::warn!(
                        hresult = e.code().0,
                        "SetEchoCancellationRenderEndpoint failed; OS auto-reference"
                    );
                }
            }
            Err(e) => {
                tracing::info!(
                    hresult = e.code().0,
                    "no IAcousticEchoCancellationControl (E_NOINTERFACE); OS auto-reference"
                );
            }
        }

        // Owned event handle, closed on every exit path by the guard's Drop (it
        // drops after the capture loop returns, i.e. after `client.Stop()`).
        let event = EventHandle(
            CreateEventW(None, false, false, PCWSTR::null()).map_err(|e| {
                AudioError::WasapiInit {
                    hresult: e.code().0,
                }
            })?,
        );
        client
            .SetEventHandle(event.0)
            .map_err(|e| AudioError::WasapiInit {
                hresult: e.code().0,
            })?;

        let capture: IAudioCaptureClient =
            client.GetService().map_err(|e| AudioError::WasapiInit {
                hresult: e.code().0,
            })?;

        client.Start().map_err(|e| AudioError::WasapiInit {
            hresult: e.code().0,
        })?;
        tracing::info!(channels, "WASAPI comms-mic (OS AEC) started");

        loop {
            if stop.load(Ordering::Relaxed) {
                let _ = client.Stop();
                return Ok(());
            }
            // Wait up to 100 ms so we can re-check the stop flag.
            if WaitForSingleObject(event.0, 100) != WAIT_OBJECT_0 {
                continue;
            }
            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let mut data: *mut u8 = std::ptr::null_mut();
                let mut frames: u32 = 0;
                let mut flags: u32 = 0;
                let hr = capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None);
                if hr.is_err() || frames == 0 {
                    break;
                }
                // Comms mix format is IEEE float; decode interleaved f32.
                let n = frames as usize * channels;
                // SAFETY: GetBuffer succeeded (hr ok, frames > 0), so `data` points
                // to `frames` valid frames; the comms mix format is interleaved f32,
                // so the buffer holds exactly `frames * channels` = `n` f32 samples.
                let mut samples: Vec<f32> =
                    std::slice::from_raw_parts(data as *const f32, n).to_vec();
                // Honor AUDCLNT_BUFFERFLAGS_SILENT: the buffer contents are undefined
                // when set, so don't feed garbage to Whisper — zero the samples.
                if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                    samples.iter_mut().for_each(|s| *s = 0.0);
                }
                let _ = capture.ReleaseBuffer(frames);
                if tx.try_send(samples).is_err() {
                    drops.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comms_mic_only_for_mix_with_aec() {
        assert!(use_comms_mic(CaptureMode::Mix, true));
        assert!(!use_comms_mic(CaptureMode::Mix, false));
        assert!(!use_comms_mic(CaptureMode::Mic, true));
        assert!(!use_comms_mic(CaptureMode::System, true));
    }

    #[test]
    fn sentinel_reference_is_auto_else_pinned() {
        assert!(aec_reference_is_auto(DEFAULT_RENDER_LOOPBACK));
        assert!(!aec_reference_is_auto("some-endpoint-id"));
    }
}
