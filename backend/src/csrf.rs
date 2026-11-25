// Issue #1287: CSRF protection middleware for backend API endpoints.
//
// Strategy: Double-Submit Cookie
// ───────────────────────────────
// 1. A client first calls `GET /api/csrf-token` which generates a random
//    token, stores it in an `__Host-csrf` HttpOnly + SameSite=Strict cookie,
//    and also returns the token in the JSON body.
// 2. On every state-mutating request (POST / PUT / DELETE / PATCH) the client
//    must include the token in the `X-CSRF-Token` header or as a form field
//    in `application/x-www-form-urlencoded` or `multipart/form-data` bodies.
// 3. The middleware reads the cookie value and the header value (or form-field
//    value for form submissions) and rejects (403 Forbidden) the request when
//    they are absent or do not match.
// 4. After a request passes validation the token is rotated: the response
//    carries a fresh `__Host-csrf` Set-Cookie and the new token in the
//    `X-CSRF-Token` response header, so a leaked token is only valid until
//    the next state-changing request (mitigates BREACH-style leakage).
//
// Safe methods (GET, HEAD, OPTIONS) are unconditionally allowed through.
// The `/health`, `/healthz`, `/ready`, `/readyz`, and `/health/consensus` diagnostics paths are also
// exempted because they carry no side effects and no session context.

use axum::{
    body::Body,
    extract::State,
    http::{
        header::{HeaderValue, SET_COOKIE},
        Method, Request, StatusCode,
    },
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use multer::Multipart;
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
    format!("{CSRF_COOKIE_NAME}={token}; HttpOnly; SameSite=Strict; Secure; Path=/")
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
    "/healthz",
    "/health/consensus",
    "/ready",
    "/readyz",
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

    // Extract the token from the request body for form submissions
    // (multipart/form-data and application/x-www-form-urlencoded).
    let (body_token, new_body) = {
        let content_type = req
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok());
        let needs_body_check = content_type.map_or(false, |ct| {
            ct.starts_with("multipart/form-data")
                || ct.starts_with("application/x-www-form-urlencoded")
        });
        if needs_body_check {
            let uri = req.uri().clone();
            let method = req.method().clone();
            let headers = req.headers().clone();
            let body = req.into_body();
            let (token, body) =
                extract_body_token(&headers, body).await;
            let new_req = Request::builder()
                .uri(uri)
                .method(method)
                .headers(headers)
                .body(body)
                .unwrap_or_else(|_| Body::empty());
            (token, new_req)
        } else {
            (None, req)
        }
    };
    let req = new_body;

    // The token is valid if it matches the cookie in either the header
    // or the form body.
    let token_valid = cookie_token.as_ref().is_some_and(|cookie| {
        !cookie.is_empty()
            && (header_token.as_ref() == Some(cookie)
                || body_token.as_ref() == Some(cookie))
    });

    match (cookie_token, token_valid) {
        (Some(cookie), true) => {
            let mut response = next.run(req).await;
            rotate_token(&mut response);
            response
        }
        _ => csrf_forbidden(),
    }
}

/// Try to extract a CSRF token from the request body for form submissions.
///
/// For `multipart/form-data` and `application/x-www-form-urlencoded`
/// requests the CSRF token may be included as a form field named
/// `_csrf`. This function reads the body into memory, parses it,
/// extracts the token if present, and then reconstructs the body
/// so the handler can still consume it.
async fn extract_body_token(
    headers: &axum::http::HeaderMap,
    body: Body,
) -> (Option<String>, Body) {
    let content_type = match headers.get("content-type") {
        Some(v) => match v.to_str() {
            Ok(s) => s,
            Err(_) => return (None, body),
        },
        None => return (None, body),
    };

    if !content_type.starts_with("multipart/form-data")
        && !content_type.starts_with("application/x-www-form-urlencoded")
    {
        return (None, body);
    }

    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return (None, Body::empty()),
    };

    let token = if content_type.starts_with("multipart/form-data") {
        extract_multipart_csrf_token(&bytes, content_type).await
    } else {
        extract_urlencoded_csrf_token(&bytes).await
    };

    (token, Body::from(bytes.to_vec()))
}

