//! X-Request-ID correlation middleware.
//!
//! Reads the `X-Request-ID` header from each incoming HTTP request. If the
//! header is absent a new UUID v4 is generated. The ID is:
//! 1. Injected into the current [`tracing`] span as the `request_id` field so
//!    every log line emitted while handling the request carries the ID.
//! 2. Echoed back to the caller via the `X-Request-ID` response header.
//!
//! # Usage
//!
//! Apply the middleware globally in `main.rs`:
//!
//! ```rust,no_run
//! use axum::middleware;
//! use crate::request_id::request_id_middleware;
//!
//! let app = Router::new()
//!     // … routes …
//!     .layer(middleware::from_fn(request_id_middleware));
//! ```
//!
//! # Issue #1172
//! Add Structured Logging with Request Correlation IDs to Backend

use axum::{
    body::Body,
    http::{Request, Response},
    middleware::Next,
};
use tracing::Instrument;
use uuid::Uuid;

/// Axum middleware that attaches a per-request correlation ID to the tracing
/// span and to the `X-Request-ID` response header.
pub async fn request_id_middleware(mut req: Request<Body>, next: Next) -> Response<Body> {
    // Extract or generate the request ID.
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Insert the request ID back into the request headers so downstream
    // handlers can read it if needed.
    req.headers_mut().insert(
        "x-request-id",
        request_id
            .parse()
            .unwrap_or_else(|_| "invalid".parse().unwrap()),
    );

    // Create a span that carries the request_id field.
    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        method = %req.method(),
        uri = %req.uri(),
    );

    // Drive the rest of the request inside the span.
    let mut response = next.run(req).instrument(span).await;

    // Echo the ID back to the caller.
    if let Ok(hv) = request_id.parse() {
        response.headers_mut().insert("x-request-id", hv);
    }

    response
}
