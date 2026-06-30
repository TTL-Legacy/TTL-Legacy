use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderValue, Method, StatusCode},
    routing::{delete, get, post},
    Json, Router,
};
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

mod db;
mod error;
mod handlers;
mod models;
mod routes;
mod scheduler;

#[cfg(test)]
mod tests;

pub use db::Db;
pub use db::AppState;

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
    }))
}

async fn ready_handler(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.db.check_connectivity() {
        Ok(()) => Ok(Json(serde_json::json!({
            "status": "ok",
            "version": env!("CARGO_PKG_VERSION"),
            "database": "connected",
        }))),
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let pool_config = db::PoolConfig::from_env();
    tracing::info!(
        min = pool_config.min,
        max = pool_config.max,
        timeout_secs = pool_config.timeout_secs,
        "database pool configuration"
    );

    let db = Arc::new(Db::open_with_pool_config(":memory:", &pool_config).expect("failed to open db"));
    db.migrate().expect("migration failed");

    let scheduler_db = Arc::clone(&db);
    tokio::spawn(async move {
        scheduler::run(scheduler_db).await;
    });

    let state = Arc::new(AppState {
        db: Arc::clone(&db),
        vault_store: db::create_vault_store(),
        event_store: db::create_event_store(),
        audit_store: db::create_audit_store(),
        share_store: db::create_share_store(),
        share_token_store: db::create_share_token_store(),
    });

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route(
            "/api/vaults/:vault_id/reminder-preferences",
            post(routes::set_preferences)
                .get(routes::get_preferences)
                .delete(routes::delete_preferences),
        )
        .route(
            "/api/vaults/:vault_id/reminders",
            get(routes::list_vault_reminders),
        )
        // Vault sharing endpoints
        .route(
            "/api/vaults/:vault_id/share",
            post(routes::share_vault),
        )
        .route(
            "/api/vaults/:vault_id/shares",
            get(routes::list_vault_shares),
        )
        .route(
            "/api/vaults/:vault_id/share/tokens",
            post(routes::generate_share_token).get(routes::list_share_tokens),
        )
        .route(
            "/api/vaults/:vault_id/share/tokens/revoke",
            post(routes::revoke_share_token),
        )
        // Read-only shared vault access (no auth required — uses token)
        .route(
            "/api/shared/vaults/{token}",
            get(routes::access_shared_vault),
        )
        .route(
            "/api/shared/vaults/{token}/export",
            get(routes::access_shared_vault_export),
        )
        .layer(build_cors_layer())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
