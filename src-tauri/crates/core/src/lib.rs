pub mod ai_prompt;
pub mod error;
pub mod lang;
pub mod models;
pub mod traits;

pub use error::AppError;
pub use error::AudioErrorCode;
pub use error::TranscriptionErrorPayload;
pub use lang::Bilingual;
pub use models::Marker;
pub use models::MeetingAsset;
pub use models::MeetingDetail;
