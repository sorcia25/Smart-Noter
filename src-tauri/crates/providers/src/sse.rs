use std::io::{BufRead, BufReader, Read};

/// Read an SSE stream, calling `on_data(payload)` for each `data: <payload>` line
/// (excluding the literal `[DONE]` and blank payloads). Returns Ok when the stream ends.
pub fn read_sse<R: Read>(reader: R, mut on_data: impl FnMut(&str)) -> std::io::Result<()> {
    let buf = BufReader::new(reader);
    for line in buf.lines() {
        let line = line?;
        if let Some(rest) = line.strip_prefix("data:") {
            let payload = rest.trim();
            if payload.is_empty() || payload == "[DONE]" {
                continue;
            }
            on_data(payload);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiple_data_lines_yield_payloads_in_order() {
        let input = b"data: hello\ndata: world\n";
        let mut collected = Vec::new();
        read_sse(&input[..], |p| collected.push(p.to_string())).unwrap();
        assert_eq!(collected, vec!["hello", "world"]);
    }

    #[test]
    fn done_sentinel_is_skipped() {
        let input = b"data: first\ndata: [DONE]\ndata: second\n";
        let mut collected = Vec::new();
        read_sse(&input[..], |p| collected.push(p.to_string())).unwrap();
        // [DONE] is skipped; second comes after [DONE] so it would be unreachable
        // in real SSE, but our parser is tolerant
        assert_eq!(collected, vec!["first", "second"]);
    }

    #[test]
    fn blank_lines_and_non_data_lines_are_ignored() {
        let input = b"event: open\n\ndata: payload\n: comment\nid: 42\n";
        let mut collected = Vec::new();
        read_sse(&input[..], |p| collected.push(p.to_string())).unwrap();
        assert_eq!(collected, vec!["payload"]);
    }

    #[test]
    fn multi_event_buffer_surfaces_exactly_a_and_b() {
        let input = b"data: a\n\ndata: b\ndata: [DONE]\n";
        let mut collected = Vec::new();
        read_sse(&input[..], |p| collected.push(p.to_string())).unwrap();
        assert_eq!(collected, vec!["a", "b"]);
    }

    #[test]
    fn blank_data_payload_is_skipped() {
        let input = b"data: \ndata: content\n";
        let mut collected = Vec::new();
        read_sse(&input[..], |p| collected.push(p.to_string())).unwrap();
        assert_eq!(collected, vec!["content"]);
    }
}
