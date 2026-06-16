#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
}

/// whisper.cpp segment timestamps are in centiseconds (10 ms units).
pub fn cs_to_seconds(centiseconds: i64) -> u32 {
    (centiseconds.max(0) / 100) as u32
}

pub fn fmt_timestamp(t_seconds: u32) -> String {
    let h = t_seconds / 3600;
    let m = (t_seconds % 3600) / 60;
    let s = t_seconds % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

pub fn word_count(text: &str) -> u32 {
    text.split_whitespace().count() as u32
}

/// Tuning knobs for one transcription run.
#[derive(Debug, Clone)]
pub struct TranscribeOpts {
    pub n_threads: i32,
    /// `None` → auto-detect language; `Some("es")` to force.
    pub language: Option<String>,
}

impl Default for TranscribeOpts {
    fn default() -> Self {
        Self {
            n_threads: 4,
            language: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centiseconds_to_t_seconds_truncates() {
        assert_eq!(cs_to_seconds(0), 0);
        assert_eq!(cs_to_seconds(450), 4); // 4.5 s → 4
        assert_eq!(cs_to_seconds(6000), 60);
    }

    #[test]
    fn t_display_is_hh_mm_ss() {
        assert_eq!(fmt_timestamp(0), "00:00:00");
        assert_eq!(fmt_timestamp(4), "00:00:04");
        assert_eq!(fmt_timestamp(3661), "01:01:01");
    }

    #[test]
    fn word_count_counts_whitespace_separated_tokens() {
        assert_eq!(word_count("hola que tal"), 3);
        assert_eq!(word_count("  uno   dos  "), 2);
        assert_eq!(word_count(""), 0);
    }
}
