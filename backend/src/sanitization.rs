/// Issue #1199 — Backend Request Input Sanitization Middleware
///
/// Enforces the following limits on every incoming request before it reaches
/// any route handler:
///
/// | Limit                         | Value     | HTTP error |
/// |-------------------------------|-----------|------------|
/// | Maximum request body size     | 64 KB     | 413        |
/// | Maximum string field length   | 512 chars | 400        |
/// | Unexpected top-level fields   | rejected  | 400        |
///
/// The middleware is applied as a `tower` layer in `main.rs`.  It buffers
/// the raw body bytes, checks the total size, then parses the JSON and
/// validates every top-level string field.
///
/// Fields with non-string values (numbers, booleans, nested objects) are
/// passed through unchanged — only string fields are length-checked.
/// Unknown top-level keys are rejected unless the route explicitly opts out
/// (not currently implemented; all routes share these limits).
use axum::{
    body::Body,
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::Value;

/// Maximum raw request body size in bytes (64 KiB).
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// Maximum length of any string field value in a JSON payload (characters).
pub const MAX_STRING_FIELD_LEN: usize = 512;

/// Known / expected top-level fields shared across TTL-Legacy API payloads.
/// Requests containing *other* top-level keys are rejected with 400.
///
/// Extend this list when new request schemas are added.  An alternative
/// approach (allow-list per route) can be implemented by passing the list as
/// router state.
pub const KNOWN_TOP_LEVEL_FIELDS: &[&str] = &[
    // reminder-preferences
    "channels",
    "hours_before_expiry",
    "frequency",
    // subscriptions
    "endpoint",
    "keys",
    "expiration_time",
    "vault_id",
    // sponsored-release / simulate-release
    "scenario_types",
    "vault_balance",
    "check_in_interval",
    // notifications
    "email",
    "phone",
    "token",
    // 2FA / auth
    "code",
    "secret",
    // generic
    "message",
    "note",
    "notes",
    "metadata",
    "name",
    "description",
    "label",
];

/// Axum middleware function.
///
/// Mount it as a global layer in `main.rs`:
/// ```rust
/// let app = Router::new()
///     /* ... routes ... */
///     .layer(axum::middleware::from_fn(sanitize_request));
/// ```
pub async fn sanitize_request(request: Request, next: Next) -> Response {
    // Only JSON POST/PUT/PATCH bodies need full validation.
    let method = request.method().clone();
    let is_mutation = matches!(
        method,
        axum::http::Method::POST | axum::http::Method::PUT | axum::http::Method::PATCH
    );

    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let is_json = content_type.contains("application/json");

    if !is_mutation || !is_json {
        // Pass through non-JSON or read-only requests without buffering.
        return next.run(request).await;
    }

    // --- 1. Split the request into parts so we can read + replace the body ---
    let (parts, body) = request.into_parts();

    // Collect the full body up to MAX_BODY_BYTES + 1 bytes.
    // If the body exceeds that limit, axum returns an error which we map to 413.
    let bytes = match axum::body::to_bytes(body, MAX_BODY_BYTES + 1).await {
        Ok(b) => b,
        Err(_) => {
            // Body exceeded the read limit — treat as too large.
            return error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                &format!(
                    "Request body exceeds the maximum allowed size of {} bytes",
                    MAX_BODY_BYTES
                ),
            );
        }
    };

    // --- 2. Enforce body size limit (413 Payload Too Large) ---
    // Double-check: if somehow we got exactly MAX_BODY_BYTES + 1 bytes, reject.
    if bytes.len() > MAX_BODY_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            &format!(
                "Request body exceeds the maximum allowed size of {} bytes",
                MAX_BODY_BYTES
            ),
        );
    }

    // Empty bodies are fine (e.g. DELETE with no payload).
    if bytes.is_empty() {
        let rebuilt = Request::from_parts(parts, Body::empty());
        return next.run(rebuilt).await;
    }

    // --- 3. Parse JSON ---
    let json: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            // Not valid JSON — let the route handler produce the error.
            let rebuilt = Request::from_parts(parts, Body::from(bytes));
            return next.run(rebuilt).await;
        }
    };

    // Only validate top-level objects.
    if let Value::Object(ref map) = json {
        // --- 4. Reject unexpected top-level fields ---
        for key in map.keys() {
            if !KNOWN_TOP_LEVEL_FIELDS.contains(&key.as_str()) {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    "unexpected_field",
                    &format!("Unexpected field: '{}'", key),
                );
            }
        }

        // --- 5. Enforce maximum string field length ---
        for (key, value) in map {
            if let Value::String(s) = value {
                if s.chars().count() > MAX_STRING_FIELD_LEN {
                    return error_response(
                        StatusCode::BAD_REQUEST,
                        "field_too_long",
                        &format!(
                            "Field '{}' exceeds the maximum allowed length of {} characters",
                            key, MAX_STRING_FIELD_LEN
                        ),
                    );
                }
            }
        }
    }

    // --- 6. Rebuild the request with the original (validated) bytes ---
    let rebuilt = Request::from_parts(parts, Body::from(bytes));
    next.run(rebuilt).await
}

