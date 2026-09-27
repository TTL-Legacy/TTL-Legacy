use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{FromRef, State},
    http::{HeaderValue, Method, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;

mod auth;
mod consensus;
mod csrf;
mod db;
mod error;
mod escalation;
mod handlers;
mod models;
mod notifications;
mod otel;
mod rate_limit;
mod request_id;
mod routes;
mod sanitization;
mod scheduler;
mod security_headers;
mod two_factor;
mod webhook_retry;

#[cfg(test)]
mod tests;

pub use consensus::NodeCache;
pub use db::Db;
// Note: db::AppState is NOT re-exported here — main.rs defines its own AppState
// that includes the Metrics field (issue #1195).

use crate::metrics::Metrics;
use crate::rate_limit::{InMemoryRateLimitStore, RateLimitStore, RedisRateLimitStore};

/// Default grace period for draining in-flight background work on shutdown.
const SHUTDOWN_DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub consensus: Arc<NodeCache>,
    pub metrics: Arc<Metrics>,
    /// Shared shutdown token signalled on SIGTERM/SIGINT (issue #1488).
    pub shutdown: CancellationToken,
    /// Rate-limit store backend (issue #1494). Defaults to in-memory for dev;
    /// set `RATE_LIMIT_STORE=redis` (with `REDIS_URL`) to share state across
    /// replicas and survive restarts.
    pub rate_limit_store: Arc<dyn RateLimitStore>,
}

impl FromRef<AppState> for Arc<Db> {
    fn from_ref(state: &AppState) -> Arc<Db> {
        Arc::clone(&state.db)
    }
}

impl FromRef<AppState> for CancellationToken {
    fn from_ref(state: &AppState) -> CancellationToken {
        state.shutdown.clone()
    }
}

impl FromRef<AppState> for Arc<dyn RateLimitStore> {
    fn from_ref(state: &AppState) -> Arc<dyn RateLimitStore> {
        Arc::clone(&state.rate_limit_store)
    }
}

/// Builds the rate-limit store from environment configuration (issue #1494).
///
/// | `RATE_LIMIT_STORE` | `REDIS_URL` | Result                                  |
/// |--------------------|-------------|-----------------------------------------|
/// | unset / `memory`   | any         | In-memory store (default, dev-friendly) |
/// | `redis`            | set         | Redis-backed store (shared, persistent) |
/// | `redis`            | unset       | Falls back to in-memory with a warning  |
fn build_rate_limit_store() -> Arc<dyn RateLimitStore> {
    let backend = std::env::var("RATE_LIMIT_STORE").unwrap_or_default();
    if backend.eq_ignore_ascii_case("redis") {
        match std::env::var("REDIS_URL") {
            Ok(url) if !url.is_empty() => {
                tracing::info!("rate limiter using Redis-backed store");
                return Arc::new(RedisRateLimitStore::new(url));
            }
            _ => {
                tracing::warn!(
                    "RATE_LIMIT_STORE=redis but REDIS_URL is unset; \
                     falling back to in-memory rate-limit store"
                );
            }
        }
    }
    tracing::info!("rate limiter using in-memory store");
    Arc::new(InMemoryRateLimitStore::new())
}

/// Builds the CORS layer based on `APP_ENV` and `ALLOWED_ORIGINS` environment variables.
///
/// # Behaviour
///
/// | `APP_ENV`                   | `ALLOWED_ORIGINS`  | Result                                              |
/// |-----------------------------|--------------------|----------------------------------------------------|
/// | unset **or** `development`  | any / empty        | `CorsLayer::permissive()` — wildcard, dev mode      |
/// | `production` / `staging`    | non-empty list     | Origin whitelist with `Vary: Origin` header         |
/// | `production` / `staging`    | empty              | `CorsLayer::new()` — blocks all cross-origin        |
///
/// Issue #1179: CORS Policy Hardening
fn build_cors_layer() -> CorsLayer {
    let app_env = std::env::var("APP_ENV").unwrap_or_default();
    let is_production = !app_env.is_empty() && app_env != "development";

    // In development (or when APP_ENV is unset), allow everything.
    if !is_production {
        return CorsLayer::permissive();
    }

    // Production / staging: honour the ALLOWED_ORIGINS whitelist.
    let allowed_origins = std::env::var("ALLOWED_ORIGINS").unwrap_or_default();
    if allowed_origins.is_empty() {
        // No origins configured → block all cross-origin requests.
        return CorsLayer::new();
    }

    let origins: Vec<HeaderValue> = allowed_origins
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(tower_http::cors::Any)
        // Instruct caches / CDNs that the response varies by origin.
        .vary([axum::http::header::ORIGIN])
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "db": "connected",
    }))
}

