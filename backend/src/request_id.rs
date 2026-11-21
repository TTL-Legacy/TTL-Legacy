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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Request, StatusCode};
    use axum::body::Body;
    use tower::ServiceBuilder;
    use tower::service_fn;

    async fn make_test_request(headers: &[(&str, &str)]) -> Response<Body> {
        let mut req = Request::builder().method("GET").uri("/test");
        for (key, value) in headers {
            req = req.header(*key, *value);
        }
        let req = req.body(Body::empty()).unwrap();

        let next = service_fn(|_req: Request<Body>| async {
            Ok::<_, std::convert::Infallible>(Response::builder().status(200).body(Body::empty()).unwrap())
        });

        request_id_middleware(req, next).await
    }

    #[tokio::test]
    async fn test_incoming_header_is_preserved() {
        let incoming_id = "custom-request-123";
        let response = make_test_request(&[("x-request-id", incoming_id)]).await;

        let header_value = response
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        assert_eq!(header_value, Some(incoming_id.to_string()));
    }

    #[tokio::test]
    async fn test_id_generated_when_missing() {
        let response = make_test_request(&[]).await;

        let header_value = response
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok());

        assert!(header_value.is_some());
        let id = header_value.unwrap();
        assert!(!id.is_empty());
        assert!(id.len() == 36, "UUID v4 should be 36 chars with hyphens");
    }

    #[tokio::test]
    async fn test_id_appears_in_response() {
        let headers = vec![("x-request-id", "test-id-456")];
        let response = make_test_request(&headers).await;

        assert!(response.headers().contains_key("x-request-id"));
        let id = response
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok());
        assert_eq!(id, Some("test-id-456"));
    }
}
