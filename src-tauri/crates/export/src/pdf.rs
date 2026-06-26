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
            doc.push(elements::Paragraph::new(format!(
                "{mark} {}",
                bi(&a.text, opts)
            )));
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
        let who = m
            .participants
            .iter()
            .find(|p| p.id == line.speaker_id)
            .map(|p| p.name.clone().unwrap_or_else(|| p.label.clone()))
            .unwrap_or_else(|| "—".into());
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
    use smart_noter_core::models::{MeetingDetail, TranscriptLine};
    use smart_noter_core::Bilingual;

    fn fixture() -> MeetingDetail {
        MeetingDetail {
            id: "m1".into(),
            title: Bilingual {
                es: "Reunión".into(),
                en: None,
            },
            template: "tecnica".into(),
            date: "2026-06-20T15:00:00Z".into(),
            duration_sec: 60,
            device_used: None,
            word_count: 1,
            summary: None,
            participants: vec![],
            actions: vec![],
            decisions: vec![],
            blockers: vec![],
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
