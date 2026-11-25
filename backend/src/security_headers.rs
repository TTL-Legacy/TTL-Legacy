//! Security response headers — Issue #1181.
//!
//! The backend previously set no security-related HTTP headers at all, on
//! any response (including the simulator HTML and WebAuthn challenge
//! endpoints), leaving clients with no baseline protection against XSS,
//! clickjacking, or MIME-sniffing.
//!
//! This middleware appends a fixed set of headers to every response. CSP is
//! the one header a developer legitimately needs to loosen locally (e.g. to
//! allow a dev-server asset origin), so it alone is overridable via the
//! `CSP_POLICY` environment variable; the rest are not configurable since
//! there is no legitimate reason to weaken them per-deployment.
//!
//! The default CSP uses a nonce-based `script-src` directive instead of
//! `'unsafe-inline'`. A fresh random nonce is generated per request and
//! injected into both the `Content-Security-Policy` header and the HTML
//! template via the `CspNonce` struct.

use axum::{
    body::Body,
    http::{HeaderValue, Request},
    middleware::Next,
    response::Response,
};
use rand::Rng;

const DEFAULT_CSP: &str = "default-src 'self'";

/// A per-request CSP nonce. Pass this into the template renderer so that
/// inline `<script>` tags can carry a matching `nonce` attribute.
#[derive(Clone, Debug)]
pub struct CspNonce {
    /// The random nonce value (base64-encoded, 16 bytes → 22 chars).
    pub value: String,
}

impl CspNonce {
    /// Generate a fresh cryptographically random nonce.
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        let bytes: [u8; 16] = rng.gen();
        let value = base64::encode(bytes);
        Self { value }
    }
}

/// Build a `script-src` CSP directive string from a nonce.
fn script_src_with_nonce(nonce: &str) -> String {
    format!("script-src 'self' 'nonce-{nonce}'")
}

