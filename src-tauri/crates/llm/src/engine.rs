//! LocalLlm: load a GGUF model and run text generation + embeddings via llama-cpp-2 0.1.150.
//!
//! API used (verified against crate source at ~/.cargo/registry/src/.../llama-cpp-2-0.1.150/):
//!   - LlamaBackend::init()
//!   - LlamaModel::load_from_file(&backend, path, &LlamaModelParams)
//!   - LlamaModelParams::default().with_n_gpu_layers(n)
//!   - model.new_context(&backend, LlamaContextParams)
//!   - LlamaContextParams::default().with_embeddings(true).with_n_ctx(...)
//!   - model.str_to_token(str, AddBos::Always / AddBos::Never)
//!   - LlamaBatch::new(capacity, n_seq_max) + batch.add(token, pos, seq_ids, logits)
//!     → batch.add returns Err(InsufficientSpace) if tokens exceed allocated capacity
//!     → batch.clear() resets n_tokens to 0 without freeing memory (safe to reuse)
//!   - ctx.decode(&mut batch)  /  ctx.encode(&mut batch)
//!   - ctx.clear_kv_cache()  — clears all KV cache entries (context/kv_cache.rs)
//!   - LlamaSampler::chain_simple([...]) + sampler.sample(&ctx, last_token_idx)
//!   - model.token_to_piece(token, &mut decoder, special, lstrip)
//!   - model.is_eog_token(token)
//!   - ctx.embeddings_seq_ith(i32) -> Result<&[f32], EmbeddingsError>

use crate::AiError;
use llama_cpp_2::{
    context::params::{LlamaContextParams, LlamaPoolingType},
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, AddBos, LlamaModel},
    sampling::LlamaSampler,
};
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

// LlamaBackend and LlamaModel are Send (the crate marks them unsafe impl Send).
// We wrap them in a struct. Because the backend must outlive the model we keep
// both together.  LlamaContext has a lifetime tied to LlamaModel so we cannot
// store it; instead we create a fresh context per call (cheap for CPU inference).
pub struct LocalLlm {
    backend: LlamaBackend,
    model: LlamaModel,
}

// SAFETY: LlamaModel + LlamaBackend are both marked Send by the crate.
unsafe impl Send for LocalLlm {}
unsafe impl Sync for LocalLlm {}

impl LocalLlm {
    /// Load a GGUF model from `path`. `n_gpu_layers = 0` forces CPU-only inference.
    pub fn load(path: &Path, n_gpu_layers: u32) -> Result<Self, AiError> {
        // The llama backend can only be initialized once per process. A second
        // LocalLlm::load() therefore fails with BackendAlreadyInitialized — surface
        // that clearly. Task 7 owns a single LocalLlm in a process-global OnceLock,
        // so in practice load() runs exactly once.
        let backend = LlamaBackend::init().map_err(|e| match e {
            llama_cpp_2::LlamaCppError::BackendAlreadyInitialized => AiError::Load(
                "LlamaBackend already initialized — only one LocalLlm may exist per \
                 process (Task 7 owns the singleton)"
                    .into(),
            ),
            other => AiError::Load(other.to_string()),
        })?;

        let model_params = LlamaModelParams::default().with_n_gpu_layers(n_gpu_layers);

        let model = LlamaModel::load_from_file(&backend, path, &model_params)
            .map_err(|e| AiError::Load(e.to_string()))?;

        Ok(Self { backend, model })
    }

    /// Generate up to `max_tokens` tokens from `prompt`.
    /// `on_token` is called for each decoded token piece (may be empty for special tokens).
    /// `abort` is checked between tokens; if set, generation stops early.
    /// Returns the full generated text (concatenation of all pieces).
    pub fn generate(
        &self,
        prompt: &str,
        max_tokens: usize,
        on_token: &mut dyn FnMut(&str),
        abort: &AtomicBool,
    ) -> Result<String, AiError> {
        // Create a generation context (no embeddings needed).
        let ctx_size = NonZeroU32::new(4096);
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(ctx_size)
            .with_n_batch(512);

        let mut ctx = self
            .model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| AiError::Inference(e.to_string()))?;

        // Tokenize without BOS: the ChatML template already opens with <|im_start|>
        // which is the true start of the Qwen2.5 prompt; a spurious BOS token before
        // it would confuse the model's attention over the turn boundaries.
        let prompt_tokens = self
            .model
            .str_to_token(prompt, AddBos::Never)
            .map_err(|e| AiError::Inference(e.to_string()))?;

        if prompt_tokens.is_empty() {
            return Ok(String::new());
        }

