//! Download default ONNX embedding model from Hugging Face when not present locally.
//! Primary: onnx-community/all-MiniLM-L6-v2-ONNX (split onnx + onnx_data).
//! Fallback: onnx-models/all-MiniLM-L6-v2-onnx (single model.onnx, often older opset / tract-compatible).

use crate::error::Error;
use std::io::Write;
use std::path::Path;

const HF_COMMUNITY: &str = "https://huggingface.co/onnx-community/all-MiniLM-L6-v2-ONNX/resolve/main";
const HF_ONNX_MODELS: &str = "https://huggingface.co/onnx-models/all-MiniLM-L6-v2-onnx/resolve/main";

fn download_file(
    client: &reqwest::blocking::Client,
    base_url: &str,
    url_path: &str,
    dest: &Path,
) -> Result<(), Error> {
    let url = format!("{base_url}/{url_path}");
    tracing::info!(url = %url, dest = %dest.display(), "downloading");
    let resp = client
        .get(&url)
        .send()
        .map_err(|e| Error::Embedding(format!("download {url_path}: {e}")))?;
    if !resp.status().is_success() {
        return Err(Error::Embedding(format!(
            "download {url_path}: HTTP {}",
            resp.status()
        )));
    }
    let bytes = resp
        .bytes()
        .map_err(|e| Error::Embedding(format!("read body {url_path}: {e}")))?;
    let mut f = std::fs::File::create(dest)
        .map_err(|e| Error::Embedding(format!("create {}: {e}", dest.display())))?;
    f.write_all(&bytes)
        .map_err(|e| Error::Embedding(format!("write {}: {e}", dest.display())))?;
    Ok(())
}

/// Download primary default model (onnx-community: model.onnx + model.onnx_data + tokenizer.json). Idempotent.
pub fn download_default_model(model_dir: &Path) -> Result<(), Error> {
    std::fs::create_dir_all(model_dir)
        .map_err(|e| Error::InvalidInput(format!("create embed_model dir: {e}")))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| Error::Embedding(format!("reqwest client: {e}")))?;

    let files = [
        ("onnx/model.onnx", "model.onnx"),
        ("onnx/model.onnx_data", "model.onnx_data"),
        ("tokenizer.json", "tokenizer.json"),
    ];

    for (url_path, filename) in files {
        let dest = model_dir.join(filename);
        if dest.is_file() {
            tracing::debug!(path = %dest.display(), "already exists, skip download");
            continue;
        }
        download_file(&client, HF_COMMUNITY, url_path, &dest)?;
    }

    Ok(())
}

/// Download alternative model (onnx-models: single model.onnx + tokenizer.json). Overwrites existing.
/// Use when primary load fails (e.g. tract Cast op). Often uses older ONNX opset.
pub fn download_alternative_model(model_dir: &Path) -> Result<(), Error> {
    std::fs::create_dir_all(model_dir)
        .map_err(|e| Error::InvalidInput(format!("create embed_model dir: {e}")))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| Error::Embedding(format!("reqwest client: {e}")))?;

    tracing::info!("downloading alternative embed model (onnx-models, single ONNX)");
    download_file(&client, HF_ONNX_MODELS, "model.onnx", &model_dir.join("model.onnx"))?;
    download_file(
        &client,
        HF_ONNX_MODELS,
        "tokenizer.json",
        &model_dir.join("tokenizer.json"),
    )?;
    // Remove onnx_data if present so we use only the single model.onnx
    let _ = std::fs::remove_file(model_dir.join("model.onnx_data"));

    Ok(())
}
