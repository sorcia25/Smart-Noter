//! Pure, model-free alignment: assign each transcription text segment the
//! diarization speaker whose segments overlap it most (in total). No external deps.

/// One diarization region (output of the sherpa-rs pipeline), in **milliseconds**.
/// (Phase 3 converts sherpa's seconds → ms before calling `align`.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiarSegment {
    pub start_ms: u32,
    pub end_ms: u32,
    pub speaker: u32,
}

/// A transcription segment after speaker assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlignedLine {
    pub start_ms: u32,
    pub end_ms: u32,
    pub speaker: u32,
    pub text: String,
}

/// Minimal text-segment shape the aligner needs (a structural mirror of
/// `smart_noter_whisper::Segment`, kept local so this crate has no whisper dep).
#[derive(Debug, Clone)]
pub struct TextSegment {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
}

/// Overlap (in ms) of [a0,a1) and [b0,b1); 0 if disjoint.
fn overlap_ms(a0: u32, a1: u32, b0: u32, b1: u32) -> u32 {
    let lo = a0.max(b0);
    let hi = a1.min(b1);
    hi.saturating_sub(lo)
}

/// Distance (in ms) between text segment [a0,a1) and diar segment [b0,b1).
/// 0 when they touch or overlap. Only ever called in the no-overlap fallback.
fn gap_ms(a0: u32, a1: u32, b0: u32, b1: u32) -> u32 {
    if a1 <= b0 {
        b0 - a1
    } else {
        a0.saturating_sub(b1)
    }
}

/// Assign each text segment a speaker. Rule: the speaker whose segments have the
/// greatest TOTAL overlap with the text wins; ties break to the lower speaker
/// number (determinism). If a text segment overlaps no diar segment, fall back to
/// the NEAREST diar segment (smallest gap; gap ties → lower speaker). If there are
/// no diar segments at all, everything is speaker 0.
pub fn align(texts: &[TextSegment], diar: &[DiarSegment]) -> Vec<AlignedLine> {
    texts
        .iter()
        .map(|t| {
            let speaker = pick_speaker(t.start_ms, t.end_ms, diar);
            AlignedLine {
                start_ms: t.start_ms,
                end_ms: t.end_ms,
                speaker,
                text: t.text.clone(),
            }
        })
        .collect()
}

fn pick_speaker(t0: u32, t1: u32, diar: &[DiarSegment]) -> u32 {
    if diar.is_empty() {
        return 0;
    }

    // 1) Greatest TOTAL overlap per speaker. A speaker may own several segments,
    //    and the text may straddle more than one — so we sum overlap per speaker.
    let mut totals: Vec<(u32 /*speaker*/, u32 /*total_overlap*/)> = Vec::new();
    for d in diar {
        let ov = overlap_ms(t0, t1, d.start_ms, d.end_ms);
        if ov == 0 {
            continue;
        }
        match totals.iter_mut().find(|(sp, _)| *sp == d.speaker) {
            Some((_, acc)) => *acc += ov,
            None => totals.push((d.speaker, ov)),
        }
    }
    if !totals.is_empty() {
        // Max total overlap; on a tie, the LOWER speaker number wins.
        return totals
            .iter()
            .copied()
            .max_by(|(sp_a, ov_a), (sp_b, ov_b)| ov_a.cmp(ov_b).then(sp_b.cmp(sp_a)))
            .map(|(sp, _)| sp)
            .unwrap();
    }

    // 2) No speaker overlaps the text → nearest diar segment by gap (tie → lower speaker).
    let mut nearest: Option<(u32 /*gap*/, u32 /*speaker*/)> = None;
    for d in diar {
        let g = gap_ms(t0, t1, d.start_ms, d.end_ms);
        match nearest {
            Some((bg, bsp)) if (g, d.speaker) >= (bg, bsp) => {}
            _ => nearest = Some((g, d.speaker)),
        }
    }
    nearest.map(|(_, sp)| sp).unwrap_or(0)
}

/// Give every text segment a real duration for overlap scoring. Transcript lines
/// persisted at second granularity (or legacy rows with NULL end) can arrive with
/// `end_ms <= start_ms`; such a point interval overlaps no diar segment and forces
/// `align` into its nearest-gap fallback. Fill each degenerate line's end from the
/// NEXT line's start; the last line uses `audio_end_ms`. A `start_ms + 1` floor
/// covers the rare case where the next line starts no later (same second), so the
/// interval is never empty.
pub fn fill_zero_durations(texts: &[TextSegment], audio_end_ms: u32) -> Vec<TextSegment> {
    let n = texts.len();
    texts
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut end = t.end_ms;
            if end <= t.start_ms {
                let next = if i + 1 < n {
                    texts[i + 1].start_ms
                } else {
                    audio_end_ms
                };
                end = if next > t.start_ms {
                    next
                } else {
                    t.start_ms + 1
                };
            }
            TextSegment {
                start_ms: t.start_ms,
                end_ms: end,
                text: t.text.clone(),
            }
        })
        .collect()
}

