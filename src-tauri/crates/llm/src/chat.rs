use crate::engine::LocalLlm;
use smart_noter_core::models::ai::Chunk;
use smart_noter_core::traits::ChatEngine;
use std::sync::atomic::AtomicBool;

/// RAG-based chat engine backed by a `LocalLlm` instance.
///
/// `embed` delegates directly to `LocalLlm::embed`.
/// `answer` builds a context string from the top-k retrieved chunks, formats a
/// prompt, and streams tokens through `on_token` using `LocalLlm::generate`.
pub struct LocalChat<'a> {
    pub llm: &'a LocalLlm,
}

impl ChatEngine for LocalChat<'_> {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        self.llm.embed(texts).map_err(|e| e.to_string())
    }

    fn answer(
        &self,
        question: &str,
        context: &[Chunk],
        lang: &str,
        on_token: &mut dyn FnMut(&str),
        abort: &AtomicBool,
    ) -> Result<(), String> {
        let ctx = context
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("\n---\n");

        let prompt = format!(
            "Responde en {lang} usando SOLO el contexto de la reunión. \
Si no está en el contexto, dilo.\n\nContexto:\n{ctx}\n\nPregunta: {question}\nRespuesta:"
        );

        self.llm
            .generate(&prompt, 512, on_token, abort)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[allow(dead_code)]
fn _assert_send_sync()
where
    LocalChat<'static>: Send + Sync,
{
}

/// Split transcript lines into overlapping-window text chunks.
/// Each chunk contains `per_chunk` lines (last chunk may have fewer).
pub fn chunk_transcript(lines: &[(String, String)], per_chunk: usize) -> Vec<String> {
    lines
        .chunks(per_chunk.max(1))
        .map(|w| {
            w.iter()
                .map(|(s, t)| format!("{s}: {t}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect()
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// Return the top-k chunks ordered by cosine similarity to `query`.
pub fn top_k<'a>(query: &[f32], chunks: &'a [Chunk], k: usize) -> Vec<&'a Chunk> {
    let mut scored: Vec<_> = chunks
        .iter()
        .map(|c| (cosine(query, &c.vector), c))
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.into_iter().take(k).map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_by_window() {
        let lines: Vec<(String, String)> = (0..10)
            .map(|i| ("S1".into(), format!("línea {i}")))
            .collect();
        let chunks = chunk_transcript(&lines, 3); // 3 lines per chunk
        assert_eq!(chunks.len(), 4); // 3+3+3+1
        assert!(chunks[0].contains("línea 0") && chunks[0].contains("línea 2"));
    }

    #[test]
    fn cosine_top_k_orders_by_similarity() {
        let q = vec![1.0, 0.0];
        let chunks = vec![
            Chunk {
                idx: 0,
                text: "a".into(),
                vector: vec![0.0, 1.0],
            }, // orthogonal
            Chunk {
                idx: 1,
                text: "b".into(),
                vector: vec![1.0, 0.0],
            }, // identical
        ];
        let top = top_k(&q, &chunks, 1);
        assert_eq!(top[0].idx, 1);
    }

    #[test]
    fn top_k_larger_than_chunks_returns_all() {
        let q = vec![1.0, 0.0];
        let chunks = vec![
            Chunk {
                idx: 0,
                text: "a".into(),
                vector: vec![1.0, 0.0],
            },
            Chunk {
                idx: 1,
                text: "b".into(),
                vector: vec![0.0, 1.0],
            },
        ];
        // k=10 but only 2 chunks — must not panic and return all 2
        let top = top_k(&q, &chunks, 10);
        assert_eq!(top.len(), 2);
    }
}
