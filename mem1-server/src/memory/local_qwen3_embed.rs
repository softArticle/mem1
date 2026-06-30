//! In-process Qwen3 text embedding via candle (pure Rust, no ONNX/ort).
//!
//! tract cannot run Qwen3-Embedding (decoder-only: RoPE, GQA, KV-cache — tract
//! targets encoder/CNN graphs). candle-transformers ships a native `qwen3`
//! implementation, so we load the safetensors directly and run the forward pass
//! in-process, matching local_embed.rs's embedded-inference philosophy but for a
//! multilingual SOTA embedding model (1024-dim, 100+ languages).
//!
//! Pooling: last-token (Qwen3-Embedding's training convention) + L2 normalize.
//!
//! Note on KV cache: candle's base `qwen3::Model` accumulates a KV cache across
//! forward() calls and does not expose `clear_kv_cache` publicly. Each embedding
//! is an independent single sequence, so we rebuild the `Model` per call — the
//! weights are mmaped via the shared VarBuilder, so only the lightweight layer
//! structs are reconstructed, not the tensor data.

use std::path::Path;
use std::sync::Mutex;

use candle_core::{DType, Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::qwen3::{Config, Model};
use tokenizers::Tokenizer;

use crate::error::Error;

const CONFIG_FILENAME: &str = "config.json";
const TOKENIZER_FILENAME: &str = "tokenizer.json";
const WEIGHTS_FILENAME: &str = "model.safetensors";

pub struct Qwen3Embedder {
    config: Config,
    weights_path: std::path::PathBuf,
    tokenizer: Tokenizer,
    device: Device,
    /// Serialize forward passes: candle tensors aren't trivially Sync across the
    /// rebuilt-model approach, and embedding is called from spawn_blocking.
    lock: Mutex<()>,
}

impl Qwen3Embedder {
    /// Load from a directory containing config.json, tokenizer.json,
    /// model.safetensors (e.g. a clone of Qwen/Qwen3-Embedding-0.6B).
    pub fn load(model_dir: &Path) -> Result<Self, Error> {
        let config_path = model_dir.join(CONFIG_FILENAME);
        let tokenizer_path = model_dir.join(TOKENIZER_FILENAME);
        let weights_path = model_dir.join(WEIGHTS_FILENAME);
        for (p, what) in [
            (&config_path, CONFIG_FILENAME),
            (&tokenizer_path, TOKENIZER_FILENAME),
            (&weights_path, WEIGHTS_FILENAME),
        ] {
            if !p.is_file() {
                return Err(Error::InvalidInput(format!(
                    "qwen3 embed: missing {what} in {}",
                    model_dir.display()
                )));
            }
        }
        let config: Config = serde_json::from_slice(
            &std::fs::read(&config_path)
                .map_err(|e| Error::Embedding(format!("qwen3 read config: {e}")))?,
        )
        .map_err(|e| Error::Embedding(format!("qwen3 parse config: {e}")))?;
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| Error::Embedding(format!("qwen3 tokenizer: {e}")))?;
        // Validate the weights load once at startup (fail fast on a bad dir).
        let _ = Self::build_model(&config, &weights_path, &Device::Cpu)?;
        Ok(Self {
            config,
            weights_path,
            tokenizer,
            device: Device::Cpu,
            lock: Mutex::new(()),
        })
    }

    fn build_model(cfg: &Config, weights: &Path, dev: &Device) -> Result<Model, Error> {
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights], DType::F32, dev)
                .map_err(|e| Error::Embedding(format!("qwen3 varbuilder: {e}")))?
        };
        // Qwen3-Embedding safetensors store tensors at the root (embed_tokens.weight,
        // layers.N...), but candle's Model expects a "model." prefix (CausalLM layout).
        let vb = vb.rename_f(|name: &str| name.strip_prefix("model.").unwrap_or(name).to_string());
        Model::new(cfg, vb).map_err(|e| Error::Embedding(format!("qwen3 model: {e}")))
    }

    pub fn embed_sync(&self, text: &str) -> Result<Vec<f32>, Error> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(Vec::new());
        }
        let _guard = self
            .lock
            .lock()
            .map_err(|e| Error::Embedding(format!("qwen3 lock: {e}")))?;
        // Fresh model per call so the decoder KV cache never accumulates.
        let mut model = Self::build_model(&self.config, &self.weights_path, &self.device)?;
        let enc = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| Error::Embedding(format!("qwen3 encode: {e}")))?;
        let ids: Vec<u32> = enc.get_ids().to_vec();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let n = ids.len();
        let input = Tensor::new(ids.as_slice(), &self.device)
            .and_then(|t| t.reshape((1, n)))
            .map_err(|e| Error::Embedding(format!("qwen3 input: {e}")))?;
        let hidden = model
            .forward(&input, 0)
            .map_err(|e| Error::Embedding(format!("qwen3 forward: {e}")))?;
        // last-token pooling + L2 normalize
        let last = hidden
            .i((0, n - 1, ..))
            .and_then(|t| t.to_dtype(DType::F32))
            .map_err(|e| Error::Embedding(format!("qwen3 pool: {e}")))?;
        let v = last
            .to_vec1::<f32>()
            .map_err(|e| Error::Embedding(format!("qwen3 vec: {e}")))?;
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm == 0.0 {
            return Ok(v);
        }
        Ok(v.iter().map(|x| x / norm).collect())
    }
}