/// Remap speaker labels to a contiguous `[0..k)` range, preserving first-appearance
/// order. `align` can emit non-contiguous labels (sherpa uses e.g. {0,4,5}); the
/// caller derives the participant count from `max(speaker)+1`, which would create
/// phantom empty participants. Returns `(remapped, k)` where `k` is the real number
/// of distinct speakers used.
pub fn remap_contiguous(speakers: &[u32]) -> (Vec<u32>, usize) {
    let mut mapping: Vec<u32> = Vec::new();
    let out = speakers
        .iter()
        .map(|&s| match mapping.iter().position(|&o| o == s) {
            Some(idx) => idx as u32,
            None => {
                mapping.push(s);
                (mapping.len() - 1) as u32
            }
        })
        .collect();
    (out, mapping.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(start_ms: u32, end_ms: u32, text: &str) -> TextSegment {
        TextSegment {
            start_ms,
            end_ms,
            text: text.into(),
        }
    }
    fn d(start_ms: u32, end_ms: u32, speaker: u32) -> DiarSegment {
        DiarSegment {
            start_ms,
            end_ms,
            speaker,
        }
    }

    #[test]
    fn fill_uses_next_line_start_then_audio_end() {
        let texts = vec![t(0, 0, "a"), t(5000, 0, "b"), t(9000, 0, "c")];
        let out = fill_zero_durations(&texts, 12000);
        assert_eq!(out[0].end_ms, 5000);
        assert_eq!(out[1].end_ms, 9000);
        assert_eq!(out[2].end_ms, 12000);
    }

    #[test]
    fn fill_keeps_real_durations() {
        let texts = vec![t(0, 3000, "a")];
        assert_eq!(fill_zero_durations(&texts, 10000)[0].end_ms, 3000);
    }

    #[test]
    fn fill_same_instant_lines_get_1ms_floor() {
        let texts = vec![t(5000, 5000, "a"), t(5000, 5000, "b")];
        let out = fill_zero_durations(&texts, 10000);
        assert_eq!(out[0].end_ms, 5001);
        assert_eq!(out[1].end_ms, 10000);
    }

    #[test]
    fn remap_non_contiguous_to_zero_based() {
        let (out, k) = remap_contiguous(&[0, 4, 4, 5, 0]);
        assert_eq!(out, vec![0, 1, 1, 2, 0]);
        assert_eq!(k, 3);
    }

    #[test]
    fn remap_preserves_first_appearance_order() {
        let (out, k) = remap_contiguous(&[5, 5, 2, 0]);
        assert_eq!(out, vec![0, 0, 1, 2]);
        assert_eq!(k, 3);
    }

    #[test]
    fn remap_empty_is_zero() {
        let (out, k) = remap_contiguous(&[]);
        assert!(out.is_empty());
        assert_eq!(k, 0);
    }

    #[test]
    fn clean_turns_each_line_gets_its_speaker() {
        let texts = vec![t(0, 1000, "hola"), t(2000, 3000, "que tal")];
        let diar = vec![d(0, 1500, 0), d(1500, 3500, 1)];
        let out = align(&texts, &diar);
        assert_eq!(out[0].speaker, 0);
        assert_eq!(out[1].speaker, 1);
    }

    #[test]
    fn text_straddling_a_boundary_goes_to_the_greater_overlap() {
        // 0..1200 overlaps spk0 by 1000ms (0..1000) and spk1 by 200ms (1000..1200)
        let texts = vec![t(0, 1200, "straddle")];
        let diar = vec![d(0, 1000, 0), d(1000, 5000, 1)];
        assert_eq!(align(&texts, &diar)[0].speaker, 0);
    }

    #[test]
    fn no_overlap_falls_back_to_nearest() {
        let texts = vec![t(4000, 4500, "orphan")];
        let diar = vec![d(0, 1000, 0), d(3000, 3800, 1)]; // nearest is spk1 (gap 200)
        assert_eq!(align(&texts, &diar)[0].speaker, 1);
    }

    #[test]
    fn empty_diarization_assigns_speaker_zero() {
        let texts = vec![t(0, 1000, "alone")];
        assert_eq!(align(&texts, &[])[0].speaker, 0);
    }

    #[test]
    fn overlap_tie_breaks_to_lower_speaker_when_higher_is_last() {
        // equal 500ms overlap; lower speaker (0) appears LAST in the slice
        let texts = vec![t(500, 1500, "tie")];
        let diar = vec![d(0, 1000, 1), d(1000, 2000, 0)];
        assert_eq!(align(&texts, &diar)[0].speaker, 0);
    }

    #[test]
    fn overlap_tie_breaks_to_lower_speaker_when_lower_is_first() {
        // equal 500ms overlap; lower speaker (0) appears FIRST in the slice.
        // Together with the previous test this proves the tie-break is
        // order-independent (not "first-wins" or "last-wins").
        let texts = vec![t(500, 1500, "tie")];
        let diar = vec![d(0, 1000, 0), d(1000, 2000, 1)];
        assert_eq!(align(&texts, &diar)[0].speaker, 0);
    }

    #[test]
    fn greatest_total_overlap_per_speaker_not_single_segment() {
        // spk0 has TWO fragments overlapping the text by 100+100=200ms total;
        // spk1 has ONE fragment overlapping by 150ms. Per-speaker total wins → spk0,
        // even though spk1's single best segment (150) beats either spk0 fragment (100).
        let texts = vec![t(0, 1000, "multi")];
        let diar = vec![d(0, 100, 0), d(100, 250, 1), d(250, 350, 0)];
        assert_eq!(align(&texts, &diar)[0].speaker, 0);
    }

    #[test]
    fn equidistant_fallback_breaks_to_lower_speaker() {
        // text overlaps neither; gap to spk1 (100) == gap to spk0 (100) → lower speaker 0
        let texts = vec![t(1100, 1900, "between")];
        let diar = vec![d(0, 1000, 1), d(2000, 3000, 0)];
        assert_eq!(align(&texts, &diar)[0].speaker, 0);
    }

    #[test]
    fn preserves_text_and_timestamps() {
        let texts = vec![t(7, 9, "x")];
        let diar = vec![d(0, 100, 3)];
        let out = align(&texts, &diar);
        assert_eq!(out[0].start_ms, 7);
        assert_eq!(out[0].end_ms, 9);
        assert_eq!(out[0].text, "x");
    }
}
