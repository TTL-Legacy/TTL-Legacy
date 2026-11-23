use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    body::Body,
    extract::{ConnectInfo, State},
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
/// `RATE_LIMIT_STORE=memory` forces per-process counters (local development,
/// single replica). `RATE_LIMIT_STORE=redis` shares counters across every
/// replica through `REDIS_URL`. When `RATE_LIMIT_STORE` is unset, Redis is used
/// automatically if `REDIS_URL` is set, because counters held in process memory
/// are independent per replica and make the limit ineffective (issue #2134).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RateLimitStore {
    Memory,
    Redis { url: String },
}

impl RateLimitStore {
    /// Resolve the store from the environment.
    ///
    /// | `RATE_LIMIT_STORE` | `REDIS_URL` | Result  |
    /// |--------------------|-------------|---------|
    /// | `memory`           | any         | Memory  |
    /// | `redis`            | set         | Redis   |
    /// | `redis`            | unset       | Memory (so a misconfiguration never disables rate limiting) |
    /// | unset              | set         | Redis   |
    /// | unset              | unset       | Memory  |
    pub fn from_env() -> Self {
        let redis_url = std::env::var("REDIS_URL").ok().filter(|u| !u.is_empty());
        match std::env::var("RATE_LIMIT_STORE").as_deref() {
            Ok("memory") => RateLimitStore::Memory,
            Ok("redis") | Err(_) => match redis_url {
                Some(url) => RateLimitStore::Redis { url },
                None => RateLimitStore::Memory,
            },
            Ok(_) => RateLimitStore::Memory,
        }
    }
}

/// What the limiter does when the Redis store is selected but unreachable.
///
/// Set with `RATE_LIMIT_FAIL_MODE`: `local` (default) or `deny`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RateLimitFailMode {
    /// Fall back to the per-process counter. Requests keep flowing, but limits
    /// are only enforced per replica until Redis recovers.
    #[default]
    Local,
    /// Reject the request (HTTP 429). Strictest option for security-sensitive
    /// deployments; an outage in Redis becomes an outage for rate-limited routes.
    Deny,
}

impl RateLimitFailMode {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "deny" | "closed" | "fail-closed" => RateLimitFailMode::Deny,
            _ => RateLimitFailMode::Local,
        }
    }

    pub fn from_env() -> Self {
        std::env::var("RATE_LIMIT_FAIL_MODE")
            .map(|v| Self::parse(&v))
            .unwrap_or_default()
    }
}

/// Upper bound for any single Redis round trip, so a hung Redis cannot stall
/// every request that passes through the limiter.
const REDIS_TIMEOUT: Duration = Duration::from_millis(250);

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
/// restarts. Uses a fixed-window counter keyed by `key`.
///
/// A single auto-reconnecting multiplexed connection is created lazily and
/// reused for every check, instead of opening a new TCP connection per request.
#[derive(Clone)]
pub struct RedisStore {
    client: redis::Client,
    conn: Arc<tokio::sync::OnceCell<redis::aio::ConnectionManager>>,
}

/// Increments the counter and sets its expiry in one atomic step, so a key can
/// never be left behind without a TTL.
const RATE_LIMIT_SCRIPT: &str = r#"
local count = redis.call('INCR', KEYS[1])
if count == 1 then
  redis.call('EXPIRE', KEYS[1], ARGV[1])
end
local ttl = redis.call('TTL', KEYS[1])
if ttl < 0 then
  redis.call('EXPIRE', KEYS[1], ARGV[1])
  ttl = tonumber(ARGV[1])
end
return {count, ttl}
"#;

impl RedisStore {
    pub fn new(url: &str) -> Result<Self, redis::RedisError> {
        Ok(Self {
            client: redis::Client::open(url)?,
            conn: Arc::new(tokio::sync::OnceCell::new()),
        })
    }

    fn redis_key(key: &str) -> String {
        format!("rate_limit:{key}")
    }

