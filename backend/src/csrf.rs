// Issue #1287: CSRF protection middleware for backend API endpoints.
//
// Strategy: Double-Submit Cookie
// ───────────────────────────────
// 1. A client first calls `GET /api/csrf-token` which generates a random
//    token, stores it in an `__Host-csrf` HttpOnly + SameSite=Strict cookie,
//    and also returns the token in the JSON body.
// 2. On every state-mutating request (POST / PUT / DELETE / PATCH) the client
//    must include the token in the `X-CSRF-Token` header.
// 3. The middleware reads the cookie value and the header value and rejects
//    (403 Forbidden) the request when they are absent or do not match.
//
// Safe methods (GET, HEAD, OPTIONS) are unconditionally allowed through.
// The `/health`, `/ready`, and `/health/consensus` diagnostics paths are also
// exempted because they carry no side effects and no session context.

use axum::{
    body::Body,
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

// ── Token generation ─────────────────────────────────────────────────────────

/// Generate a cryptographically random CSRF token (UUID v4 hex string, 36 bytes).
pub fn generate_token() -> String {
    Uuid::new_v4().to_string()
}

// ── Cookie helpers ────────────────────────────────────────────────────────────

/// Name of the CSRF cookie.
///
/// The `__Host-` prefix is a browser security feature: it requires the cookie
/// to be set with `Secure`, no `Domain` attribute, and `Path=/`. This prevents
/// subdomain injection attacks.
pub const CSRF_COOKIE_NAME: &str = "__Host-csrf";

/// Name of the request header clients must send.
pub const CSRF_HEADER_NAME: &str = "x-csrf-token";

/// Build the `Set-Cookie` header value for the CSRF token.
///
/// Flags:
/// - `HttpOnly` – prevents JavaScript access (the header copy is still readable
///   by JS; the cookie is only used for server-side comparison).
/// - `SameSite=Strict` – blocks the cookie from being sent on cross-site
///   requests originating from third-party pages.
/// - `Secure` – only transmitted over HTTPS (enforced by `__Host-` prefix too).
/// - `Path=/` – required by `__Host-` prefix rules.
pub fn csrf_cookie_header_value(token: &str) -> String {
    format!(
        "{CSRF_COOKIE_NAME}={token}; HttpOnly; SameSite=Strict; Secure; Path=/"
    )
}

// ── Error response ────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct CsrfErrorBody {
    code: &'static str,
    message: &'static str,
}

fn csrf_forbidden() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(CsrfErrorBody {
            code: "csrf_invalid",
            message: "CSRF token missing or invalid. Include the token from GET /api/csrf-token in the X-CSRF-Token header.",
        }),
    )
        .into_response()
}

// ── Middleware ────────────────────────────────────────────────────────────────

/// Paths that bypass CSRF validation entirely (diagnostics / token issuance).
const EXEMPT_PATHS: &[&str] = &[
    "/health",
    "/health/consensus",
    "/ready",
    "/api/csrf-token",
];

/// Axum middleware that enforces CSRF token validation on state-mutating
/// requests.
///
/// Mount it at the router level with:
/// ```ignore
/// Router::new()
///     .layer(axum::middleware::from_fn(csrf_middleware))
/// ```
pub async fn csrf_middleware(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();

    // Safe methods and exempt paths skip validation.
    if method == Method::GET
        || method == Method::HEAD
        || method == Method::OPTIONS
        || EXEMPT_PATHS.contains(&req.uri().path())
    {
        return next.run(req).await;
    }

    // Extract the token from the cookie.
    let cookie_token = extract_cookie_token(req.headers());

    // Extract the token from the request header.
    let header_token = req
        .headers()
        .get(CSRF_HEADER_NAME)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    match (cookie_token, header_token) {
        (Some(cookie), Some(header)) if !cookie.is_empty() && cookie == header => {
            next.run(req).await
        }
        _ => csrf_forbidden(),
    }
}

/// Parse the `Cookie` header and return the value of `__Host-csrf` if present.
fn extract_cookie_token(headers: &axum::http::HeaderMap) -> Option<String> {
    let cookie_header = headers.get("cookie")?.to_str().ok()?;
    for pair in cookie_header.split(';') {
        let pair = pair.trim();
        if let Some(value) = pair.strip_prefix(CSRF_COOKIE_NAME) {
            if let Some(value) = value.strip_prefix('=') {
                return Some(value.to_owned());
            }
        }
    }
    None
}

// ── /api/csrf-token handler ───────────────────────────────────────────────────

#[derive(Serialize)]
pub struct CsrfTokenResponse {
    pub csrf_token: String,
}

/// `GET /api/csrf-token`
///
/// Issues a fresh CSRF token. The token is set in a `__Host-csrf` cookie and
/// also returned in the JSON body so JavaScript code can read it and attach it
/// to subsequent mutating requests via the `X-CSRF-Token` header.
pub async fn csrf_token_handler() -> impl IntoResponse {
    let token = generate_token();
    let cookie = csrf_cookie_header_value(&token);
    (
        [
            ("Set-Cookie", cookie),
            ("Content-Type", "application/json".to_owned()),
        ],
        Json(CsrfTokenResponse {
            csrf_token: token,
        }),
    )
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod unit_tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn token_is_non_empty_uuid() {
        let t = generate_token();
        assert!(!t.is_empty());
        assert_eq!(t.len(), 36); // UUID v4: 8-4-4-4-12 + 4 dashes
    }

    #[test]
    fn tokens_are_unique() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
    }

    #[test]
    fn cookie_header_contains_all_flags() {
        let v = csrf_cookie_header_value("abc");
        assert!(v.contains("__Host-csrf=abc"));
        assert!(v.contains("HttpOnly"));
        assert!(v.contains("SameSite=Strict"));
        assert!(v.contains("Secure"));
        assert!(v.contains("Path=/"));
    }

    #[test]
    fn extract_cookie_returns_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "cookie",
            "__Host-csrf=my-token; other=value".parse().unwrap(),
        );
        assert_eq!(
            extract_cookie_token(&headers),
            Some("my-token".to_owned())
        );
    }

    #[test]
    fn extract_cookie_returns_none_when_absent() {
        let mut headers = HeaderMap::new();
        headers.insert("cookie", "other=value".parse().unwrap());
        assert!(extract_cookie_token(&headers).is_none());
    }

    #[test]
    fn extract_cookie_returns_none_with_no_cookie_header() {
        let headers = HeaderMap::new();
        assert!(extract_cookie_token(&headers).is_none());
    }
}
