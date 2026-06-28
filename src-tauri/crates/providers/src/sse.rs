use std::io::{BufRead, BufReader, Read};
use std::ops::ControlFlow;

/// Read an SSE stream, calling `on_data(payload)` for each `data: <payload>` line
/// (excluding the literal `[DONE]` and blank payloads). Returns Ok when the stream ends.
///
/// The callback returns `ControlFlow::Continue(())` to keep reading or
/// `ControlFlow::Break(())` to stop early (e.g. when abort is requested).
pub fn read_sse<R: Read>(
    reader: R,
    mut on_data: impl FnMut(&str) -> ControlFlow<()>,
) -> std::io::Result<()> {
    let buf = BufReader::new(reader);
    for line in buf.lines() {
        let line = line?;
        if let Some(rest) = line.strip_prefix("data:") {
            let payload = rest.trim();
            if payload.is_empty() || payload == "[DONE]" {
                continue;
            }
            if on_data(payload).is_break() {
                break;
            }
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
        read_sse(&input[..], |p| {
            collected.push(p.to_string());
            ControlFlow::Continue(())
        })
        .unwrap();
        assert_eq!(collected, vec!["hello", "world"]);
    }

    #[test]
    fn done_sentinel_is_skipped() {
        let input = b"data: first\ndata: [DONE]\ndata: second\n";
        let mut collected = Vec::new();
        read_sse(&input[..], |p| {
            collected.push(p.to_string());
            ControlFlow::Continue(())
        })
        .unwrap();
        // [DONE] is skipped; second comes after [DONE] so it would be unreachable
        // in real SSE, but our parser is tolerant
        assert_eq!(collected, vec!["first", "second"]);
    }

    #[test]
    fn blank_lines_and_non_data_lines_are_ignored() {
        let input = b"event: open\n\ndata: payload\n: comment\nid: 42\n";
        let mut collected = Vec::new();
        read_sse(&input[..], |p| {
            collected.push(p.to_string());
            ControlFlow::Continue(())
        })
        .unwrap();
        assert_eq!(collected, vec!["payload"]);
    }

    #[test]
    fn multi_event_buffer_surfaces_exactly_a_and_b() {
        let input = b"data: a\n\ndata: b\ndata: [DONE]\n";
        let mut collected = Vec::new();
        read_sse(&input[..], |p| {
            collected.push(p.to_string());
            ControlFlow::Continue(())
        })
        .unwrap();
        assert_eq!(collected, vec!["a", "b"]);
    }

    #[test]
    fn blank_data_payload_is_skipped() {
        let input = b"data: \ndata: content\n";
        let mut collected = Vec::new();
        read_sse(&input[..], |p| {
            collected.push(p.to_string());
            ControlFlow::Continue(())
        })
        .unwrap();
        assert_eq!(collected, vec!["content"]);
    }

    #[test]
    fn break_stops_processing_early() {
        // Three payloads but the callback breaks after the first.
        let input = b"data: one\ndata: two\ndata: three\n";
        let mut collected = Vec::new();
        read_sse(&input[..], |p| {
            collected.push(p.to_string());
            ControlFlow::Break(()) // stop after the very first payload
        })
        .unwrap();
        assert_eq!(collected, vec!["one"]);
    }
}
