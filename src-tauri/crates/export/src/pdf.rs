use crate::{ExportError, ExportOpts};
use smart_noter_core::models::MeetingDetail;

pub fn to_pdf(_m: &MeetingDetail, _opts: &ExportOpts) -> Result<Vec<u8>, ExportError> {
    Ok(Vec::new())
}