/// Axum middleware that appends baseline security headers to every response.
///
/// A random nonce is generated per request and included in the CSP
/// `script-src` directive so that inline scripts in the simulator HTML
/// can execute without resorting to `'unsafe-inline'`.
pub async fn security_headers_middleware(req: Request<Body>, next: Next) -> Response<Body> {
    let nonce = CspNonce::new();
    let nonce_for_header = nonce.value.clone();

    let mut response = next.run(req).await;
    let headers = response.headers_mut();

    let csp = std::env::var("CSP_POLICY").unwrap_or_else(|_| DEFAULT_CSP.to_string());
    let csp_with_nonce = if csp == DEFAULT_CSP {
        // Only inject the nonce when using the default policy; a custom
        // CSP_POLICY set by the operator is respected as-is.
        format!("{csp}; {}", script_src_with_nonce(&nonce_for_header))
    } else {
        csp
    };

    if let Ok(value) = HeaderValue::from_str(&csp_with_nonce) {
        headers.insert("content-security-policy", value);
    } else {
        tracing::warn!(csp = %csp_with_nonce, "CSP_POLICY is not a valid header value; falling back to default");
        headers.insert(
            "content-security-policy",
            HeaderValue::from_static(DEFAULT_CSP),
        );
    }

    // Store the nonce in the response extensions so handlers / templates can
    // retrieve it and inject it into inline `<script>` tags.
    response.extensions_mut().insert(nonce);

    headers.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=63072000; includeSubDomains"),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use tower::ServiceExt;

    async fn ok_handler() -> &'static str {
        "ok"
    }

    fn test_app() -> Router {
        Router::new()
            .route("/", get(ok_handler))
            .layer(axum::middleware::from_fn(security_headers_middleware))
    }

    #[tokio::test]
    async fn adds_all_baseline_security_headers() {
        std::env::remove_var("CSP_POLICY");
        let app = test_app();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let headers = response.headers();
        let csp = headers.get("content-security-policy").unwrap().to_str().unwrap();
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("script-src 'self' 'nonce-"));
        assert_eq!(
            headers.get("strict-transport-security").unwrap(),
            "max-age=63072000; includeSubDomains"
        );
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
        assert_eq!(headers.get("referrer-policy").unwrap(), "no-referrer");
        assert_eq!(
            headers.get("permissions-policy").unwrap(),
            "camera=(), microphone=(), geolocation=()"
        );
    }

    #[tokio::test]
    async fn csp_is_overridable_via_env_var_for_development() {
        std::env::set_var("CSP_POLICY", "default-src 'self' http://localhost:3001");
        let app = test_app();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(
            response.headers().get("content-security-policy").unwrap(),
            "default-src 'self' http://localhost:3001"
        );

        std::env::remove_var("CSP_POLICY");
    }

    #[tokio::test]
    async fn falls_back_to_default_csp_when_env_var_is_not_a_valid_header_value() {
        // A raw newline is rejected by HeaderValue — must not panic, must fall back.
        std::env::set_var("CSP_POLICY", "default-src 'self'\nx-injected: evil");
        let app = test_app();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(
            response.headers().get("content-security-policy").unwrap(),
            "default-src 'self'"
        );

        std::env::remove_var("CSP_POLICY");
    }

    #[tokio::test]
    async fn headers_are_present_on_a_non_2xx_response_too() {
        async fn not_found_handler() -> axum::http::StatusCode {
            axum::http::StatusCode::NOT_FOUND
        }
        let app = Router::new()
            .route("/missing", get(not_found_handler))
            .layer(axum::middleware::from_fn(security_headers_middleware));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        assert!(response.headers().get("x-frame-options").is_some());
    }

    #[tokio::test]
    async fn csp_header_present_on_every_response() {
        std::env::remove_var("CSP_POLICY");
        let app = test_app();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(response.headers().contains_key("content-security-policy"));
        let csp = response.headers().get("content-security-policy").unwrap().to_str().unwrap();
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("script-src 'self' 'nonce-"));
    }

    #[tokio::test]
    async fn hsts_header_present_on_every_response() {
        std::env::remove_var("CSP_POLICY");
        let app = test_app();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(response.headers().contains_key("strict-transport-security"));
        assert_eq!(
            response.headers().get("strict-transport-security").unwrap(),
            "max-age=63072000; includeSubDomains"
        );
    }

    #[tokio::test]
    async fn x_content_type_options_header_present_on_every_response() {
        std::env::remove_var("CSP_POLICY");
        let app = test_app();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(response.headers().contains_key("x-content-type-options"));
        assert_eq!(
            response.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
    }

    #[tokio::test]
    async fn x_frame_options_header_present_on_every_response() {
        std::env::remove_var("CSP_POLICY");
        let app = test_app();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(response.headers().contains_key("x-frame-options"));
        assert_eq!(
            response.headers().get("x-frame-options").unwrap(),
            "DENY"
        );
    }

    #[tokio::test]
    async fn permissions_policy_header_present_on_every_response() {
        std::env::remove_var("CSP_POLICY");
        let app = test_app();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(response.headers().contains_key("permissions-policy"));
        assert_eq!(
            response.headers().get("permissions-policy").unwrap(),
            "camera=(), microphone=(), geolocation=()"
        );
    }

    #[tokio::test]
    async fn referrer_policy_header_present_on_every_response() {
        std::env::remove_var("CSP_POLICY");
        let app = test_app();

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(response.headers().contains_key("referrer-policy"));
        assert_eq!(
            response.headers().get("referrer-policy").unwrap(),
            "no-referrer"
        );
    }

    #[tokio::test]
    async fn nonce_is_unique_per_request() {
        std::env::remove_var("CSP_POLICY");
        let app = test_app();

        let response1 = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let response2 = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let nonce1 = response1.extensions().get::<CspNonce>().unwrap();
        let nonce2 = response2.extensions().get::<CspNonce>().unwrap();
        assert_ne!(nonce1.value, nonce2.value);
    }
}