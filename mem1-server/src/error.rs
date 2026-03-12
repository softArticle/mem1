//! Stable error codes and error type (Constitution V).

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// Stable error codes for API and logs (contract: api-http.md).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidInput,
    StorageError,
    EmbeddingError,
    NotFound,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput => write!(f, "INVALID_INPUT"),
            Self::StorageError => write!(f, "STORAGE_ERROR"),
            Self::EmbeddingError => write!(f, "EMBEDDING_ERROR"),
            Self::NotFound => write!(f, "NOT_FOUND"),
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("storage error: {0}")]
    Storage(#[from] anyhow::Error),

    #[error("embedding error: {0}")]
    Embedding(String),

    #[error("not found")]
    NotFound,
}

impl Error {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidInput(_) => ErrorCode::InvalidInput,
            Self::Storage(_) => ErrorCode::StorageError,
            Self::Embedding(_) => ErrorCode::EmbeddingError,
            Self::NotFound => ErrorCode::NotFound,
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_id: Option<String>,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Error::InvalidInput(_) => (StatusCode::BAD_REQUEST, self.code()),
            Error::NotFound => (StatusCode::NOT_FOUND, self.code()),
            Error::Storage(_) | Error::Embedding(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.code()),
        };
        let body = ErrorBody {
            code: code.to_string(),
            message: self.to_string(),
            trace_id: None, // set by middleware when available
        };
        (status, Json(body)).into_response()
    }
}
