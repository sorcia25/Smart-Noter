//! Pure, side-effect-free meeting exporters: one function per format, each
//! takes the already-loaded `MeetingDetail` (or an audio path) and returns
//! bytes/string. No DB, no filesystem dialog — the binary's `export_meeting`
//! command orchestrates I/O.

pub mod audio;
pub mod markdown;
pub mod pdf;

use thiserror::Error;

use smart_noter_core::models::Participant;
use smart_noter_core::Bilingual;

/// One text line for a bilingual value: `es` always; ` / en` appended when
/// `bilingual` is on and an `en` exists.
pub(crate) fn bi(text: &Bilingual, opts: &ExportOpts) -> String {
    match (&text.en, opts.bilingual) {
        (Some(en), true) if !en.is_empty() => format!("{} / {}", text.es, en),
        _ => text.es.clone(),
    }
}

/// Resolves a transcript line's speaker to a display name: the participant's
/// `name` if set, else its `label`, else an em dash for unknown speakers.
pub(crate) fn speaker_name(participants: &[Participant], speaker_id: &str) -> String {
    participants
        .iter()
        .find(|p| p.id == speaker_id)
        .map(|p| p.name.clone().unwrap_or_else(|| p.label.clone()))
        .unwrap_or_else(|| "—".into())
}

/// Formats a duration in seconds as `MM:SS`, or `H:MM:SS` when at least an hour.
pub(crate) fn fmt_duration(sec: i64) -> String {
    let h = sec / 3600;
    let m = (sec % 3600) / 60;
    let s = sec % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// Per-export options from the modal. `timestamps`/`bilingual` apply to text
/// formats only (ignored by MP3).
#[derive(Debug, Clone, Copy)]
pub struct ExportOpts {
    pub timestamps: bool,
    pub bilingual: bool,
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("audio decode failed: {0}")]
    Decode(String),
    #[error("mp3 encode failed: {0}")]
    Mp3(String),
    #[error("pdf render failed: {0}")]
    Pdf(String),
    #[error("unsupported audio format: {0}")]
    UnsupportedAudio(String),
}

pub use audio::wav_or_flac_to_mp3;
pub use markdown::to_markdown;
pub use pdf::to_pdf;