        // Truncate: ensure prompt + generation fit within n_ctx (4096).
        // We keep the FIRST tokens (instruction + start of transcript); dropping the tail
        // of a very long transcript is preferable to crashing with GGML_ASSERT.
        let max_prompt = 4096usize.saturating_sub(max_tokens).max(1);
        let prompt_tokens = if prompt_tokens.len() > max_prompt {
            tracing::warn!(
                n_prompt = prompt_tokens.len(),
                max = max_prompt,
                "prompt truncated to fit context; transcript tail dropped"
            );
            prompt_tokens[..max_prompt].to_vec()
        } else {
            prompt_tokens
        };
        let n_prompt = prompt_tokens.len();

        // Feed the prompt in n_batch (512) chunks so we never exceed the batch limit
        // that llama.cpp enforces: n_tokens_all <= cparams.n_batch.
        // Only the LAST token of the full prompt needs its logits flag set.
        const N_BATCH: usize = 512;
        let mut batch = LlamaBatch::new(N_BATCH, 1);
        let mut fed = 0usize;
        while fed < n_prompt {
            let end = (fed + N_BATCH).min(n_prompt);
            batch.clear();
            for (chunk_idx, &tok) in prompt_tokens[fed..end].iter().enumerate() {
                let abs_pos = fed + chunk_idx;
                let is_last_of_prompt = abs_pos == n_prompt - 1;
                batch
                    .add(tok, abs_pos as i32, &[0], is_last_of_prompt)
                    .map_err(|e| AiError::Inference(e.to_string()))?;
            }
            ctx.decode(&mut batch)
                .map_err(|e| AiError::Inference(e.to_string()))?;
            fed = end;
        }

        // Sampler: temperature 0.8 → greedy for deterministic-ish but not boring output.
        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::temp(0.8),
            LlamaSampler::top_p(0.95, 1),
            LlamaSampler::greedy(),
        ]);

        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut output = String::new();
        const IM_END: &str = "<|im_end|>";

        // n_cur tracks the KV-cache position; starts right after the prompt.
        let mut n_cur = n_prompt as i32;
        let n_max = n_cur + max_tokens as i32;

        while n_cur < n_max {
            if abort.load(Ordering::Relaxed) {
                break;
            }

            // Sample from the last available logit. llama.cpp resolves -1 to the
            // final decoded token's logits — correct for BOTH the initial multi-token
            // prompt batch AND the single-token loop batches below (after batch.clear()
            // the batch holds one token at offset 0, so n_cur-1 would index OOB).
            let new_token = sampler.sample(&ctx, -1);

            // Stop at end-of-generation.
            if self.model.is_eog_token(new_token) {
                break;
            }

            // Decode token to string piece.
            let piece = self
                .model
                .token_to_piece(new_token, &mut decoder, true, None)
                .unwrap_or_default();

            on_token(&piece);
            output.push_str(&piece);

            // Defensive stop: if the model emits <|im_end|> as text rather than
            // triggering is_eog_token (shouldn't happen with Qwen2.5 but guards
            // against model variants that don't mark it as EOG), strip it and stop.
            if output.ends_with(IM_END) {
                output.truncate(output.len() - IM_END.len());
                break;
            }

            // Feed the new token back into the context.
            batch.clear();
            batch
                .add(new_token, n_cur, &[0], true)
                .map_err(|e| AiError::Inference(e.to_string()))?;

            ctx.decode(&mut batch)
                .map_err(|e| AiError::Inference(e.to_string()))?;

            // Accept the token into the sampler (updates repetition penalty state, etc.).
            sampler.accept(new_token);

            n_cur += 1;
        }

        Ok(output)
    }

    /// Embed each text using the last-token hidden state of a decoder context.
    /// Returns one `Vec<f32>` per input text, all of the same dimension (`n_embd`).
    ///
    /// Implementation note: Qwen2.5-Instruct is a CAUSAL (decoder) language model.
    /// Using `encode()` (which forces a bidirectional/encoder graph) crashes with
    /// STATUS_ACCESS_VIOLATION because the GGML compute graph for an encoder does not
    /// match the weight tensors of a decoder-only architecture.
    ///
    /// The correct approach for causal LMs:
    ///   1. Create a context with `embeddings=true` and `pooling_type=None` (per-token).
    ///   2. Run `decode()` (decoder graph, causal attention — correct for Qwen2.5).
    ///   3. Extract the last-token hidden state via `embeddings_ith(n_tokens - 1)`.
    ///
    /// The last token attends to the full left context, so its hidden state is a
    /// compact sequence representation suitable for RAG similarity search.
    pub fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AiError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Truncate each text to at most 480 tokens — leaves a safe margin under n_ctx=512.
        const MAX_EMBED_TOKENS: usize = 480;
        let ctx_size = NonZeroU32::new(512);

        let mut results: Vec<Vec<f32>> = Vec::with_capacity(texts.len());

        for text in texts.iter() {
            // Tokenize without BOS.
            let tokens = self
                .model
                .str_to_token(text, AddBos::Never)
                .map_err(|e| AiError::Inference(e.to_string()))?;

            if tokens.is_empty() {
                // Return a zero vector of the correct dimension for empty input.
                let n_embd = self.model.n_embd() as usize;
                results.push(vec![0.0f32; n_embd]);
                continue;
            }

            // Truncate to stay within the context window.
            let tokens = if tokens.len() > MAX_EMBED_TOKENS {
                tokens[..MAX_EMBED_TOKENS].to_vec()
            } else {
                tokens
            };
            let n_tokens = tokens.len();

            // Fresh decode context per text with per-token (None) pooling.
            // A per-text context avoids KV-cache reuse issues between calls.
            // Context creation is cheap (~5ms) relative to model load.
            let ctx_params = LlamaContextParams::default()
                .with_embeddings(true)
                .with_pooling_type(LlamaPoolingType::None)
                .with_n_ctx(ctx_size)
                .with_n_batch(512);
            let mut ctx = self
                .model
                .new_context(&self.backend, ctx_params)
                .map_err(|e| AiError::Inference(e.to_string()))?;

            // Build a batch for this single sequence (seq_id 0).
            // Only the LAST token needs logits=true — that is the embedding we extract.
            let mut batch = LlamaBatch::new(n_tokens, 1);
            for (i, &tok) in tokens.iter().enumerate() {
                let is_last = i == n_tokens - 1;
                batch
                    .add(tok, i as i32, &[0], is_last)
                    .map_err(|e| AiError::Inference(e.to_string()))?;
            }

            // decode() runs the causal decoder graph — correct for Qwen2.5.
            ctx.decode(&mut batch)
                .map_err(|e| AiError::Inference(e.to_string()))?;

            // Retrieve the per-token embedding of the LAST token (index = n_tokens - 1).
            // With pooling_type=None + embeddings=true, llama.cpp overrides ALL tokens
            // as outputs. embeddings_ith(i) indexes by token position, so we read the
            // last token's hidden state, which attends to the full left context — correct
            // last-token pooling for a causal LM.
            let emb_slice = ctx
                .embeddings_ith((n_tokens - 1) as i32)
                .map_err(|e| AiError::Inference(e.to_string()))?;

            // L2-normalize so cosine similarity equals dot product.
            let norm = l2_norm(emb_slice);
            let normalized: Vec<f32> = emb_slice.iter().map(|&v| v / norm.max(1e-12)).collect();
            results.push(normalized);
        }

        Ok(results)
    }
}