    async fn connection(&self) -> Option<redis::aio::ConnectionManager> {
        let client = self.client.clone();
        let init = self.conn.get_or_try_init(|| async move {
            redis::aio::ConnectionManager::new(client).await
        });
        match tokio::time::timeout(REDIS_TIMEOUT, init).await {
            Ok(Ok(conn)) => Some(conn.clone()),
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "rate limiter: cannot connect to Redis");
                None
            }
            Err(_) => {
                tracing::warn!("rate limiter: timed out connecting to Redis");
                None
            }
        }
    }

    /// Returns `None` when Redis cannot be reached, so the caller can apply the
    /// configured [`RateLimitFailMode`]. Otherwise returns `Some(Ok(()))` when
    /// the request is allowed, or `Some(Err(retry_after_secs))` when the quota
    /// is exhausted.
    pub async fn check(
        &self,
        key: &str,
        max_requests: u32,
        window_secs: u64,
    ) -> Option<Result<(), u64>> {
        let mut conn = self.connection().await?;
        let redis_key = Self::redis_key(key);

        let script = redis::Script::new(RATE_LIMIT_SCRIPT);
        let mut invocation = script.key(&redis_key);
        invocation.arg(window_secs);
        let reply = tokio::time::timeout(REDIS_TIMEOUT, invocation.invoke_async(&mut conn)).await;
        let (count, ttl): (u32, i64) = match reply {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "rate limiter: Redis script failed");
                return None;
            }
            Err(_) => {
                tracing::warn!("rate limiter: Redis script timed out");
                return None;
            }
        };

        if count > max_requests {
            let retry_after = if ttl > 0 { ttl as u64 } else { window_secs };
            Some(Err(retry_after))
        } else {
            Some(Ok(()))
        }
    }
}

/// A single trusted-proxy network expressed as an IP prefix length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrustedProxy {
    network: IpAddr,
    prefix_len: u8,
}

impl TrustedProxy {
    /// Parse a CIDR string such as `10.0.0.0/8` or `2001:db8::/32`.
    /// A bare IP (no `/`) is treated as a single-address network.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        let (addr_part, prefix_part) = match s.split_once('/') {
            Some((a, p)) => (a, Some(p)),
            None => (s, None),
        };
        let network: IpAddr = addr_part.parse().ok()?;
        let max_len = match network {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        let prefix_len = match prefix_part {
            Some(p) => {
                let len: u8 = p.parse().ok()?;
                if len > max_len {
                    return None;
                }
                len
            }
            None => max_len,
        };
        Some(Self {
            network,
            prefix_len,
        })
    }

    /// Returns true when `ip` falls inside this network.
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(net), IpAddr::V4(ip)) => {
                let mask = if self.prefix_len == 0 {
                    0
                } else {
                    u32::MAX << (32 - self.prefix_len as u32)
                };
                (u32::from(net) & mask) == (u32::from(ip) & mask)
            }
            (IpAddr::V6(net), IpAddr::V6(ip)) => {
                let mask = if self.prefix_len == 0 {
                    0
                } else {
                    u128::MAX << (128 - self.prefix_len as u32)
                };
                (u128::from(net) & mask) == (u128::from(ip) & mask)
            }
            _ => false,
        }
    }
}

/// Parse a comma-separated list of trusted proxy CIDRs (e.g. from the
/// `TRUSTED_PROXIES` env var). Invalid entries are ignored.
pub fn parse_trusted_proxies(list: &str) -> Vec<TrustedProxy> {
    list.split(',')
        .filter_map(TrustedProxy::parse)
        .collect()
}

