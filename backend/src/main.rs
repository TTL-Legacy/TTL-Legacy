use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use tracing_subscriber::EnvFilter;

mod db;
mod error;
mod models;
mod routes;
mod scheduler;

#[cfg(test)]
mod tests;

pub use db::Db;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let db = Arc::new(Db::open(":memory:").expect("failed to open db"));
    db.migrate().expect("migration failed");

    let scheduler_db = Arc::clone(&db);
    tokio::spawn(async move {
        scheduler::run(scheduler_db).await;
    });

    let app = Router::new()
        .route(
            "/api/vaults/:vault_id/reminder-preferences",
            post(routes::set_preferences).get(routes::get_preferences),
        )
        .with_state(db);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