/// Construct a plain JSON error response.
fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({
            "code": code,
            "message": message,
        })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        middleware,
        response::IntoResponse,
        routing::post,
        Json, Router,
    };
    use serde_json::json;
    use tower::ServiceExt;

    async fn echo_handler() -> impl IntoResponse {
        (StatusCode::OK, Json(json!({ "ok": true })))
    }

    fn test_app() -> Router {
        Router::new()
            .route("/api/test", post(echo_handler))
            .layer(middleware::from_fn(sanitize_request))
    }

    async fn post_json(app: Router, body: impl Into<String>) -> axum::response::Response {
        let body_str = body.into();
        app.oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/test")
                .header("content-type", "application/json")
                .body(Body::from(body_str))
                .unwrap(),
        )
        .await
        .unwrap()
    }

    /// A valid JSON payload within all limits should pass through (200 OK).
    #[tokio::test]
    async fn test_valid_request_passes_through() {
        let app = test_app();
        let body =
            json!({ "channels": ["email"], "hours_before_expiry": 24, "frequency": "daily" });
        let res = post_json(app, body.to_string()).await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    /// A body exceeding 64 KB must return 413 Payload Too Large.
    #[tokio::test]
    async fn test_oversized_body_returns_413() {
        let app = test_app();
        // Build a JSON string that is just over 64 KiB.
        let large_value = "x".repeat(MAX_BODY_BYTES + 1);
        // We construct raw JSON manually to guarantee the full byte count.
        let body = format!("{{\"note\":\"{}\"}}", large_value);
        assert!(body.len() > MAX_BODY_BYTES);

        let res = post_json(app, body).await;
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    /// A string field exceeding 512 characters must return 400 Bad Request.
    #[tokio::test]
    async fn test_oversized_field_returns_400() {
        let app = test_app();
        let long_value = "a".repeat(MAX_STRING_FIELD_LEN + 1);
        let body = json!({ "note": long_value });
        let res = post_json(app, body.to_string()).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // Confirm the error payload contains a meaningful code.
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed["code"], "field_too_long");
    }

    /// An unexpected top-level field must return 400 Bad Request.
    #[tokio::test]
    async fn test_unexpected_field_returns_400() {
        let app = test_app();
        let body = json!({ "unknown_field_xyz": "value" });
        let res = post_json(app, body.to_string()).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed["code"], "unexpected_field");
    }

    /// A GET request must pass through without body validation.
    #[tokio::test]
    async fn test_get_request_passes_through_without_validation() {
        let app = Router::new()
            .route("/api/test", axum::routing::get(echo_handler))
            .layer(middleware::from_fn(sanitize_request));

        let res = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
    }

    /// A string field at exactly the limit should pass through (boundary check).
    #[tokio::test]
    async fn test_field_at_exact_limit_passes() {
        let app = test_app();
        let exact_value = "a".repeat(MAX_STRING_FIELD_LEN);
        let body = json!({ "note": exact_value });
        let res = post_json(app, body.to_string()).await;
        assert_eq!(res.status(), StatusCode::OK);
    }
}