/// Compute the L2 norm of a float slice.
fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|&x| x * x).sum::<f32>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l2_norm_zero_vector() {
        let v = vec![0.0f32; 4];
        let n = l2_norm(&v);
        assert!((n - 0.0).abs() < 1e-9);
    }

    #[test]
    fn l2_norm_unit_vector() {
        let v = vec![1.0f32, 0.0, 0.0, 0.0];
        let n = l2_norm(&v);
        assert!((n - 1.0).abs() < 1e-6);
    }

    #[test]
    fn l2_norm_known_value() {
        // [3, 4] → norm = 5
        let v = vec![3.0f32, 4.0];
        let n = l2_norm(&v);
        assert!((n - 5.0).abs() < 1e-5, "expected 5.0, got {n}");
    }

    /// Full smoke test: requires a downloaded GGUF model.
    /// Set LLM_GGUF env var to the path of a text-gen GGUF file.
    #[test]
    #[ignore = "requires a downloaded GGUF model; run manually"]
    fn generate_and_embed_smoke() {
        let path = std::env::var("LLM_GGUF").expect("LLM_GGUF must be set");
        let m = LocalLlm::load(std::path::Path::new(&path), 0).unwrap();

        let mut toks = 0usize;
        let out = m
            .generate(
                "Responde solo: hola",
                16,
                &mut |_| toks += 1,
                &AtomicBool::new(false),
            )
            .unwrap();
        assert!(!out.is_empty(), "generate should return non-empty text");
        assert!(toks > 0, "on_token should have been called at least once");

        let e = m.embed(&["hola".into(), "adiós".into()]).unwrap();
        assert_eq!(e.len(), 2, "embed should return one vector per input");
        assert_eq!(
            e[0].len(),
            e[1].len(),
            "all embedding vectors must have the same dimension"
        );
        assert!(!e[0].is_empty(), "embedding dimension must be > 0");

        // Sanity: embeddings of different words must not be identical vectors.
        // Cosine similarity < 0.9999 or simply check at least one coordinate differs.
        let identical = e[0]
            .iter()
            .zip(e[1].iter())
            .all(|(a, b)| (a - b).abs() < 1e-6);
        assert!(
            !identical,
            "embeddings for 'hola' and 'adiós' must not be identical vectors"
        );
    }
}
