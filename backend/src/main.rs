use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{FromRef, State},
    http::{HeaderValue, Method, StatusCode},
    middleware,
    routing::{delete, get, post},
    Json, Router,
};
use tower_http::cors::CorsLayer;

mod consensus;
mod db;
mod error;
mod handlers;
mod models;
mod notifications;
mod otel;
mod rate_limit;
mod routes;
mod scheduler;
mod two_factor;
mod escalation;
mod webhook_retry;

#[cfg(test)]
mod tests;

pub use consensus::NodeCache;
pub use db::Db;
pub use db::AppState;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub consensus: Arc<NodeCache>,
}

impl FromRef<AppState> for Arc<Db> {
    fn from_ref(state: &AppState) -> Arc<Db> {
        Arc::clone(&state.db)
    }
}

fn build_cors_layer() -> CorsLayer {
    let allowed_origins = std::env::var("ALLOWED_ORIGINS").unwrap_or_default();
    if allowed_origins.is_empty() {
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

#[tokio::main]
async fn main() {
    // Initialise OpenTelemetry distributed tracing.
    // Spans are exported to the OTLP endpoint configured via
    // OTEL_EXPORTER_OTLP_ENDPOINT (default: http://localhost:4317).
    // Issue #1145: Add OpenTelemetry Distributed Tracing to Backend
    let _otel_guard = otel::init_tracer("ttl-legacy-backend");

    // Check contract version before proceeding with server startup
    let min_contract_version = parse_min_contract_version(std::env::var("MIN_CONTRACT_VERSION").ok());

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

    let db = Arc::new(Db::open_with_pool_config(":memory:", &pool_config).expect("failed to open db"));
    db.migrate().expect("migration failed");

    let consensus = NodeCache::from_env();
    tracing::info!(
        node_id = consensus.node_id(),
        strategy = ?consensus.strategy(),
        "consensus cache initialized"
    );

    let scheduler_db = Arc::clone(&db);
    tokio::spawn(async move {
        scheduler::run(scheduler_db).await;
    });

    let state = AppState {
        db,
        consensus,
    };

    let global_limiter = rate_limit::RateLimiter::new(rate_limit::RateLimitConfig::new(100, 60));
    let checkin_limiter = rate_limit::RateLimiter::new(rate_limit::RateLimitConfig::new(10, 60));
    let release_limiter = rate_limit::RateLimiter::new(rate_limit::RateLimitConfig::new(5, 60));
    let email_token_limiter = rate_limit::RateLimiter::new(rate_limit::RateLimitConfig::new(3, 60));
    let sensitive_limiter = rate_limit::RateLimiter::new(rate_limit::RateLimitConfig::new(20, 60));

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/health/consensus", get(consensus_health_handler))
        .route("/ready", get(ready_handler))
        .route(
            "/api/vaults/:vault_id/reminder-preferences",
            post(routes::set_preferences).layer(middleware::from_fn_with_state(sensitive_limiter.clone(), rate_limit::rate_limit_middleware))
                .get(routes::get_preferences)
                .delete(routes::delete_preferences),
        )
        .route(
            "/api/vaults/:vault_id/subscriptions",
            post(routes::set_subscription).layer(middleware::from_fn_with_state(sensitive_limiter.clone(), rate_limit::rate_limit_middleware))
                .delete(routes::delete_subscription),
        )
        .route(
            "/api/vaults/:vault_id/reminders",
            get(routes::list_vault_reminders),
        )
        .route(
            "/api/vaults/:vault_id/simulate-release",
            get(routes::simulate_release),
        )
        .route(
            "/api/vaults/:vault_id/sponsored-release",
            post(routes::create_sponsored_release).layer(middleware::from_fn_with_state(sensitive_limiter, rate_limit::rate_limit_middleware))
                .get(routes::get_sponsored_releases),
        )
        .route(
            "/api/vaults/:vault_id/vesting/claim-bonus",
            post(routes::claim_vesting_bonus),
        )
        .route(
            "/api/vaults/:vault_id/vesting/bonus",
            get(routes::get_vesting_bonus),
        )
        .route(
            "/api/vaults/:vault_id/withdrawal-alert-preferences",
            get(routes::get_withdrawal_alert_prefs)
                .put(routes::set_withdrawal_alert_prefs)
                .delete(routes::delete_withdrawal_alert_prefs),
        )
        .layer(build_cors_layer())
        .layer(middleware::from_fn_with_state(global_limiter, rate_limit::rate_limit_middleware))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
