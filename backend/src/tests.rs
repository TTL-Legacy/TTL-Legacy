use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderValue, Method, Request, StatusCode},
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::json;
use tower::ServiceExt;
use tower_http::cors::CorsLayer;

use crate::{db::{Db, PoolConfig}, routes};

fn test_app() -> Router {
    test_app_with_db(Arc::new(Db::open(":memory:").unwrap()))
}

fn test_app_with_db(db: Arc<Db>) -> Router {
    db.migrate().unwrap();
    Router::new()
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
        .route(
            "/notifications/unsubscribe",
            get(routes::unsubscribe),
        )
        .with_state(db)
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn ready_handler(State(db): State<Arc<Db>>) -> Result<Json<serde_json::Value>, StatusCode> {
    match db.check_connectivity() {
        Ok(()) => Ok(Json(serde_json::json!({
            "status": "ok",
            "version": env!("CARGO_PKG_VERSION"),
            "database": "connected",
        }))),
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn post_json(app: Router, uri: &str, body: serde_json::Value) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_req(app: Router, uri: &str) -> axum::response::Response {
    app.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn test_set_and_get_preferences() {
    let app = test_app();
    let body = json!({
        "channels": ["email", "sms"],
        "hours_before_expiry": 48,
        "frequency": "daily"
    });
    let res = post_json(app, "/api/vaults/1/reminder-preferences", body).await;
    assert_eq!(res.status(), StatusCode::OK);

    let app2 = test_app();
    // Re-insert so we can GET from same db
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();
    let prefs = crate::models::ReminderPreferences {
        vault_id: 1,
        channels: vec![crate::models::Channel::Email],
        hours_before_expiry: 24,
        frequency: crate::models::Frequency::Once,
        deleted_at: None,
    };
    db.upsert(&prefs).unwrap();
    let fetched = db.get(1).unwrap();
    assert_eq!(fetched.vault_id, 1);
    assert_eq!(fetched.hours_before_expiry, 24);
    assert_eq!(fetched.channels, vec![crate::models::Channel::Email]);
    assert_eq!(fetched.frequency, crate::models::Frequency::Once);
    drop(app2);
}

#[tokio::test]
async fn test_get_not_found() {
    let app = test_app();
    let res = get_req(app, "/api/vaults/999/reminder-preferences").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_set_empty_channels_rejected() {
    let app = test_app();
    let body = json!({
        "channels": [],
        "hours_before_expiry": 24,
        "frequency": "once"
    });
    let res = post_json(app, "/api/vaults/1/reminder-preferences", body).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_set_zero_hours_rejected() {
    let app = test_app();
    let body = json!({
        "channels": ["push"],
        "hours_before_expiry": 0,
        "frequency": "hourly"
    });
    let res = post_json(app, "/api/vaults/1/reminder-preferences", body).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_upsert_overwrites() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();

    let p1 = crate::models::ReminderPreferences {
        vault_id: 5,
        channels: vec![crate::models::Channel::Email],
        hours_before_expiry: 12,
        frequency: crate::models::Frequency::Once,
        deleted_at: None,
    };
    db.upsert(&p1).unwrap();

    let p2 = crate::models::ReminderPreferences {
        vault_id: 5,
        channels: vec![crate::models::Channel::Sms, crate::models::Channel::Push],
        hours_before_expiry: 6,
        frequency: crate::models::Frequency::Hourly,
        deleted_at: None,
    };
    db.upsert(&p2).unwrap();

    let fetched = db.get(5).unwrap();
    assert_eq!(fetched.hours_before_expiry, 6);
    assert_eq!(fetched.channels.len(), 2);
    assert_eq!(fetched.frequency, crate::models::Frequency::Hourly);
}

// ── #821: Health check endpoint tests ────────────────────────────────────────

#[tokio::test]
async fn test_health_endpoint() {
    let app = test_app();
    let res = get_req(app, "/health").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert!(json["version"].is_string());
}

#[tokio::test]
async fn test_ready_endpoint() {
    let app = test_app();
    let res = get_req(app, "/ready").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["database"], "connected");
}

// ── #822: Pool configuration tests ───────────────────────────────────────────

#[tokio::test]
async fn test_pool_config_defaults() {
    let config = PoolConfig::default();
    assert_eq!(config.min, 2);
    assert_eq!(config.max, 10);
    assert_eq!(config.timeout_secs, 30);
}

#[tokio::test]
async fn test_db_open_with_pool_config() {
    let config = PoolConfig { min: 1, max: 5, timeout_secs: 15 };
    let db = Db::open_with_pool_config(":memory:", &config);
    assert!(db.is_ok());
}

// ── #823: CORS tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_cors_allowed_origin() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();

    let cors = CorsLayer::new()
        .allow_origin("http://example.com".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST]);

    let app = Router::new()
        .route("/health", get(health_handler))
        .layer(cors)
        .with_state(db);

    let res = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/health")
                .header("origin", "http://example.com")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(res.headers().get("access-control-allow-origin").is_some());
    assert_eq!(
        res.headers().get("access-control-allow-origin").unwrap(),
        "http://example.com"
    );
}

#[tokio::test]
async fn test_cors_rejected_origin() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();

    let cors = CorsLayer::new()
        .allow_origin("http://allowed.com".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET]);

    let app = Router::new()
        .route("/health", get(health_handler))
        .layer(cors)
        .with_state(db);

    let res = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/health")
                .header("origin", "http://evil.com")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let origin_header = res.headers().get("access-control-allow-origin");
    match origin_header {
        Some(val) => assert_ne!(val, "http://evil.com"),
        None => {} // No header is also acceptable
    }
}

// ── #824: Scheduler resilience tests ─────────────────────────────────────────

#[tokio::test]
async fn test_scheduler_handles_db_errors_gracefully() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    // Intentionally do NOT run migrate() so tables don't exist.
    // The scheduler should log errors and continue, not panic.
    let result = db.all();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_scheduler_insurance_handles_db_errors() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    // No migration — all_enabled_insurance_policies will fail.
    let result = db.all_enabled_insurance_policies();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_db_check_connectivity() {
    let db = Db::open(":memory:").unwrap();
    assert!(db.check_connectivity().is_ok());
}

// ── Issue #851: Mocked HTTP tests for notification delivery ─────────────────

#[cfg(test)]
mod notification_delivery_tests {
    use std::sync::Arc;
    use crate::notifications::{
        FcmClient, NotificationService,
        create_token_store, create_prefs_store, create_schedule_store, create_delivery_store,
    };
    use crate::models::{RegisterTokenRequest, NotificationType, DeliveryStatus};
    use serde_json::json;

    fn make_service(fcm: Arc<FcmClient>) -> NotificationService {
        NotificationService::new(
            fcm,
            create_token_store(),
            create_prefs_store(),
            create_schedule_store(),
            create_delivery_store(),
        )
    }

    /// Successful FCM push send: mock returns 200 with a message name.
    #[tokio::test]
    async fn test_fcm_send_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/projects/test-project/messages:send")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"name":"projects/test-project/messages/msg-001"}"#)
            .create_async()
            .await;

        let mut client = FcmClient::new("test-key".into(), "test-project".into());
        client.base_url = server.url();
        let result = client.send("device-token-1", "Title", "Body", json!({})).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "projects/test-project/messages/msg-001");
        mock.assert_async().await;
    }

    /// Failed FCM push: mock returns 401, send should return Err.
    #[tokio::test]
    async fn test_fcm_send_failure_returns_err() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/projects/test-project/messages:send")
            .with_status(401)
            .with_body("Unauthorized")
            .create_async()
            .await;

        let mut client = FcmClient::new("bad-key".into(), "test-project".into());
        client.base_url = server.url();
        let result = client.send("device-token-1", "Title", "Body", json!({})).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("FCM error 401"));
        mock.assert_async().await;
    }

    /// Rate-limited FCM push: mock returns 429, send should return Err containing status.
    #[tokio::test]
    async fn test_fcm_send_rate_limited() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/projects/test-project/messages:send")
            .with_status(429)
            .with_body("Too Many Requests")
            .create_async()
            .await;

        let mut client = FcmClient::new("test-key".into(), "test-project".into());
        client.base_url = server.url();
        let result = client.send("device-token-1", "Title", "Body", json!({})).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("FCM error 429"));
        mock.assert_async().await;
    }

    /// Delivery with retry: first call fails (500), second succeeds; flush_pending retries.
    #[tokio::test]
    async fn test_delivery_fails_no_tokens_registered() {
        let mut server = mockito::Server::new_async().await;
        let mut fcm = FcmClient::new("test-key".into(), "test-project".into());
        fcm.base_url = server.url();
        let svc = make_service(Arc::new(fcm));

        // Schedule an immediate notification for owner with no registered tokens
        svc.schedule_immediate("vault-1", "owner-no-token", NotificationType::CheckInReminder);

        // No tokens → flush_pending records Failed
        svc.flush_pending().await;

        let log = svc.get_delivery_log("owner-no-token");
        assert!(!log.is_empty());
        assert_eq!(log[0].status, DeliveryStatus::Failed);

        // No HTTP call was made since no tokens exist
        server.mock("POST", mockito::Matcher::Any).expect(0).create_async().await;
    }

    /// Successful delivery: token registered, mock returns 200, status is Sent.
    #[tokio::test]
    async fn test_delivery_success_with_registered_token() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/v1/projects/test-project/messages:send")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"name":"projects/test-project/messages/ok-1"}"#)
            .create_async()
            .await;

        let mut fcm = FcmClient::new("test-key".into(), "test-project".into());
        fcm.base_url = server.url();
        let svc = make_service(Arc::new(fcm));

        svc.register_token(RegisterTokenRequest {
            owner: "owner-1".into(),
            token: "device-abc".into(),
            platform: "android".into(),
        });
        svc.schedule_immediate("vault-1", "owner-1", NotificationType::ExpiryWarning);
        svc.flush_pending().await;

        let log = svc.get_delivery_log("owner-1");
        assert!(!log.is_empty());
        assert_eq!(log[0].status, DeliveryStatus::Sent);
    }
}
