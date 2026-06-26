use crate::ExportError;
use std::path::Path;

pub fn wav_or_flac_to_mp3(_path: &Path) -> Result<Vec<u8>, ExportError> {
    Ok(Vec::new())
}
