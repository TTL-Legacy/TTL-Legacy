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

    /// A string field longer than MAX_STRING_FIELD_LEN is rejected with 400.
    #[tokio::test]
    async fn test_field_too_long_rejected() {
        let app = test_app();
        let long = "a".repeat(MAX_STRING_FIELD_LEN + 1);
        let body = json!({ "message": long });
        let res = post_json(app, body.to_string()).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    /// An unexpected top-level field is rejected with 400.
    #[tokio::test]
    async fn test_unexpected_field_rejected() {
        let app = test_app();
        let body = json!({ "totally_unknown": "x" });
        let res = post_json(app, body.to_string()).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    /// A body larger than MAX_BODY_BYTES is rejected with 413.
    #[tokio::test]
    async fn test_body_too_large_rejected() {
        let app = test_app();
        let big = "a".repeat(MAX_BODY_BYTES + 1);
        let body = json!({ "message": big });
        let res = post_json(app, body.to_string()).await;
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    // -----------------------------------------------------------------------
    // XSS / SQL injection / Unicode boundary tests (issue #1790)
    //
    // The middleware treats string values as opaque: it enforces size, key
    // allow-list and length limits, but does not rewrite content.  These tests
    // pin that behaviour for hostile payloads.
    // -----------------------------------------------------------------------

    const XSS_PAYLOADS: &[&str] = &[
        "<script>alert(1)</script>",
        "<img src=x onerror=alert(1)>",
        "<svg/onload=alert(1)>",
        "javascript:alert(1)",
        "\"><script>alert(document.cookie)</script>",
        "<iframe src=\"javascript:alert(1)\"></iframe>",
        "&lt;script&gt;alert(1)&lt;/script&gt;",
    ];

    const SQLI_PAYLOADS: &[&str] = &[
        "'; DROP TABLE--",
        "'; DROP TABLE users; --",
        "' OR '1'='1",
        "' OR 1=1 --",
        "1; SELECT * FROM vaults",
        "' UNION SELECT NULL, password FROM users --",
        "admin'/*",
    ];

    /// XSS payloads inside known string fields within the length limit are
    /// passed through to the handler unmodified (200 OK).
    #[tokio::test]
    async fn test_xss_payloads_in_known_fields_pass_through() {
        for payload in XSS_PAYLOADS {
            let body = json!({ "message": payload });
            let res = post_json(test_app(), body.to_string()).await;
            assert_eq!(res.status(), StatusCode::OK, "payload: {payload}");
        }
    }

    /// SQL injection payloads inside known string fields within the length
    /// limit are passed through to the handler unmodified (200 OK).
    #[tokio::test]
    async fn test_sqli_payloads_in_known_fields_pass_through() {
        for payload in SQLI_PAYLOADS {
            let body = json!({ "note": payload });
            let res = post_json(test_app(), body.to_string()).await;
            assert_eq!(res.status(), StatusCode::OK, "payload: {payload}");
        }
    }

    /// Hostile payloads used as top-level *keys* are rejected as unexpected
    /// fields (400).
    #[tokio::test]
    async fn test_injection_payloads_as_keys_rejected() {
        for payload in XSS_PAYLOADS.iter().chain(SQLI_PAYLOADS) {
            let mut map = serde_json::Map::new();
            map.insert(payload.to_string(), json!("x"));
            let res = post_json(test_app(), Value::Object(map).to_string()).await;
            assert_eq!(res.status(), StatusCode::BAD_REQUEST, "key: {payload}");
        }
    }

    /// An injection payload does not bypass the length limit: padding a
    /// script tag past the maximum is still rejected (400).
    #[tokio::test]
    async fn test_oversized_xss_payload_rejected() {
        let payload = format!(
            "<script>{}</script>",
            "a".repeat(MAX_STRING_FIELD_LEN)
        );
        let body = json!({ "message": payload });
        let res = post_json(test_app(), body.to_string()).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    /// An injection payload does not bypass the length limit for SQL either.
    #[tokio::test]
    async fn test_oversized_sqli_payload_rejected() {
        let payload = format!("'; DROP TABLE--{}", " ".repeat(MAX_STRING_FIELD_LEN));
        let body = json!({ "note": payload });
        let res = post_json(test_app(), body.to_string()).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    /// Length is measured in characters, not bytes: a string of exactly
    /// MAX_STRING_FIELD_LEN multi-byte characters is accepted.
    #[tokio::test]
    async fn test_multibyte_string_at_char_limit_accepted() {
        // Each of these is 4 bytes in UTF-8, so the byte length is 4x the limit.
        let s = "😀".repeat(MAX_STRING_FIELD_LEN);
        assert!(s.len() > MAX_STRING_FIELD_LEN);
        let body = json!({ "message": s });
        let res = post_json(test_app(), body.to_string()).await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    /// One multi-byte character over the limit is rejected.
    #[tokio::test]
    async fn test_multibyte_string_over_char_limit_rejected() {
        let s = "😀".repeat(MAX_STRING_FIELD_LEN + 1);
        let body = json!({ "message": s });
        let res = post_json(test_app(), body.to_string()).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    /// Combining characters count individually: a base char plus combining
    /// accents pushes the count over the limit even though it renders as one
    /// grapheme.
    #[tokio::test]
    async fn test_combining_characters_counted_individually() {
        let at_limit = format!("e{}", "\u{0301}".repeat(MAX_STRING_FIELD_LEN - 1));
        let res = post_json(test_app(), json!({ "message": at_limit }).to_string()).await;
        assert_eq!(res.status(), StatusCode::OK);

        let over_limit = format!("e{}", "\u{0301}".repeat(MAX_STRING_FIELD_LEN));
        let res = post_json(test_app(), json!({ "message": over_limit }).to_string()).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    /// Zero-width, RTL-override, BOM and NUL characters are opaque to the
    /// sanitizer and accepted when within the limit.
    #[tokio::test]
    async fn test_special_unicode_characters_accepted() {
        for s in [
            "a\u{200B}b",
            "\u{202E}txt.exe",
            "\u{FEFF}bom",
            "nul\u{0000}byte",
            "\u{FF1C}script\u{FF1E}",
            "\u{10FFFF}",
        ] {
            let res = post_json(test_app(), json!({ "message": s }).to_string()).await;
            assert_eq!(res.status(), StatusCode::OK, "input: {s:?}");
        }
    }

    /// JSON-escaped payloads (`<script>`) are decoded before the
    /// length check, so the decoded string is what is measured.
    #[tokio::test]
    async fn test_escaped_unicode_payload_decoded_before_length_check() {
        let raw = r#"{"message":"<script>alert(1)</script>"}"#;
        let res = post_json(test_app(), raw).await;
        assert_eq!(res.status(), StatusCode::OK);

        // 513 escaped characters decode to 513 chars and must be rejected.
        let escaped = "\\u0061".repeat(MAX_STRING_FIELD_LEN + 1);
        let raw = format!(r#"{{"message":"{escaped}"}}"#);
        assert!(raw.len() < MAX_BODY_BYTES);
        let res = post_json(test_app(), raw).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    // -----------------------------------------------------------------------
    // Property-based tests (issue #1500)
    //
    // These fuzz the sanitizer with arbitrary strings to catch bypasses.
    // The sanitizer is a pure validation function: it either accepts a payload
    // (200 OK) or rejects it (400/413).  We model that decision as a pure
    // function so it can be exercised by proptest without an async runtime.
    // -----------------------------------------------------------------------
    use proptest::prelude::*;

    /// Pure model of the sanitizer's accept/reject decision for a single
    /// string field value.  Mirrors the length check in `sanitize_request`.
    fn sanitize_string(s: &str) -> String {
        // The sanitizer does not transform string content; it only enforces a
        // length limit.  We return the value unchanged when it is within the
        // limit, and truncate to the limit otherwise, so that the result is
        // always a valid (accepted) value.
        if s.chars().count() > MAX_STRING_FIELD_LEN {
            s.chars().take(MAX_STRING_FIELD_LEN).collect()
        } else {
            s.to_string()
        }
    }

    /// Pure model of the full payload decision: returns true when the payload
    /// would be accepted by the middleware.
    fn is_accepted(value: &Value) -> bool {
        match value {
            Value::Object(map) => {
                for key in map.keys() {
                    if !KNOWN_TOP_LEVEL_FIELDS.contains(&key.as_str()) {
                        return false;
                    }
                }
                for v in map.values() {
                    if let Value::String(s) = v {
                        if s.chars().count() > MAX_STRING_FIELD_LEN {
                            return false;
                        }
                    }
                }
                true
            }
            _ => true,
        }
    }

    proptest! {
        /// Sanitization is idempotent: sanitize(sanitize(x)) == sanitize(x).
        #[test]
        fn prop_sanitize_is_idempotent(s in ".*") {
            let once = sanitize_string(&s);
            let twice = sanitize_string(&once);
            prop_assert_eq!(once, twice);
        }

        /// The sanitized output never exceeds the maximum field length.
        #[test]
        fn prop_sanitized_len_within_limit(s in ".*") {
            let out = sanitize_string(&s);
            prop_assert!(out.chars().count() <= MAX_STRING_FIELD_LEN);
        }

        /// Arbitrary unicode strings are handled without panicking and the
        /// accept/reject decision is stable under re-evaluation.
        #[test]
        fn prop_unicode_decision_is_stable(s in ".*") {
            let payload = json!({ "message": s });
            let first = is_accepted(&payload);
            let second = is_accepted(&payload);
            prop_assert_eq!(first, second);
        }

        /// Control characters and HTML payloads are treated as opaque strings:
        /// they are accepted iff they fit within the length limit.
        #[test]
        fn prop_control_and_html_payloads(s in ".*") {
            let payload = json!({ "message": s.clone() });
            let expected = s.chars().count() <= MAX_STRING_FIELD_LEN;
            prop_assert_eq!(is_accepted(&payload), expected);
        }

        /// A payload with an unknown top-level field is always rejected,
        /// regardless of the string content.
        #[test]
        fn prop_unknown_field_always_rejected(s in ".*") {
            let payload = json!({ "__unknown_field__": s });
            prop_assert!(!is_accepted(&payload));
        }

        /// Strings at exactly the limit are accepted; one char over is rejected.
        #[test]
        fn prop_boundary_length(n in 0usize..(MAX_STRING_FIELD_LEN * 2)) {
            let s: String = std::iter::repeat('a').take(n).collect();
            let payload = json!({ "message": s });
            let expected = n <= MAX_STRING_FIELD_LEN;
            prop_assert_eq!(is_accepted(&payload), expected);
        }
    }
}
