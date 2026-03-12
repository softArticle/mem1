use crate::error::Error;
use rig::client::EmbeddingsClient;
use rig::embeddings::EmbeddingsBuilder;

#[cfg(feature = "local-embed")]
use std::sync::Arc;
#[cfg(feature = "local-embed")]
use super::local_embed::LocalEmbedder;
#[cfg(feature = "local-embed")]
use std::path::PathBuf;
#[cfg(feature = "local-embed")]
use super::download;

#[derive(Clone)]
pub enum Embedder {
    Off,
    OpenAI {
        model: rig::providers::openai::EmbeddingModel,
    },
    #[cfg(feature = "local-embed")]
    Local {
        inner: Arc<LocalEmbedder>,
    },
}

impl Embedder {
    /// Default provider when MEM1_EMBED_PROVIDER is unset: "local" if local-embed feature is enabled, else "off".
    pub fn from_env() -> Result<Self, Error> {
        let default_provider = if cfg!(feature = "local-embed") {
            "local"
        } else {
            "off"
        };
        let provider =
            std::env::var("MEM1_EMBED_PROVIDER").unwrap_or_else(|_| default_provider.into());
        match provider.as_str() {
            "off" | "disabled" | "none" => Ok(Self::Off),
            "openai" => {
                let key = std::env::var("OPENAI_API_KEY")
                    .map_err(|_| Error::InvalidInput("OPENAI_API_KEY is required".to_string()))?;
                let client = rig::providers::openai::Client::new(&key)
                    .map_err(|e| Error::Embedding(e.to_string()))?;
                let model = std::env::var("MEM1_OPENAI_EMBED_MODEL")
                    .unwrap_or_else(|_| rig::providers::openai::TEXT_EMBEDDING_3_SMALL.to_string());
                Ok(Self::OpenAI {
                    model: client.embedding_model(model.as_str()),
                })
            }
            "local" => {
                #[cfg(feature = "local-embed")]
                {
                    const DEFAULT_LOCAL_EMBED_MODEL_DIR: &str = "embed_model";
                    let (model_dir, is_default) =
                        match std::env::var("MEM1_LOCAL_EMBED_MODEL_DIR") {
                            Ok(d) => (d, false),
                            Err(_) => (DEFAULT_LOCAL_EMBED_MODEL_DIR.to_string(), true),
                        };
                    let path = PathBuf::from(&model_dir);
                    let max_length = std::env::var("MEM1_LOCAL_EMBED_MAX_LENGTH")
                        .ok()
                        .and_then(|s| s.parse().ok());
                    match LocalEmbedder::load(&path, max_length) {
                        Ok(inner) => Ok(Self::Local {
                            inner: Arc::new(inner),
                        }),
                        Err(e) if is_default => {
                            tracing::info!(
                                path = %path.display(),
                                error = %e,
                                "local embed model not found, downloading default model..."
                            );
                            if let Err(dl) = download::download_default_model(&path) {
                                tracing::warn!(
                                    path = %path.display(),
                                    error = %dl,
                                    "download default model failed, running without embedding"
                                );
                                return Ok(Self::Off);
                            }
                            match LocalEmbedder::load(&path, max_length) {
                                Ok(inner) => {
                                    tracing::info!("default embed model loaded");
                                    return Ok(Self::Local {
                                        inner: Arc::new(inner),
                                    });
                                }
                                Err(e2) => {
                                    tracing::warn!(
                                        path = %path.display(),
                                        error = %e2,
                                        "load after download failed, trying alternative model..."
                                    );
                                    if let Err(dl2) = download::download_alternative_model(&path) {
                                        tracing::warn!(
                                            path = %path.display(),
                                            error = %dl2,
                                            "download alternative model failed, running without embedding"
                                        );
                                        return Ok(Self::Off);
                                    }
                                    match LocalEmbedder::load(&path, max_length) {
                                        Ok(inner) => {
                                            tracing::info!("alternative embed model loaded");
                                            return Ok(Self::Local {
                                                inner: Arc::new(inner),
                                            });
                                        }
                                        Err(e3) => {
                                            tracing::warn!(
                                                path = %path.display(),
                                                error = %e3,
                                                "load after alternative download failed, running without embedding"
                                            );
                                            return Ok(Self::Off);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => Err(e),
                    }
                }
                #[cfg(not(feature = "local-embed"))]
                Err(Error::InvalidInput(
                    "MEM1_EMBED_PROVIDER=local requires the local-embed feature. \
                     Build with default features, or set MEM1_EMBED_PROVIDER=off to run without embedding."
                        .to_string(),
                ))
            }
            other => Err(Error::InvalidInput(format!(
                "invalid MEM1_EMBED_PROVIDER: {other} (use off|local|openai)"
            ))),
        }
    }

    pub async fn embed_text(&self, text: &str) -> Result<Option<Vec<f32>>, Error> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(None);
        }

        match self {
            Self::Off => Ok(None),
            Self::OpenAI { model } => {
                let embeddings = EmbeddingsBuilder::new(model.clone())
                    .document(text.to_string())
                    .map_err(|e| Error::Embedding(e.to_string()))?
                    .build()
                    .await
                    .map_err(|e| Error::Embedding(e.to_string()))?;

                let (_doc, one_or_many) = embeddings
                    .into_iter()
                    .next()
                    .ok_or_else(|| Error::Embedding("no embedding returned".to_string()))?;
                let first = one_or_many.first_ref();
                Ok(Some(first.vec.iter().map(|v| *v as f32).collect()))
            }
            #[cfg(feature = "local-embed")]
            Self::Local { inner } => {
                let inner = Arc::clone(inner);
                let text = text.to_string();
                let vec = tokio::task::spawn_blocking(move || inner.embed_sync(&text))
                    .await
                    .map_err(|e| Error::Embedding(format!("spawn_blocking: {e}")))??;
                if vec.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(vec))
                }
            }
        }
    }
}

