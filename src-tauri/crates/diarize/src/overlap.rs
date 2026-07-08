//! Flatten overlapping diarization segments into a non-overlapping partition.
use crate::align::DiarSegment;

/// Resolve temporal overlaps in sherpa's diarization output. Where two or more
/// segments cover the same instant, the SHORTEST original segment wins that slice
/// (a short nested turn is more specific than the long span that encloses it);
/// ties in duration break to the lower speaker number. Adjacent output slices with
/// the same speaker are merged. If the input has no overlaps, the output is
/// identical to the input (no-op) — this is why it's safe on clean diarization.
pub fn flatten_overlaps(segments: &[DiarSegment]) -> Vec<DiarSegment> {
    if segments.len() < 2 {
        return segments.to_vec();
    }
    let mut cuts: Vec<u32> = Vec::with_capacity(segments.len() * 2);
    for s in segments {
        cuts.push(s.start_ms);
        cuts.push(s.end_ms);
    }
    cuts.sort_unstable();
    cuts.dedup();

    let mut out: Vec<DiarSegment> = Vec::new();
    for w in cuts.windows(2) {
        let (lo, hi) = (w[0], w[1]);
        if lo >= hi {
            continue;
        }
        let mut best: Option<&DiarSegment> = None;
        for s in segments {
            if s.start_ms <= lo && hi <= s.end_ms {
                match best {
                    Some(b) => {
                        let cand = s.end_ms - s.start_ms;
                        let cur = b.end_ms - b.start_ms;
                        if cand < cur || (cand == cur && s.speaker < b.speaker) {
                            best = Some(s);
                        }
                    }
                    None => best = Some(s),
                }
            }
        }
        let Some(b) = best else {
            continue;
        };
        if let Some(last) = out.last_mut() {
            if last.speaker == b.speaker && last.end_ms == lo {
                last.end_ms = hi;
                continue;
            }
        }
        out.push(DiarSegment {
            start_ms: lo,
            end_ms: hi,
            speaker: b.speaker,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(start_ms: u32, end_ms: u32, speaker: u32) -> DiarSegment {
        DiarSegment {
            start_ms,
            end_ms,
            speaker,
        }
    }

    #[test]
    fn no_overlap_is_identity() {
        let segs = vec![d(0, 1000, 0), d(1000, 2000, 1), d(2000, 3000, 0)];
        assert_eq!(flatten_overlaps(&segs), segs);
    }

    #[test]
    fn nested_short_turn_surfaces() {
        let segs = vec![d(0, 10000, 0), d(4000, 5000, 1)];
        let out = flatten_overlaps(&segs);
        assert_eq!(
            out,
            vec![d(0, 4000, 0), d(4000, 5000, 1), d(5000, 10000, 0)]
        );
    }

    #[test]
    fn partial_overlap_shorter_wins_shared_slice() {
        let segs = vec![d(0, 6000, 0), d(4000, 7000, 1)];
        let out = flatten_overlaps(&segs);
        assert_eq!(out, vec![d(0, 4000, 0), d(4000, 7000, 1)]);
    }

    #[test]
    fn equal_duration_tie_breaks_to_lower_speaker_then_merges() {
        let segs = vec![d(0, 2000, 1), d(1000, 3000, 0)];
        let out = flatten_overlaps(&segs);
        assert_eq!(out, vec![d(0, 1000, 1), d(1000, 3000, 0)]);
    }
}
