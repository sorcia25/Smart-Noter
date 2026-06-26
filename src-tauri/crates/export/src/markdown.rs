use crate::ExportOpts;
use smart_noter_core::models::{MeetingDetail, Participant};
use smart_noter_core::Bilingual;

/// One Markdown line for a bilingual value: `es` always; ` / en` appended when
/// `bilingual` is on and an `en` exists.
fn bi(text: &Bilingual, opts: &ExportOpts) -> String {
    match (&text.en, opts.bilingual) {
        (Some(en), true) if !en.is_empty() => format!("{} / {}", text.es, en),
        _ => text.es.clone(),
    }
}

fn speaker_name(participants: &[Participant], speaker_id: &str) -> String {
    participants
        .iter()
        .find(|p| p.id == speaker_id)
        .map(|p| p.name.clone().unwrap_or_else(|| p.label.clone()))
        .unwrap_or_else(|| "—".into())
}

fn fmt_duration(sec: i64) -> String {
    let h = sec / 3600;
    let m = (sec % 3600) / 60;
    let s = sec % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

pub fn to_markdown(m: &MeetingDetail, opts: &ExportOpts) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", bi(&m.title, opts)));
    out.push_str(&format!("**Fecha:** {}  \n", m.date));
    out.push_str(&format!("**Duración:** {}\n", fmt_duration(m.duration_sec)));

    if !m.participants.is_empty() {
        out.push_str("\n## Participantes\n\n");
        for p in &m.participants {
            let name = p.name.clone().unwrap_or_else(|| p.label.clone());
            out.push_str(&format!("- {} ({}%)\n", name, p.talk_pct));
        }
    }
    if let Some(s) = &m.summary {
        out.push_str("\n## Resumen\n\n");
        out.push_str(&format!("{}\n", bi(s, opts)));
    }
    if !m.decisions.is_empty() {
        out.push_str("\n## Decisiones\n\n");
        for d in &m.decisions {
            out.push_str(&format!("- {}\n", bi(&d.text, opts)));
        }
    }
    if !m.blockers.is_empty() {
        out.push_str("\n## Bloqueos\n\n");
        for b in &m.blockers {
            out.push_str(&format!("- {}\n", bi(&b.text, opts)));
        }
    }
    if !m.actions.is_empty() {
        out.push_str("\n## Acciones\n\n");
        for a in &m.actions {
            let check = if a.done { "x" } else { " " };
            let mut line = format!("- [{}] {}", check, bi(&a.text, opts));
            if let Some(due) = &a.due {
                line.push_str(&format!(" _(vence: {due})_"));
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    out.push_str("\n## Transcripción\n\n");
    for line in &m.transcript {
        let ts = if opts.timestamps {
            format!("`[{}]` ", line.t)
        } else {
            String::new()
        };
        let who = speaker_name(&m.participants, &line.speaker_id);
        out.push_str(&format!("{ts}**{who}:** {}\n\n", bi(&line.text, opts)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_noter_core::models::{Action, Decision, MeetingDetail, Participant, TranscriptLine};
    use smart_noter_core::Bilingual;

    fn fixture() -> MeetingDetail {
        MeetingDetail {
            id: "m1".into(),
            title: Bilingual {
                es: "Reunión técnica".into(),
                en: Some("Technical meeting".into()),
            },
            template: "tecnica".into(),
            date: "2026-06-20T15:00:00Z".into(),
            duration_sec: 95,
            device_used: None,
            word_count: 3,
            summary: Some(Bilingual {
                es: "Resumen es".into(),
                en: Some("Summary en".into()),
            }),
            participants: vec![Participant {
                id: "p1".into(),
                meeting_id: "m1".into(),
                label: "S1".into(),
                name: Some("Ana".into()),
                color_class: "c1".into(),
                word_count: 3,
                talk_pct: 100,
            }],
            actions: vec![],
            decisions: vec![Decision {
                id: 1,
                text: Bilingual {
                    es: "Decidir X".into(),
                    en: None,
                },
            }],
            blockers: vec![],
            transcript: vec![TranscriptLine {
                id: 1,
                t: "00:00".into(),
                speaker_id: "p1".into(),
                text: Bilingual {
                    es: "hola equipo".into(),
                    en: Some("hi team".into()),
                },
            }],
        }
    }

    #[test]
    fn has_core_sections() {
        let md = to_markdown(
            &fixture(),
            &ExportOpts {
                timestamps: true,
                bilingual: false,
            },
        );
        assert!(md.starts_with("# Reunión técnica"), "title heading");
        assert!(md.contains("## Participantes"));
        assert!(md.contains("Ana"));
        assert!(md.contains("## Resumen"));
        assert!(md.contains("Resumen es"));
        assert!(md.contains("## Decisiones"));
        assert!(md.contains("Decidir X"));
        assert!(md.contains("## Transcripción"));
        assert!(md.contains("hola equipo"));
    }

    #[test]
    fn timestamps_toggle() {
        let on = to_markdown(
            &fixture(),
            &ExportOpts {
                timestamps: true,
                bilingual: false,
            },
        );
        assert!(on.contains("[00:00]"), "timestamp present when on");
        let off = to_markdown(
            &fixture(),
            &ExportOpts {
                timestamps: false,
                bilingual: false,
            },
        );
        assert!(!off.contains("[00:00]"), "timestamp absent when off");
    }

    #[test]
    fn bilingual_emits_en_alongside_es() {
        let md = to_markdown(
            &fixture(),
            &ExportOpts {
                timestamps: false,
                bilingual: true,
            },
        );
        assert!(md.contains("hola equipo"), "es text");
        assert!(md.contains("hi team"), "en text when bilingual");
    }

    #[test]
    fn empty_sections_are_skipped() {
        let mut m = fixture();
        m.decisions.clear();
        let md = to_markdown(
            &m,
            &ExportOpts {
                timestamps: false,
                bilingual: false,
            },
        );
        assert!(
            !md.contains("## Decisiones"),
            "no Decisiones heading when none"
        );
    }

    #[test]
    fn action_renders_checkbox_and_due() {
        let mut m = fixture();
        m.actions.push(Action {
            id: "a1".into(),
            meeting_id: "m1".into(),
            text: Bilingual {
                es: "Enviar reporte".into(),
                en: None,
            },
            owner_participant_id: Some("p1".into()),
            due: Some("2026-07-01".into()),
            done: true,
        });
        let md = to_markdown(
            &m,
            &ExportOpts {
                timestamps: false,
                bilingual: false,
            },
        );
        assert!(md.contains("## Acciones"), "Acciones heading when present");
        assert!(md.contains("[x]"), "checked checkbox when done");
        assert!(md.contains("Enviar reporte"), "action text");
        assert!(md.contains("vence: 2026-07-01"), "due date suffix");
    }

    #[test]
    fn duration_over_an_hour_uses_hms() {
        let mut m = fixture();
        m.duration_sec = 3661;
        let long = to_markdown(
            &m,
            &ExportOpts {
                timestamps: false,
                bilingual: false,
            },
        );
        assert!(long.contains("1:01:01"), "H:MM:SS for durations >= 1h");

        // Short meetings keep the MM:SS form (fixture is 95s = 01:35).
        let short = to_markdown(
            &fixture(),
            &ExportOpts {
                timestamps: false,
                bilingual: false,
            },
        );
        assert!(short.contains("01:35"), "MM:SS for durations < 1h");
    }
}