async fn ready_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.db.check_connectivity() {
        Ok(()) => Ok(Json(serde_json::json!({
            "status": "ok",
            "version": env!("CARGO_PKG_VERSION"),
            "database": "connected",
        }))),
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn consensus_health_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.consensus.check_and_resolve() {
        Ok(report) => {
            let status = if report.consistent { "ok" } else { "degraded" };
            Ok(Json(serde_json::json!({
                "status": status,
                "cache_consistent": report.consistent,
                "node_id": report.node_id,
                "strategy": report.strategy,
                "conflicts_detected": report.conflicts.len(),
                "conflicts_resolved": report.conflicts_resolved,
                "keys_checked": report.keys_checked,
            })))
        }
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

/// GET /metrics — Prometheus text exposition endpoint (issue #1195).
///
/// Returns all application metrics in Prometheus text format
/// (content-type: text/plain; version=0.0.4; charset=utf-8).
async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    let body = state.metrics.render();
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

/// Waits for SIGTERM (or SIGINT) and then signals the shared shutdown token.
///
/// Issue #1488: graceful shutdown for background workers.
async fn shutdown_signal(token: CancellationToken) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received SIGINT, starting graceful shutdown"),
        _ = terminate => tracing::info!("received SIGTERM, starting graceful shutdown"),
    }

    // Signal every background task sharing this token to stop accepting work
    // and drain in-flight jobs.
    token.cancel();
}

#[tokio::main]
async fn main() {
    // Initialise OpenTelemetry distributed tracing.
    // Spans are exported to the OTLP endpoint configured via
    // OTEL_EXPORTER_OTLP_ENDPOINT (default: http://localhost:4317).
    // Issue #1145: Add OpenTelemetry Distributed Tracing to Backend
    let _otel_guard = otel::init_tracer("ttl-legacy-backend");

    // Check contract version before proceeding with server startup
    let min_contract_version =
        parse_min_contract_version(std::env::var("MIN_CONTRACT_VERSION").ok());

    let version_result = check_contract_version(
        || async {
            // TODO: replace with real Soroban client call when available
            // For now, this is a stub that returns Ok(1) so startup proceeds
            Ok::<u32, String>(1)
        },
        min_contract_version,
    )
    .await;

    tracing::info!("{}", version_result);

    if let Some(err) = &version_result.error {
        tracing::error!("Contract version check failed: {}", err);
        std::process::exit(1);
    }

    if !version_result.compatible {
        tracing::error!("{}", version_result);
        std::process::exit(1);
    }

    let pool_config = db::PoolConfig::from_env();
    tracing::info!(
        min = pool_config.min,
        max = pool_config.max,
        timeout_secs = pool_config.timeout_secs,
        "database pool configuration"
    );

    let db =
        Arc::new(Db::open_with_pool_config(":memory:", &pool_config).expect("failed to open db"));
    db.migrate().expect("migration failed");

    let consensus = NodeCache::from_env();
    tracing::info!(
        node_id = consensus.node_id(),
        strategy = ?consensus.strategy(),
        "consensus cache initialized"
    );

    // Rate-limit store: in-memory by default, Redis when configured (issue #1494).
    let rate_limit_store = build_rate_limit_store();

    // Shared shutdown token: every background task observes this and drains
    // in-flight work when SIGTERM/SIGINT is received (issue #1488).
    let shutdown = CancellationToken::new();

    let state = AppState {
        db: Arc::clone(&db),
        consensus: Arc::new(consensus),
        metrics: Arc::new(Metrics::new()),
        shutdown: shutdown.clone(),
        rate_limit_store,
    };

    let app = routes::build_router(state.clone());

    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind address");

    tracing::info!("listening on {}", addr);

    let shutdown_for_server = shutdown.clone();
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        shutdown_signal(shutdown_for_server).await;
    });

    if let Err(err) = server.await {
        tracing::error!("server error: {}", err);
    }

    // Allow background workers to drain in-flight work before exiting.
    if tokio::time::timeout(SHUTDOWN_DRAIN_TIMEOUT, shutdown.cancelled())
        .await
        .is_err()
    {
        tracing::warn!("shutdown drain timed out");
    }

    tracing::info!("shutdown complete");
}
