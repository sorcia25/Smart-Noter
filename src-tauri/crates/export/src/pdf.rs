use crate::{bi, ExportError, ExportOpts};
use genpdf::fonts::{FontData, FontFamily};
use genpdf::style::Style;
use genpdf::{elements, Document, Element, SimplePageDecorator};
use smart_noter_core::models::MeetingDetail;

fn embedded_font_family() -> Result<FontFamily<FontData>, ExportError> {
    let load = |bytes: &[u8]| {
        FontData::new(bytes.to_vec(), None).map_err(|e| ExportError::Pdf(format!("font: {e}")))
    };
    Ok(FontFamily {
        regular: load(include_bytes!("../fonts/LiberationSans-Regular.ttf"))?,
        bold: load(include_bytes!("../fonts/LiberationSans-Bold.ttf"))?,
        italic: load(include_bytes!("../fonts/LiberationSans-Italic.ttf"))?,
        bold_italic: load(include_bytes!("../fonts/LiberationSans-BoldItalic.ttf"))?,
    })
}

fn heading(text: &str, size: u8) -> impl Element {
    elements::Paragraph::new(text).styled(Style::new().bold().with_font_size(size))
}

// genpdf with the embedded Liberation Sans family renders full UTF-8
// (Spanish accents, ñ, ¿, ¡, •) correctly — verified via pdftotext. Do NOT
// strip non-ASCII from headings or content.
pub fn to_pdf(m: &MeetingDetail, opts: &ExportOpts) -> Result<Vec<u8>, ExportError> {
    let mut doc = Document::new(embedded_font_family()?);
    doc.set_title(m.title.es.clone());

    let mut decorator = SimplePageDecorator::new();
    decorator.set_margins(10);
    doc.set_page_decorator(decorator);

    doc.push(heading(&bi(&m.title, opts), 18));
    doc.push(elements::Paragraph::new(format!("Fecha: {}", m.date)));
    doc.push(elements::Paragraph::new(format!(
        "Duración: {}",
        crate::fmt_duration(m.duration_sec)
    )));
    doc.push(elements::Break::new(1));

    if !m.participants.is_empty() {
        doc.push(heading("Participantes", 14));
        for p in &m.participants {
            let name = p.name.clone().unwrap_or_else(|| p.label.clone());
            doc.push(elements::Paragraph::new(format!(
                "• {} ({}%)",
                name, p.talk_pct
            )));
        }
        doc.push(elements::Break::new(1));
    }
    if let Some(s) = &m.summary {
        doc.push(heading("Resumen", 14));
        doc.push(elements::Paragraph::new(bi(s, opts)));
        doc.push(elements::Break::new(1));
    }
    if !m.decisions.is_empty() {
        doc.push(heading("Decisiones", 14));
        for d in &m.decisions {
            doc.push(elements::Paragraph::new(format!("• {}", bi(&d.text, opts))));
        }
        doc.push(elements::Break::new(1));
    }
    if !m.blockers.is_empty() {
        doc.push(heading("Bloqueos", 14));
        for b in &m.blockers {
            doc.push(elements::Paragraph::new(format!("• {}", bi(&b.text, opts))));
        }
        doc.push(elements::Break::new(1));
    }
    if !m.actions.is_empty() {
        doc.push(heading("Acciones", 14));
        for a in &m.actions {
            let mark = if a.done { "[x]" } else { "[ ]" };
            let mut line = format!("{mark} {}", bi(&a.text, opts));
            if let Some(due) = &a.due {
                line.push_str(&format!(" (vence: {due})"));
            }
            doc.push(elements::Paragraph::new(line));
        }
        doc.push(elements::Break::new(1));
    }

    doc.push(heading("Transcripción", 14));
    for line in &m.transcript {
        let ts = if opts.timestamps {
            format!("[{}] ", line.t)
        } else {
            String::new()
        };
        let who = crate::speaker_name(&m.participants, &line.speaker_id);
        doc.push(elements::Paragraph::new(format!(
            "{ts}{who}: {}",
            bi(&line.text, opts)
        )));
    }

    let mut buf = Vec::new();
    doc.render(&mut buf)
        .map_err(|e| ExportError::Pdf(e.to_string()))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExportOpts;
    use smart_noter_core::models::{
        Action, Blocker, Decision, MeetingDetail, Participant, TranscriptLine,
    };
    use smart_noter_core::Bilingual;

    /// A rich fixture that exercises every section's code path: a named
    /// participant, a summary, a decision, a blocker, and a done action with a
    /// due date. This ensures `to_pdf` renders all branches without panicking
    /// on real data (an empty fixture skipped most sections).
    fn fixture() -> MeetingDetail {
        MeetingDetail {
            id: "m1".into(),
            title: Bilingual {
                es: "Reunión".into(),
                en: None,
            },
            template: "tecnica".into(),
            date: "2026-06-20T15:00:00Z".into(),
            duration_sec: 95,
            device_used: None,
            word_count: 1,
            summary: Some(Bilingual {
                es: "Resumen de la reunión".into(),
                en: None,
            }),
            participants: vec![Participant {
                id: "p1".into(),
                meeting_id: "m1".into(),
                label: "S1".into(),
                name: Some("Ana".into()),
                color_class: "c1".into(),
                word_count: 1,
                talk_pct: 100,
            }],
            actions: vec![Action {
                id: "a1".into(),
                meeting_id: "m1".into(),
                text: Bilingual {
                    es: "Enviar reporte".into(),
                    en: None,
                },
                owner_participant_id: Some("p1".into()),
                due: Some("2026-07-01".into()),
                done: true,
            }],
            decisions: vec![Decision {
                id: 1,
                text: Bilingual {
                    es: "Decidir X".into(),
                    en: None,
                },
            }],
            blockers: vec![Blocker {
                id: 1,
                text: Bilingual {
                    es: "Falta acceso".into(),
                    en: None,
                },
            }],
            transcript: vec![TranscriptLine {
                id: 1,
                t: "00:00".into(),
                speaker_id: "p1".into(),
                text: Bilingual {
                    es: "contenido".into(),
                    en: None,
                },
            }],
        }
    }

    #[test]
    fn renders_nonempty_pdf() {
        let bytes = to_pdf(
            &fixture(),
            &ExportOpts {
                timestamps: true,
                bilingual: false,
            },
        )
        .unwrap();
        assert!(
            bytes.len() > 1000,
            "pdf should have real content, got {}",
            bytes.len()
        );
        assert_eq!(&bytes[0..5], b"%PDF-", "starts with the PDF magic header");
    }
}