/// Resolve the trusted-proxy list from the environment.
///
/// `TRUSTED_PROXIES` is a comma-separated list of CIDRs (e.g.
/// `10.0.0.0/8,172.16.0.0/12`). When unset or empty, no proxy is trusted and
/// forwarding headers are ignored.
pub fn trusted_proxies_from_env() -> Vec<TrustedProxy> {
    match std::env::var("TRUSTED_PROXIES") {
        Ok(list) => parse_trusted_proxies(&list),
        Err(_) => Vec::new(),
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    pub config: RateLimitConfig,
    store: RateLimitStore,
    memory: MemoryStore,
    redis: Option<RedisStore>,
    fail_mode: RateLimitFailMode,
    trusted_proxies: Arc<Vec<TrustedProxy>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self::with_store(config, RateLimitStore::Memory)
    }

    pub fn with_store(config: RateLimitConfig, store: RateLimitStore) -> Self {
        Self::with_store_and_proxies(config, store, Vec::new())
    }

    /// Build a limiter with an explicit trusted-proxy list.
    pub fn with_store_and_proxies(
        config: RateLimitConfig,
        store: RateLimitStore,
        trusted_proxies: Vec<TrustedProxy>,
    ) -> Self {
        let redis = match &store {
            RateLimitStore::Redis { url } => RedisStore::new(url).ok(),
            RateLimitStore::Memory => None,
        };
        Self {
            config,
            store,
            memory: MemoryStore::new(),
            redis,
            fail_mode: RateLimitFailMode::default(),
            trusted_proxies: Arc::new(trusted_proxies),
        }
    }

    /// Choose what happens when Redis is selected but unreachable.
    #[must_use]
    pub fn with_fail_mode(mut self, fail_mode: RateLimitFailMode) -> Self {
        self.fail_mode = fail_mode;
        self
    }

    /// Build a limiter using the store selected from the environment.
    pub fn from_env(config: RateLimitConfig) -> Self {
        Self::with_store_and_proxies(
            config,
            RateLimitStore::from_env(),
            trusted_proxies_from_env(),
        )
        .with_fail_mode(RateLimitFailMode::from_env())
    }

    /// Returns true when `ip` is one of the configured trusted proxies.
    pub fn is_trusted_proxy(&self, ip: IpAddr) -> bool {
        self.trusted_proxies.iter().any(|p| p.contains(ip))
    }

    /// Check a single key against the configured quota.
    ///
    /// With the Redis store the counter is shared by every replica. If Redis is
    /// selected but unreachable, the configured [`RateLimitFailMode`] decides
    /// between the local per-process counter (default) and rejecting the
    /// request.
    pub async fn check(&self, key: &str) -> Result<(), u64> {
        if matches!(self.store, RateLimitStore::Redis { .. }) {
            let outcome = match &self.redis {
                Some(redis) => {
                    redis
                        .check(key, self.config.max_requests, self.config.window_secs)
                        .await
                }
                None => None,
            };
            match outcome {
                Some(result) => return result,
                None => {
                    tracing::warn!(
                        fail_mode = ?self.fail_mode,
                        "rate limiter: Redis unavailable; applying fail mode"
                    );
                    if self.fail_mode == RateLimitFailMode::Deny {
                        return Err(self.config.window_secs);
                    }
                }
            }
        }
        self.memory
            .check(key, self.config.max_requests, self.config.window_secs)
            .await
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
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let ip = extract_client_ip(&limiter, peer, req.headers());

    if let Err(retry_after) = limiter.check(&ip).await {
        return rate_limit_response(retry_after, "Too many requests");
    }

    next.run(req).await
}

/// Determine the client IP for rate-limiting purposes.
///
/// Forwarding headers (`X-Forwarded-For`, `X-Real-IP`) are only honoured when
/// the immediate peer (`peer`) is a configured trusted proxy. Otherwise the
/// peer address is used, so a client cannot spoof its identity by sending
/// these headers directly.
///
/// When the peer is trusted, `X-Forwarded-For` is walked from right to left
/// (the rightmost entry is the one appended by the nearest proxy) and the
/// first address that is *not* a trusted proxy is returned. If every entry is
/// trusted, the leftmost address is used. `X-Real-IP` is used as a fallback
/// when `X-Forwarded-For` is absent.
fn extract_client_ip(
    limiter: &RateLimiter,
    peer: SocketAddr,
    headers: &axum::http::HeaderMap,
) -> String {
    let peer_ip = peer.ip();

    if !limiter.is_trusted_proxy(peer_ip) {
        return peer_ip.to_string();
    }

    if let Some(val) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        let hops: Vec<IpAddr> = val
            .split(',')
            .filter_map(|s| s.trim().parse::<IpAddr>().ok())
            .collect();
        if !hops.is_empty() {
            // Walk right-to-left, skipping trusted proxies, and return the
            // first untrusted hop (the real client).
            for hop in hops.iter().rev() {
                if !limiter.is_trusted_proxy(*hop) {
                    return hop.to_string();
                }
            }
            // All hops are trusted; fall back to the leftmost.
            return hops[0].to_string();
        }
    }

    if let Some(val) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        if let Ok(ip) = val.trim().parse::<IpAddr>() {
            return ip.to_string();
        }
    }

    peer_ip.to_string()
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
    async fn redis_store_returns_none_when_unreachable() {
        // Point at an unreachable Redis; the store reports "unavailable" so the
        // limiter can fall back to the local counter.
        let store = RedisStore::new("redis://127.0.0.1:1").expect("client builds");
        assert!(store.check("k", 1, 60).await.is_none());
    }

    #[tokio::test]
    async fn limiter_falls_back_to_memory_when_redis_unreachable() {
        let limiter = RateLimiter::with_store(
            RateLimitConfig::new(1, 60),
            RateLimitStore::Redis {
                url: "redis://127.0.0.1:1".to_string(),
            },
        );
        assert!(limiter.check("ip").await.is_ok());
        assert_eq!(limiter.check("ip").await, Err(60));
    }

    #[tokio::test]
    async fn limiter_denies_when_redis_unreachable_in_deny_mode() {
        let limiter = RateLimiter::with_store(
            RateLimitConfig::new(5, 60),
            RateLimitStore::Redis {
                url: "redis://127.0.0.1:1".to_string(),
            },
        )
        .with_fail_mode(RateLimitFailMode::Deny);
        assert_eq!(limiter.check("ip").await, Err(60));
    }

    #[test]
    fn fail_mode_parse() {
        assert_eq!(RateLimitFailMode::parse("deny"), RateLimitFailMode::Deny);
        assert_eq!(RateLimitFailMode::parse(" Closed "), RateLimitFailMode::Deny);
        assert_eq!(RateLimitFailMode::parse("local"), RateLimitFailMode::Local);
        assert_eq!(RateLimitFailMode::parse("garbage"), RateLimitFailMode::Local);
    }

    #[test]
    fn redis_key_is_namespaced() {
        assert_eq!(RedisStore::redis_key("1.2.3.4"), "rate_limit:1.2.3.4");
    }

    /// Environment-variable cases live in one test because the process
    /// environment is shared between tests running in parallel.
    #[test]
    fn store_from_env_selection() {
        std::env::remove_var("RATE_LIMIT_STORE");
        std::env::remove_var("REDIS_URL");
        assert_eq!(RateLimitStore::from_env(), RateLimitStore::Memory);

        std::env::set_var("REDIS_URL", "redis://cache:6379");
        let redis = RateLimitStore::Redis {
            url: "redis://cache:6379".to_string(),
        };
        // Unset RATE_LIMIT_STORE + REDIS_URL => shared Redis counters.
        assert_eq!(RateLimitStore::from_env(), redis);

        std::env::set_var("RATE_LIMIT_STORE", "redis");
        assert_eq!(RateLimitStore::from_env(), redis);

        // An explicit opt-out still wins.
        std::env::set_var("RATE_LIMIT_STORE", "memory");
        assert_eq!(RateLimitStore::from_env(), RateLimitStore::Memory);

        // redis requested but no URL: never disable limiting.
        std::env::set_var("RATE_LIMIT_STORE", "redis");
        std::env::remove_var("REDIS_URL");
        assert_eq!(RateLimitStore::from_env(), RateLimitStore::Memory);

        std::env::remove_var("RATE_LIMIT_STORE");
    }

    /// Acceptance test for issue #2134: two limiter instances (standing in for
    /// two backend replicas) must share one counter.
    ///
    /// Requires a running Redis: `REDIS_URL=redis://127.0.0.1:6379 cargo test
    /// -p ttl-legacy-backend -- --ignored shared_counter`.
    #[tokio::test]
    #[ignore = "requires a running Redis (set REDIS_URL)"]
    async fn shared_counter_across_limiter_instances() {
        let url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
        let store = RateLimitStore::Redis { url };
        let replica_a = RateLimiter::with_store(RateLimitConfig::new(2, 30), store.clone())
            .with_fail_mode(RateLimitFailMode::Deny);
        let replica_b = RateLimiter::with_store(RateLimitConfig::new(2, 30), store)
            .with_fail_mode(RateLimitFailMode::Deny);
        let key = format!("test-{}", uuid::Uuid::new_v4());

        assert!(replica_a.check(&key).await.is_ok());
        assert!(replica_b.check(&key).await.is_ok());
        // Third request lands on either replica and must be rejected.
        assert!(replica_a.check(&key).await.is_err());
        assert!(replica_b.check(&key).await.is_err());
    }

    use axum::http::HeaderMap;

    fn limiter_with_proxies(cidrs: &str) -> RateLimiter {
        RateLimiter::with_store_and_proxies(
            RateLimitConfig::new(10, 60),
            RateLimitStore::Memory,
            parse_trusted_proxies(cidrs),
        )
    }

    fn headers_with(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (k, v) in pairs {
            headers.insert(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                axum::http::HeaderValue::from_str(v).unwrap(),
            );
        }
        headers
    }

    #[test]
    fn trusted_proxy_parse_and_contains() {
        let p = TrustedProxy::parse("10.0.0.0/8").unwrap();
        assert!(p.contains("10.1.2.3".parse().unwrap()));
        assert!(!p.contains("11.0.0.1".parse().unwrap()));

        let single = TrustedProxy::parse("192.168.1.1").unwrap();
        assert!(single.contains("192.168.1.1".parse().unwrap()));
        assert!(!single.contains("192.168.1.2".parse().unwrap()));

        let v6 = TrustedProxy::parse("2001:db8::/32").unwrap();
        assert!(v6.contains("2001:db8::1".parse().unwrap()));
        assert!(!v6.contains("2001:db9::1".parse().unwrap()));

        assert!(TrustedProxy::parse("not-an-ip").is_none());
        assert!(TrustedProxy::parse("10.0.0.0/33").is_none());
    }

    #[test]
    fn direct_client_ignores_forwarding_headers() {
        // Peer is not a trusted proxy, so spoofed headers must be ignored.
        let limiter = limiter_with_proxies("10.0.0.0/8");
        let peer: SocketAddr = "203.0.113.5:1234".parse().unwrap();
        let headers = headers_with(&[
            ("x-forwarded-for", "1.2.3.4"),
            ("x-real-ip", "5.6.7.8"),
        ]);
        assert_eq!(extract_client_ip(&limiter, peer, &headers), "203.0.113.5");
    }

    #[test]
    fn single_proxy_uses_forwarded_for() {
        let limiter = limiter_with_proxies("10.0.0.0/8");
        let peer: SocketAddr = "10.0.0.1:443".parse().unwrap();
        let headers = headers_with(&[("x-forwarded-for", "198.51.100.7")]);
        assert_eq!(extract_client_ip(&limiter, peer, &headers), "198.51.100.7");
    }

    #[test]
    fn single_proxy_falls_back_to_x_real_ip() {
        let limiter = limiter_with_proxies("10.0.0.0/8");
        let peer: SocketAddr = "10.0.0.1:443".parse().unwrap();
        let headers = headers_with(&[("x-real-ip", "198.51.100.9")]);
        assert_eq!(extract_client_ip(&limiter, peer, &headers), "198.51.100.9");
    }

    #[test]
    fn multi_proxy_returns_leftmost_untrusted_hop() {
        // Client -> proxy A (trusted) -> proxy B (trusted) -> our peer (trusted).
        // XFF accumulates: client, proxyA. The rightmost untrusted hop is the
        // client.
        let limiter = limiter_with_proxies("10.0.0.0/8");
        let peer: SocketAddr = "10.0.0.2:443".parse().unwrap();
        let headers = headers_with(&[("x-forwarded-for", "198.51.100.7, 10.0.0.1")]);
        assert_eq!(extract_client_ip(&limiter, peer, &headers), "198.51.100.7");
    }

    #[test]
    fn multi_proxy_skips_spoofed_leftmost_entry() {
        // A malicious client prepends a fake IP; the trusted proxy appends the
        // real client IP. Walking right-to-left must return the real client.
        let limiter = limiter_with_proxies("10.0.0.0/8");
        let peer: SocketAddr = "10.0.0.2:443".parse().unwrap();
        let headers = headers_with(&[("x-forwarded-for", "1.2.3.4, 198.51.100.7, 10.0.0.1")]);
        assert_eq!(extract_client_ip(&limiter, peer, &headers), "198.51.100.7");
    }

    #[test]
    fn no_trusted_proxies_always_uses_peer() {
        let limiter = limiter_with_proxies("");
        let peer: SocketAddr = "10.0.0.1:443".parse().unwrap();
        let headers = headers_with(&[("x-forwarded-for", "198.51.100.7")]);
        assert_eq!(extract_client_ip(&limiter, peer, &headers), "10.0.0.1");
    }
}
