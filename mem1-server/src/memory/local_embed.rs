//! In-process ONNX embedding: load model + tokenizer from a directory, run inference in spawn_blocking.
//! Uses tract (pure Rust) for ONNX inference; no native libs, works on macOS.

use crate::error::Error;
use ndarray::Array1;
use std::path::Path;
use std::sync::Arc;
use tract_onnx::prelude::*;
use tokenizers::tokenizer::Tokenizer;
use tokenizers::{
    pad_encodings, truncate_encodings, PaddingDirection, PaddingParams, PaddingStrategy,
    TruncationDirection, TruncationParams, TruncationStrategy,
};

const DEFAULT_MAX_LENGTH: usize = 256;
const MODEL_FILENAME: &str = "model.onnx";
const TOKENIZER_FILENAME: &str = "tokenizer.json";

/// Input/output names expected by all-MiniLM-L6-v2-style ONNX models (optimum export). Tract uses input order.
#[allow(dead_code)]
const INPUT_IDS_NAME: &str = "input_ids";
#[allow(dead_code)]
const ATTENTION_MASK_NAME: &str = "attention_mask";
#[allow(dead_code)]
const LAST_HIDDEN_STATE_NAME: &str = "last_hidden_state";

pub struct LocalEmbedder {
    /// Runnable model; we run it under this mutex (tract runnable is not Sync by default in some setups).
    model: Arc<std::sync::Mutex<RunnableModel<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>>>,
    tokenizer: Tokenizer,
    max_length: usize,
    /// Number of model inputs (2 = input_ids, attention_mask; 3 = + token_type_ids). Some ONNX exports need 3.
    num_inputs: usize,
}

impl LocalEmbedder {
    /// Load ONNX model (via tract) and tokenizer from `model_dir`. Expects `model.onnx` and `tokenizer.json`.
    pub fn load(model_dir: &Path, max_length: Option<usize>) -> Result<Self, Error> {
        let model_path = model_dir.join(MODEL_FILENAME);
        let tokenizer_path = model_dir.join(TOKENIZER_FILENAME);

        if !model_path.is_file() {
            return Err(Error::InvalidInput(format!(
                "local embed: missing {} in {}",
                MODEL_FILENAME,
                model_dir.display()
            )));
        }
        if !tokenizer_path.is_file() {
            return Err(Error::InvalidInput(format!(
                "local embed: missing {} in {}",
                TOKENIZER_FILENAME,
                model_dir.display()
            )));
        }

        let max_length = max_length.unwrap_or(DEFAULT_MAX_LENGTH);

        let decl = tract_onnx::onnx()
            .model_for_path(&model_path)
            .map_err(|e| Error::Embedding(format!("tract load {}: {e}", model_path.display())))?;

        // Some ONNX exports (e.g. onnx-models) need 3 inputs (input_ids, attention_mask, token_type_ids). Try 3 first.
        let (model, num_inputs) = {
            let with_three = decl
                .clone()
                .with_input_fact(
                    0,
                    InferenceFact::dt_shape(i64::datum_type(), tvec!(1, max_length)),
                )
                .map_err(|e| Error::Embedding(format!("tract input_ids fact: {e}")))?
                .with_input_fact(
                    1,
                    InferenceFact::dt_shape(i64::datum_type(), tvec!(1, max_length)),
                )
                .map_err(|e| Error::Embedding(format!("tract attention_mask fact: {e}")))?
                .with_input_fact(
                    2,
                    InferenceFact::dt_shape(i64::datum_type(), tvec!(1, max_length)),
                )
                .map_err(|e| Error::Embedding(format!("tract token_type_ids fact: {e}")))?
                .into_optimized()
                .and_then(|m| m.into_runnable());

            match with_three {
                Ok(m) => (m, 3),
                Err(_) => {
                    let with_two = decl
                        .with_input_fact(
                            0,
                            InferenceFact::dt_shape(i64::datum_type(), tvec!(1, max_length)),
                        )
                        .map_err(|e| Error::Embedding(format!("tract input_ids fact: {e}")))?
                        .with_input_fact(
                            1,
                            InferenceFact::dt_shape(i64::datum_type(), tvec!(1, max_length)),
                        )
                        .map_err(|e| Error::Embedding(format!("tract attention_mask fact: {e}")))?
                        .into_optimized()
                        .map_err(|e| Error::Embedding(format!("tract optimize: {e}")))?
                        .into_runnable()
                        .map_err(|e| Error::Embedding(format!("tract runnable: {e}")))?;
                    (with_two, 2)
                }
            }
        };

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| Error::Embedding(format!("tokenizer load: {e}")))?;

