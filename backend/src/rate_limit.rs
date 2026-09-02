use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use serde_json::json;

#[derive(Clone)]
pub struct RateLimitConfig {
    pub max_requests: u32,
    pub window_secs: u64,
}

impl RateLimitConfig {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            max_requests,
            window_secs,
        }
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    pub config: RateLimitConfig,
    store: Arc<tokio::sync::Mutex<HashMap<String, (u32, Instant)>>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            store: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }
}

pub async fn rate_limit_middleware(
    State(limiter): State<RateLimiter>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let ip = extract_client_ip(req.headers());
    let config = limiter.config.clone();
    let mut store = limiter.store.lock().await;
    let now = Instant::now();

    let entry = store.entry(ip).or_insert((0, now));
    if now - entry.1 > Duration::from_secs(config.window_secs) {
        *entry = (1, now);
    } else if entry.0 >= config.max_requests {
        let retry_after = config.window_secs;
        return Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("Retry-After", retry_after.to_string())
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::to_string(&json!({
                    "error": "rate_limit_exceeded",
                    "message": "Too many requests"
                }))
                .unwrap(),
            ))
            .unwrap();
    } else {
        entry.0 += 1;
    }

    next.run(req).await
}

fn extract_client_ip(headers: &axum::http::HeaderMap) -> String {
    if let Some(val) = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        return val.split(',').next().unwrap_or("unknown").trim().to_string();
    }
    if let Some(val) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        return val.to_string();
    }
    "unknown".to_string()
}

/// Extract the `vault_id` segment from a path like `/api/vaults/{vault_id}/...`.
/// Falls back to the full URI path so the bucket still exists even when no
/// vault segment is present.
fn extract_vault_id_from_path(path: &str) -> String {
    // Expected pattern: /api/vaults/<vault_id>/check-in
    let mut parts = path.split('/');
    // Skip leading empty string from the leading '/'
    parts.next();
    // "api"
    if parts.next() != Some("api") {
        return path.to_string();
    }
    // "vaults"
    if parts.next() != Some("vaults") {
        return path.to_string();
    }
    // The actual vault_id
    parts.next().unwrap_or(path).to_string()
}

/// Per-user (vault_id-keyed) rate-limit middleware.
///
/// Intended for the check-in endpoint. Keys the sliding-window counter on the
/// `vault_id` extracted from the URL path so that each vault owner has their
/// own independent quota (1 request per `window_secs` seconds by default).
pub async fn checkin_rate_limit_middleware(
    State(limiter): State<RateLimiter>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let vault_id = extract_vault_id_from_path(req.uri().path());
    let config = limiter.config.clone();
    let mut store = limiter.store.lock().await;
    let now = Instant::now();

    let entry = store.entry(vault_id).or_insert((0, now));
    if now - entry.1 > Duration::from_secs(config.window_secs) {
        // Window expired — reset counter and allow the request.
        *entry = (1, now);
    } else if entry.0 >= config.max_requests {
        let retry_after = config.window_secs;
        return Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("Retry-After", retry_after.to_string())
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::to_string(&json!({
                    "error": "rate_limit_exceeded",
                    "message": "Check-in rate limit exceeded. Please wait before checking in again."
                }))
                .unwrap(),
            ))
            .unwrap();
    } else {
        entry.0 += 1;
    }

    next.run(req).await
}
