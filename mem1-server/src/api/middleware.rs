//! Structured logging and X-Trace-Id (Constitution V, T007).

use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use tracing::Span;
use uuid::Uuid;

const HEADER_TRACE_ID: &str = "x-trace-id";

/// Middleware: ensure each request has a trace_id (from header or generate), attach to span and response.
pub async fn trace_layer(request: Request, next: Next) -> Response {
    let trace_id = request
        .headers()
        .get(HEADER_TRACE_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    Span::current().record("trace_id", tracing::field::display(&trace_id));

    let mut response = next.run(request).await;

    if let Some(h) = response.headers_mut().get_mut(HEADER_TRACE_ID) {
        *h = HeaderValue::try_from(trace_id.as_str()).unwrap_or(HeaderValue::from_static(""));
    } else if let Ok(h) = HeaderValue::try_from(trace_id) {
        response.headers_mut().insert(HEADER_TRACE_ID, h);
    }

    response
}