        Ok(Self {
            model: Arc::new(std::sync::Mutex::new(model)),
            tokenizer,
            max_length,
            num_inputs,
        })
    }

    /// Run embedding for one text. Call from sync context (e.g. inside spawn_blocking).
    pub fn embed_sync(&self, text: &str) -> Result<Vec<f32>, Error> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(vec![]);
        }

        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| Error::Embedding(format!("tokenize: {e}")))?;

        let trunc_params = TruncationParams {
            max_length: self.max_length,
            strategy: TruncationStrategy::LongestFirst,
            stride: 0,
            direction: TruncationDirection::Right,
        };
        let (encoding, _) = truncate_encodings(encoding, None, &trunc_params)
            .map_err(|e| Error::Embedding(format!("truncate: {e}")))?;

        let mut encodings = vec![encoding];
        let pad_params = PaddingParams {
            strategy: PaddingStrategy::Fixed(self.max_length),
            direction: PaddingDirection::Right,
            pad_to_multiple_of: None,
            pad_id: 0,
            pad_type_id: 0,
            pad_token: "[PAD]".to_string(),
        };
        pad_encodings(&mut encodings, &pad_params)
            .map_err(|e| Error::Embedding(format!("pad: {e}")))?;

        let encoding: &tokenizers::Encoding = &encodings[0];
        let ids: Vec<i64> = encoding.get_ids().iter().map(|&u| u as i64).collect();
        let mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&u| u as i64).collect();

        let seq_len = ids.len();

        let input_ids = tract_ndarray::Array::from_shape_vec((1, seq_len), ids)
            .map_err(|e| Error::Embedding(format!("input_ids tensor: {e}")))?
            .into_tensor();
        let attention_mask = tract_ndarray::Array::from_shape_vec((1, seq_len), mask.clone())
            .map_err(|e| Error::Embedding(format!("attention_mask tensor: {e}")))?
            .into_tensor();

        let guard = self
            .model
            .lock()
            .map_err(|e| Error::Embedding(format!("model lock: {e}")))?;

        let inputs: Vec<TValue> = if self.num_inputs >= 3 {
            let token_type_ids: Vec<i64> = (0..seq_len).map(|_| 0i64).collect();
            let tt = tract_ndarray::Array::from_shape_vec((1, seq_len), token_type_ids)
                .map_err(|e| Error::Embedding(format!("token_type_ids tensor: {e}")))?
                .into_tensor();
            vec![input_ids.into(), attention_mask.into(), tt.into()]
        } else {
            vec![input_ids.into(), attention_mask.into()]
        };

        let outputs: tract_data::internal::tract_smallvec::SmallVec<[TValue; 4]> = guard
            .run(inputs.into())
            .map_err(|e| Error::Embedding(format!("tract run: {e}")))?;

        let last_hidden = outputs
            .first()
            .ok_or_else(|| Error::Embedding("missing embedding output".to_string()))?;

        let view = last_hidden
            .to_array_view::<f32>()
            .map_err(|e| Error::Embedding(format!("last_hidden_state to f32 view: {e}")))?;

        let shape = view.shape();
        if shape.len() != 3 {
            return Err(Error::Embedding(format!(
                "last_hidden_state expected ndim 3, got {}",
                shape.len()
            )));
        }
        let batch = shape[0];
        let seq_len_out = shape[1];
        let hidden_size = shape[2];
        if batch != 1 {
            return Err(Error::Embedding("expected batch size 1".to_string()));
        }

        let mask_slice: &[i64] = &mask;
        let mask_sum: f32 = mask_slice.iter().map(|&m| m as f32).sum();
        if mask_sum <= 0.0 {
            return Err(Error::Embedding("attention_mask sum is 0".to_string()));
        }

        let mut pooled = Array1::<f32>::zeros(hidden_size);
        for d in 0..hidden_size {
            let s: f32 = (0..seq_len_out)
                .map(|s_idx| {
                    let m = mask_slice.get(s_idx).copied().unwrap_or(0) as f32;
                    let v = view[[0, s_idx, d]];
                    v * m
                })
                .sum();
            pooled[d] = s / mask_sum;
        }

        let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
        let normalized: Vec<f32> = if norm > 1e-12 {
            pooled.iter().map(|&x| x / norm).collect()
        } else {
            pooled.iter().copied().collect()
        };

        Ok(normalized)
    }
}
