use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MeetingAsset {
    pub id: String,
    pub meeting_id: String,
    pub kind: String,
    pub path: String,
    pub bytes: i64,
    pub mime_type: Option<String>,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_camelcase_keys() {
        let a = MeetingAsset {
            id: "a-1".into(),
            meeting_id: "m-1".into(),
            kind: "audio".into(),
            path: "C:/x.wav".into(),
            bytes: 1024,
            mime_type: Some("audio/wav".into()),
            created_at: "2026-05-19T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""meetingId":"m-1""#));
        assert!(json.contains(r#""mimeType":"audio/wav""#));
        assert!(json.contains(r#""createdAt":"2026-05-19T00:00:00Z""#));
    }
}
