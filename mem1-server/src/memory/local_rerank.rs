//! In-process cross-encoder reranker (tract, pure Rust) — same embedded-inference
//! philosophy as local_embed.rs: load an ONNX model + tokenizer from a directory
//! and run inference inside the mem1-server process, no external service.
//!
//! Model: cross-encoder/ms-marco-MiniLM-L6-v2 (BERT + classification head). Inputs
//! input_ids / attention_mask / token_type_ids (query=0, doc=1 segment); output
//! logits[batch,1] = a relevance score. We score each (query, passage) pair and
//! return passage indices sorted by descending score.

use std::path::Path;
use std::sync::Mutex;

use tokenizers::tokenizer::Tokenizer;
use tract_onnx::prelude::*;

use crate::error::Error;

const MODEL_FILENAME: &str = "model.onnx";
const TOKENIZER_FILENAME: &str = "tokenizer.json";
const DEFAULT_MAX_LENGTH: usize = 256;

type TractRunnable = RunnableModel<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

pub struct LocalCrossEncoder {
    model: Mutex<TractRunnable>,
    tokenizer: Tokenizer,
    max_length: usize,
    /// Number of model inputs: 3 = BERT (input_ids/attention_mask/token_type_ids),
    /// 2 = RoBERTa-family (no segment ids, e.g. bge-reranker-base).
    n_inputs: usize,
}

impl LocalCrossEncoder {
    /// Build from env: when MEM1_RERANK_PROVIDER=crossencoder, load the ONNX
    /// cross-encoder from MEM1_RERANK_MODEL_DIR (default ./rerank_model). Returns
    /// None when not configured or the model dir is missing, so the caller falls
    /// back to no rerank / the HTTP reranker.
    pub fn from_env() -> Option<Self> {
        if std::env::var("MEM1_RERANK_PROVIDER").unwrap_or_default() != "crossencoder" {
            return None;
        }
        let dir =
            std::env::var("MEM1_RERANK_MODEL_DIR").unwrap_or_else(|_| "rerank_model".to_string());
        match Self::load(Path::new(&dir), None) {
            Ok(ce) => Some(ce),
            Err(e) => {
                tracing::warn!(error = %e, "cross-encoder rerank: load failed, disabling");
                None
            }
        }
    }

    pub fn load(model_dir: &Path, max_length: Option<usize>) -> Result<Self, Error> {
        let model_path = model_dir.join(MODEL_FILENAME);
        let tokenizer_path = model_dir.join(TOKENIZER_FILENAME);
        if !model_path.is_file() || !tokenizer_path.is_file() {
            return Err(Error::InvalidInput(format!(
                "local rerank: missing model.onnx/tokenizer.json in {}",
                model_dir.display()
            )));
        }
        let max_length = max_length.unwrap_or(DEFAULT_MAX_LENGTH);

        // Detect input arity from the raw ONNX before fixing facts: BERT cross-encoders
        // take 3 inputs (input_ids/attention_mask/token_type_ids), RoBERTa-family (bge)
        // take 2 (no segment ids).
        let raw = tract_onnx::onnx()
            .model_for_path(&model_path)
            .map_err(|e| Error::Embedding(format!("tract rerank load: {e}")))?;
        let n_inputs = raw.input_outlets().map(|o| o.len()).unwrap_or(3);

        let mut model = raw;
        for i in 0..n_inputs {
            model = model
                .with_input_fact(
                    i,
                    InferenceFact::dt_shape(i64::datum_type(), tvec!(1, max_length)),
                )
                .map_err(|e| Error::Embedding(format!("rerank input {i} fact: {e}")))?;
        }
        let model = model
            .into_optimized()
            .map_err(|e| Error::Embedding(format!("rerank optimize: {e}")))?
            .into_runnable()
            .map_err(|e| Error::Embedding(format!("rerank runnable: {e}")))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| Error::Embedding(format!("rerank tokenizer: {e}")))?;

