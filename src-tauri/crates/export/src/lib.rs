//! Pure, side-effect-free meeting exporters: one function per format, each
//! takes the already-loaded `MeetingDetail` (or an audio path) and returns
//! bytes/string. No DB, no filesystem dialog — the binary's `export_meeting`
//! command orchestrates I/O.

pub mod audio;
pub mod markdown;
pub mod pdf;

use thiserror::Error;

use smart_noter_core::Bilingual;

/// One text line for a bilingual value: `es` always; ` / en` appended when
/// `bilingual` is on and an `en` exists.
pub(crate) fn bi(text: &Bilingual, opts: &ExportOpts) -> String {
    match (&text.en, opts.bilingual) {
        (Some(en), true) if !en.is_empty() => format!("{} / {}", text.es, en),
        _ => text.es.clone(),
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
