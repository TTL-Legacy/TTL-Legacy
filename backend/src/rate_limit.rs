use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    body::Body,
    extract::State,
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

/// Selects which backend the rate limiter uses to persist counters.
///
/// Defaults to [`RateLimitStore::Memory`] so local development keeps working
/// without any external dependency. Set `RATE_LIMIT_STORE=redis` (and
/// `REDIS_URL`) to share counters across processes/replicas so limits survive
/// restarts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RateLimitStore {
    Memory,
    Redis { url: String },
}

impl RateLimitStore {
    /// Resolve the store from the environment.
    ///
    /// `RATE_LIMIT_STORE` accepts `memory` (default) or `redis`. When `redis`
    /// is selected, `REDIS_URL` must be set; otherwise we fall back to the
    /// in-memory store so a misconfiguration never disables rate limiting.
    pub fn from_env() -> Self {
        match std::env::var("RATE_LIMIT_STORE").as_deref() {
            Ok("redis") => match std::env::var("REDIS_URL") {
                Ok(url) if !url.is_empty() => RateLimitStore::Redis { url },
                _ => RateLimitStore::Memory,
            },
            _ => RateLimitStore::Memory,
        }
    }
}

/// In-memory sliding-window counter store. Per-process only; used as the
/// default backend for development.
#[derive(Clone, Default)]
pub struct MemoryStore {
    entries: Arc<tokio::sync::Mutex<HashMap<String, (u32, Instant)>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment `key` and return `Ok(())` when the request is allowed, or
    /// `Err(retry_after_secs)` when the window quota is exhausted.
    pub async fn check(
        &self,
        key: &str,
        max_requests: u32,
        window_secs: u64,
    ) -> Result<(), u64> {
        let mut store = self.entries.lock().await;
        let now = Instant::now();
        let entry = store.entry(key.to_string()).or_insert((0, now));
        if now - entry.1 > Duration::from_secs(window_secs) {
            *entry = (1, now);
            Ok(())
        } else if entry.0 >= max_requests {
            Err(window_secs)
        } else {
            entry.0 += 1;
            Ok(())
        }
    }
}

/// Redis-backed counter store so limits are shared across replicas and survive
/// restarts. Uses a fixed-window counter keyed by `key` with a TTL equal to the
/// configured window.
#[derive(Clone)]
pub struct RedisStore {
    client: redis::Client,
}

impl RedisStore {
    pub fn new(url: &str) -> Result<Self, redis::RedisError> {
        Ok(Self {
            client: redis::Client::open(url)?,
        })
    }

    /// Increment `key` and return `Ok(())` when the request is allowed, or
    /// `Err(retry_after_secs)` when the window quota is exhausted.
    ///
    /// On any Redis error we fail open (allow the request) so a transient
    /// outage does not take down the API.
    pub async fn check(
        &self,
        key: &str,
        max_requests: u32,
        window_secs: u64,
    ) -> Result<(), u64> {
        let mut conn = match self.client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => return Ok(()),
        };

        let redis_key = format!("rate_limit:{key}");
        let count: u32 = match redis::cmd("INCR")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await
        {
            Ok(count) => count,
            Err(_) => return Ok(()),
        };

        if count == 1 {
            let _: Result<(), _> = redis::cmd("EXPIRE")
                .arg(&redis_key)
                .arg(window_secs)
                .query_async(&mut conn)
                .await;
        }

        if count > max_requests {
            Err(window_secs)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    pub config: RateLimitConfig,
    store: RateLimitStore,
    memory: MemoryStore,
    redis: Option<RedisStore>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self::with_store(config, RateLimitStore::Memory)
    }

    pub fn with_store(config: RateLimitConfig, store: RateLimitStore) -> Self {
        let redis = match &store {
            RateLimitStore::Redis { url } => RedisStore::new(url).ok(),
            RateLimitStore::Memory => None,
        };
        Self {
            config,
            store,
            memory: MemoryStore::new(),
            redis,
        }
    }

    /// Build a limiter using the store selected from the environment.
    pub fn from_env(config: RateLimitConfig) -> Self {
        Self::with_store(config, RateLimitStore::from_env())
    }

    /// Check a single key against the configured quota.
    pub async fn check(&self, key: &str) -> Result<(), u64> {
        match (&self.store, &self.redis) {
            (RateLimitStore::Redis { .. }, Some(redis)) => {
                redis
                    .check(key, self.config.max_requests, self.config.window_secs)
                    .await
            }
            _ => {
                self.memory
                    .check(key, self.config.max_requests, self.config.window_secs)
                    .await
            }
        }
    }
}

fn rate_limit_response(retry_after: u64, message: &str) -> Response {
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header("Retry-After", retry_after.to_string())
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "error": "rate_limit_exceeded",
                "message": message
            }))
            .unwrap(),
        ))
        .unwrap()
}

pub async fn rate_limit_middleware(
    State(limiter): State<RateLimiter>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let ip = extract_client_ip(req.headers());

    if let Err(retry_after) = limiter.check(&ip).await {
        return rate_limit_response(retry_after, "Too many requests");
    }

    next.run(req).await
}

fn extract_client_ip(headers: &axum::http::HeaderMap) -> String {
    if let Some(val) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        return val
            .split(',')
            .next()
            .unwrap_or("unknown")
            .trim()
            .to_string();
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

    if let Err(retry_after) = limiter.check(&vault_id).await {
        return rate_limit_response(
            retry_after,
            "Check-in rate limit exceeded. Please wait before checking in again.",
        );
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_store_allows_within_quota_and_blocks_over() {
        let store = MemoryStore::new();
        assert!(store.check("k", 2, 60).await.is_ok());
        assert!(store.check("k", 2, 60).await.is_ok());
        assert_eq!(store.check("k", 2, 60).await, Err(60));
    }

    #[tokio::test]
    async fn memory_store_keys_are_independent() {
        let store = MemoryStore::new();
        assert!(store.check("a", 1, 60).await.is_ok());
        assert!(store.check("b", 1, 60).await.is_ok());
        assert_eq!(store.check("a", 1, 60).await, Err(60));
    }

    #[tokio::test]
    async fn memory_store_resets_after_window() {
        let store = MemoryStore::new();
        assert!(store.check("k", 1, 0).await.is_ok());
        // window_secs = 0 means the window is always expired.
        assert!(store.check("k", 1, 0).await.is_ok());
    }

    #[tokio::test]
    async fn limiter_defaults_to_memory_store() {
        let limiter = RateLimiter::new(RateLimitConfig::new(1, 60));
        assert!(limiter.check("ip").await.is_ok());
        assert_eq!(limiter.check("ip").await, Err(60));
    }

    #[tokio::test]
    async fn redis_store_fails_open_when_unreachable() {
        // Point at an unreachable Redis; the store must fail open so a Redis
        // outage does not take down the API.
        let store = RedisStore::new("redis://127.0.0.1:1").expect("client builds");
        assert!(store.check("k", 1, 60).await.is_ok());
    }

    #[test]
    fn store_from_env_defaults_to_memory() {
        std::env::remove_var("RATE_LIMIT_STORE");
        assert_eq!(RateLimitStore::from_env(), RateLimitStore::Memory);
    }
}