        Ok(Self {
            model: Mutex::new(model),
            tokenizer,
            max_length,
            n_inputs,
        })
    }

    /// Relevance score for a (query, passage) pair via the cross-encoder logit.
    pub fn score(&self, query: &str, passage: &str) -> Result<f32, Error> {
        let encoding = self
            .tokenizer
            .encode((query, passage), true)
            .map_err(|e| Error::Embedding(format!("rerank tokenize: {e}")))?;

        let take = self.max_length;
        let mut ids: Vec<i64> = encoding
            .get_ids()
            .iter()
            .take(take)
            .map(|&u| u as i64)
            .collect();
        let mut mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .take(take)
            .map(|&u| u as i64)
            .collect();
        let mut types: Vec<i64> = encoding
            .get_type_ids()
            .iter()
            .take(take)
            .map(|&u| u as i64)
            .collect();
        // Pad right to max_length (matches the fixed input fact shape).
        while ids.len() < take {
            ids.push(0);
            mask.push(0);
            types.push(0);
        }

        let seq = take;
        let input_ids = tract_ndarray::Array::from_shape_vec((1, seq), ids)
            .map_err(|e| Error::Embedding(format!("rerank input_ids tensor: {e}")))?
            .into_tensor();
        let attention_mask = tract_ndarray::Array::from_shape_vec((1, seq), mask)
            .map_err(|e| Error::Embedding(format!("rerank attn tensor: {e}")))?
            .into_tensor();

        let guard = self
            .model
            .lock()
            .map_err(|e| Error::Embedding(format!("rerank lock: {e}")))?;
        // BERT cross-encoders take 3 inputs (with token_type_ids); RoBERTa-family
        // (bge-reranker-base) take 2.
        let inputs: TVec<TValue> = if self.n_inputs >= 3 {
            let token_type_ids = tract_ndarray::Array::from_shape_vec((1, seq), types)
                .map_err(|e| Error::Embedding(format!("rerank tt tensor: {e}")))?
                .into_tensor();
            tvec!(
                input_ids.into(),
                attention_mask.into(),
                token_type_ids.into()
            )
        } else {
            tvec!(input_ids.into(), attention_mask.into())
        };
        let outputs = guard
            .run(inputs)
            .map_err(|e| Error::Embedding(format!("rerank run: {e}")))?;
        let logits = outputs
            .first()
            .ok_or_else(|| Error::Embedding("rerank missing logits".to_string()))?
            .to_array_view::<f32>()
            .map_err(|e| Error::Embedding(format!("rerank logits view: {e}")))?;
        Ok(logits.iter().copied().next().unwrap_or(0.0))
    }

    /// Return passage indices ordered by descending relevance to `query`.
    pub fn rerank(&self, query: &str, passages: &[String]) -> Vec<usize> {
        let mut scored: Vec<(usize, f32)> = passages
            .iter()
            .enumerate()
            .map(|(i, p)| (i, self.score(query, p).unwrap_or(f32::NEG_INFINITY)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(i, _)| i).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Requires rerank_model/ (model.onnx + tokenizer.json). Run explicitly:
    // cargo test --release local_rerank -- --ignored --nocapture
    #[test]
    #[ignore]
    fn cross_encoder_ranks_relevant_first() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("rerank_model");
        let ce = LocalCrossEncoder::load(&dir, None).expect("load cross-encoder");
        let q = "Where has Melanie camped?";
        let docs = vec![
            "Caroline likes tea".to_string(),
            "Melanie camped at the beach, mountains and forest".to_string(),
            "It was a sunny day".to_string(),
        ];
        let order = ce.rerank(q, &docs);
        println!("order: {:?}", order);
        for (i, d) in docs.iter().enumerate() {
            println!("  score[{}]={:.3}  {}", i, ce.score(q, d).unwrap(), d);
        }
        assert_eq!(order[0], 1, "camping doc should rank first");
    }

    // Loads bge-reranker-base (1GB XLM-RoBERTa, 2-input) to validate tract load
    // time + arity auto-detect. Run: cargo test --release bge_loads -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bge_loads_and_ranks() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("rerank_model_bge");
        let ce = LocalCrossEncoder::load(&dir, None).expect("load bge-reranker-base");
        println!("n_inputs = {}", ce.n_inputs);
        let q = "Where has Melanie camped?";
        let docs = vec![
            "Caroline likes tea".to_string(),
            "Melanie camped at the beach, mountains and forest".to_string(),
            "It was a sunny day".to_string(),
        ];
        let order = ce.rerank(q, &docs);
        println!("order: {:?}", order);
        assert_eq!(order[0], 1, "camping doc should rank first");
    }
}
