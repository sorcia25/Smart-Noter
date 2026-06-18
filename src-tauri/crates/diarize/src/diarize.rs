use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::align::DiarSegment;
use crate::error::{DiarizationError, DiarizationErrorCode};

/// Knobs for one diarization run.
#[derive(Debug, Clone, Default)]
pub struct DiarizeOpts {
    /// `Some(n)` forces exactly n speakers (the user's hint); `None` auto-detects via clustering.
    pub num_speakers: Option<u32>,
}

fn derr(code: DiarizationErrorCode, m: impl Into<String>) -> DiarizationError {
    DiarizationError {
        code,
        message: m.into(),
    }
}

/// Run sherpa-rs diarization over 16 kHz mono f32 PCM. Returns speaker regions
/// in **milliseconds** (sherpa reports seconds; we convert here so `align` is ms-only).
/// `abort` is polled cooperatively; on abort we return `Cancelled`.
pub fn diarize(
    pcm: &[f32],
    seg_model: &Path,
    emb_model: &Path,
    opts: &DiarizeOpts,
    abort: Arc<AtomicBool>,
) -> Result<Vec<DiarSegment>, DiarizationError> {
    if abort.load(Ordering::Relaxed) {
        return Err(derr(
            DiarizationErrorCode::Cancelled,
            "cancelled before start",
        ));
    }

    use sherpa_rs::diarize::{Diarize, DiarizeConfig};

    // num_clusters and threshold are mutually exclusive: a hint forces the count;
    // otherwise auto-detect via the clustering threshold.
    let (num_clusters, threshold) = match opts.num_speakers {
        Some(n) => (Some(n as i32), None),
        None => (None, Some(0.5_f32)),
    };
    let config = DiarizeConfig {
        num_clusters,
        threshold,
        min_duration_on: Some(0.3),
        min_duration_off: Some(0.5),
        provider: None,
        debug: false,
    };

    let mut sd = Diarize::new(seg_model, emb_model, config)
        .map_err(|e| derr(DiarizationErrorCode::ModelLoadFailed, e.to_string()))?;

    // sherpa-rs `compute` takes ownership of the samples; it has no abort hook, so
    // diarization runs to completion once started (we checked `abort` above; the
    // job-level cancel still interrupts the whisper phase). Segments come back
    // sorted by start, in SECONDS — convert to ms for the aligner.
    let raw = sd
        .compute(pcm.to_vec(), None)
        .map_err(|e| derr(DiarizationErrorCode::DiarizationFailed, e.to_string()))?;

    let segments = raw
        .into_iter()
        .map(|s| DiarSegment {
            start_ms: (s.start.max(0.0) * 1000.0) as u32,
            end_ms: (s.end.max(0.0) * 1000.0) as u32,
            speaker: s.speaker.max(0) as u32,
        })
        .collect();
    Ok(segments)
}