/// Replace the CSRF token after a validated state-mutating request.
///
/// The new token is written to the `__Host-csrf` cookie and returned in the
/// `X-CSRF-Token` response header so the client can use it for its next request.
fn rotate_token(response: &mut Response) {
    let token = generate_token();
    if let Ok(cookie) = HeaderValue::from_str(&csrf_cookie_header_value(&token)) {
        response.headers_mut().append(SET_COOKIE, cookie);
    }
    if let Ok(value) = HeaderValue::from_str(&token) {
        response.headers_mut().insert(CSRF_HEADER_NAME, value);
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

/// Extract the CSRF token from a `multipart/form-data` body.
async fn extract_multipart_csrf_token(
    bytes: &[u8],
    content_type: &str,
) -> Option<String> {
    let boundary = get_boundary(content_type)?;
    let body_bytes = axum::body::Bytes::copy_from_slice(bytes);
    let stream = futures::stream::once(async move {
        Ok::<_, multer::Error>(body_bytes)
    });
    let mut multipart = Multipart::new(stream, &boundary);
    while let Some(field) = multipart.next_field().await.ok()? {
        if field.name()? == "_csrf" {
            return String::from_utf8(field.bytes().await.ok()?.to_vec()).ok();
        }
    }
    None
}

/// Extract the CSRF token from an `application/x-www-form-urlencoded` body.
async fn extract_urlencoded_csrf_token(bytes: &[u8]) -> Option<String> {
    let body_str = String::from_utf8(bytes.to_vec()).ok()?;
    for pair in body_str.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            if key == "_csrf" {
                return Some(urlencoding::decode(value)?.into_owned());
            }
        }
    }
    None
}

/// Extract the boundary parameter from a `Content-Type` header value.
fn get_boundary(content_type: &str) -> Option<String> {
    content_type
        .split(';')
        .find_map(|part| {
            let part = part.trim();
            if part.starts_with("boundary=") {
                Some(part["boundary=".len()..].trim().to_string())
            } else {
                None
            }
        })
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
        Json(CsrfTokenResponse { csrf_token: token }),
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
        assert_eq!(extract_cookie_token(&headers), Some("my-token".to_owned()));
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

// ── Middleware integration tests ───────────────────────────────────────────

#[cfg(test)]
mod middleware_tests {
    use super::*;
    use axum::{routing::post, Router};
    use tower::ServiceExt;

    async fn state_mutating_handler() -> &'static str {
        "mutated"
    }

    fn test_app() -> Router {
        Router::new()
            .route("/api/data", post(state_mutating_handler))
            .layer(axum::middleware::from_fn(csrf_middleware))
    }

    #[tokio::test]
    async fn missing_token_rejected() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/data")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn invalid_token_rejected() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/data")
                    .header(
                        "cookie",
                        "__Host-csrf=token-from-cookie; other=value",
                    )
                    .header("x-csrf-token", "different-token-from-header")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn valid_token_accepted() {
        let app = test_app();
        let token = "same-token-everywhere";

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/data")
                    .header("cookie", format!("__Host-csrf={}", token))
                    .header("x-csrf-token", token)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn safe_methods_exempt() {
        for method in &["GET", "HEAD", "OPTIONS"] {
            let app = test_app();

            let response = app
                .oneshot(
                    Request::builder()
                        .method(*method)
                        .uri("/api/data")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert!(
                response.status().is_success(),
                "Method {} should be exempt from CSRF checks",
                method
            );
        }
    }

    #[tokio::test]
    async fn exempt_paths_bypass_validation() {
        for path in &[
            "/health",
            "/healthz",
            "/health/consensus",
            "/ready",
            "/readyz",
            "/api/csrf-token",
        ] {
            let app = test_app();

            let response = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(*path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_ne!(
                response.status(),
                StatusCode::FORBIDDEN,
                "Path {} should be exempt from CSRF checks",
                path
            );
        }
    }

    #[tokio::test]
    async fn missing_header_token_rejected() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/data")
                    .header("cookie", "__Host-csrf=token-in-cookie")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn missing_cookie_token_rejected() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/data")
                    .header("x-csrf-token", "token-in-header")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn multipart_form_with_valid_csrf_token_accepted() {
        let app = test_app();
        let token = "same-token-everywhere";
        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";

        let body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"_csrf\"\r\n\
             \r\n\
             {token}\r\n\
             --{boundary}--\r\n"
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/data")
                    .header("cookie", format!("__Host-csrf={}", token))
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn multipart_form_with_invalid_csrf_token_rejected() {
        let app = test_app();
        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";

        let body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"_csrf\"\r\n\
             \r\n\
             wrong-token\r\n\
             --{boundary}--\r\n"
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/data")
                    .header("cookie", "__Host-csrf=token-in-cookie")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn urlencoded_form_with_valid_csrf_token_accepted() {
        let app = test_app();
        let token = "same-token-everywhere";

        let body = format!("_csrf={token}");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/data")
                    .header("cookie", format!("__Host-csrf={}", token))
                    .header(
                        "content-type",
                        "application/x-www-form-urlencoded",
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn urlencoded_form_with_invalid_csrf_token_rejected() {
        let app = test_app();

        let body = "_csrf=wrong-token";

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/data")
                    .header("cookie", "__Host-csrf=token-in-cookie")
                    .header(
                        "content-type",
                        "application/x-www-form-urlencoded",
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
