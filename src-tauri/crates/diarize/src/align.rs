//! Pure, model-free alignment: assign each transcription text segment the
//! diarization speaker whose time range overlaps it most. No external deps.

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

/// Distance (in ms) from text segment [a0,a1) to diar segment [b0,b1); 0 if they touch/overlap.
fn gap_ms(a0: u32, a1: u32, b0: u32, b1: u32) -> u32 {
    if a1 <= b0 {
        b0 - a1
    } else {
        a0.saturating_sub(b1)
    }
}

/// Assign each text segment a speaker. Rule: the diar segment with the greatest
/// overlap wins; ties break to the lower speaker number for determinism. If a
/// text segment overlaps no diar segment, fall back to the **nearest** diar
/// segment (smallest gap). If there are no diar segments at all, everything is
/// speaker 0.
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
    // 1) best by overlap
    let mut best: Option<(u32 /*overlap*/, u32 /*speaker*/)> = None;
    for d in diar {
        let ov = overlap_ms(t0, t1, d.start_ms, d.end_ms);
        if ov > 0 {
            match best {
                Some((bov, bsp))
                    if (ov, std::cmp::Reverse(d.speaker)) <= (bov, std::cmp::Reverse(bsp)) => {}
                _ => best = Some((ov, d.speaker)),
            }
        }
    }
    if let Some((_, sp)) = best {
        return sp;
    }
    // 2) no overlap → nearest by gap (ties → lower speaker)
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
    fn overlap_tie_breaks_to_lower_speaker_number() {
        // equal 500ms overlap with spk0 and spk1 → spk0 wins
        let texts = vec![t(500, 1500, "tie")];
        let diar = vec![d(0, 1000, 1), d(1000, 2000, 0)];
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
