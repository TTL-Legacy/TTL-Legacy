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

use axum::{
    body::Body,
    http::{HeaderValue, Request},
    middleware::Next,
    response::Response,
};

const DEFAULT_CSP: &str = "default-src 'self'";

/// Axum middleware that appends baseline security headers to every response.
pub async fn security_headers_middleware(req: Request<Body>, next: Next) -> Response<Body> {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();

    let csp = std::env::var("CSP_POLICY").unwrap_or_else(|_| DEFAULT_CSP.to_string());
    if let Ok(value) = HeaderValue::from_str(&csp) {
        headers.insert("content-security-policy", value);
    } else {
        tracing::warn!(csp = %csp, "CSP_POLICY is not a valid header value; falling back to default");
        headers.insert(
            "content-security-policy",
            HeaderValue::from_static(DEFAULT_CSP),
        );
    }

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
        assert_eq!(
            headers.get("content-security-policy").unwrap(),
            "default-src 'self'"
        );
        assert_eq!(
            headers.get("strict-transport-security").unwrap(),
            "max-age=63072000; includeSubDomains"
        );
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
        assert_eq!(headers.get("referrer-policy").unwrap(), "no-referrer");
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
}
