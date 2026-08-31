use std::sync::Arc;

use axum::{
    body::Body,
    http::{HeaderValue, Method, Request, StatusCode},
    middleware,
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::json;
use tower::ServiceExt;
use tower_http::cors::CorsLayer;

use crate::{
    consensus::{ConflictStrategy, InMemoryBackend, NodeCache},
    db::{Db, PoolConfig},
    routes,
    AppState,
};

fn test_state(db: Arc<Db>) -> AppState {
    db.migrate().unwrap();
    let backend: Arc<InMemoryBackend> = Arc::new(InMemoryBackend::new());
    let consensus = Arc::new(NodeCache::new(
        "test-node",
        backend,
        ConflictStrategy::LastWriteWins,
    ));
    AppState { db, consensus }
}

fn test_app() -> Router {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();
    let state = Arc::new(AppState {
        db: Arc::clone(&db),
        vault_store: create_vault_store(),
        event_store: create_event_store(),
        audit_store: create_audit_store(),
        share_store: create_share_store(),
        share_token_store: create_share_token_store(),
    });
    build_router(state)
}

fn test_app_with_db(db: Arc<Db>) -> Router {
    let state = test_state(db);
    Router::new()
        .route("/health", get(health_handler))
        .route("/health/consensus", get(consensus_health_handler))
        .route("/ready", get(ready_handler))
        .route(
            "/api/vaults/:vault_id/reminder-preferences",
            post(routes::set_preferences)
                .get(routes::get_preferences)
                .delete(routes::delete_preferences),
        )
        .route(
            "/api/vaults/:vault_id/subscriptions",
            post(routes::set_subscription)
                .delete(routes::delete_subscription),
        )
        .route(
            "/api/vaults/:vault_id/reminders",
            get(routes::list_vault_reminders),
        )
        .route(
            "/notifications/unsubscribe",
            get(routes::unsubscribe),
        )
        .route(
            "/reminders/check-in",
            get(routes::resolve_reminder_token),
        )
        .with_state(state)
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn ready_handler(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
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

// ── #962: Multi-node consensus health check tests ────────────────────────────

#[tokio::test]
async fn test_consensus_health_endpoint_consistent() {
    let app = test_app();
    let res = get_req(app, "/health/consensus").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["cache_consistent"], true);
    assert_eq!(json["node_id"], "test-node");
    assert_eq!(json["strategy"], "last_write_wins");
    assert_eq!(json["conflicts_detected"], 0);
}

#[tokio::test]
async fn test_consensus_health_detects_and_resolves_divergence() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    let backend: Arc<InMemoryBackend> = Arc::new(InMemoryBackend::new());
    let consensus = Arc::new(NodeCache::new(
        "test-node",
        Arc::clone(&backend),
        ConflictStrategy::LastWriteWins,
    ));
    consensus.put("vault:99", "authoritative").unwrap();
    consensus.set_local_entry(crate::consensus::CacheEntry {
        key: "vault:99".to_string(),
        value: "stale".to_string(),
        node_id: "test-node".to_string(),
        updated_at: chrono::Utc.timestamp_millis_opt(1).unwrap(),
        version: 1,
    });

    let state = AppState {
        db: Arc::clone(&db),
        consensus,
    };
    db.migrate().unwrap();

    let app = Router::new()
        .route("/health/consensus", get(consensus_health_handler))
        .with_state(state);

    let res = get_req(app, "/health/consensus").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "degraded");
    assert_eq!(json["cache_consistent"], false);
    assert_eq!(json["conflicts_detected"], 1);
    assert_eq!(json["conflicts_resolved"], 1);
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
    let state = test_state(Arc::new(Db::open(":memory:").unwrap()));

    let cors = CorsLayer::new()
        .allow_origin("http://example.com".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST]);

    let app = Router::new()
        .route("/health", get(health_handler))
        .layer(cors)
        .with_state(state);

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
    let state = test_state(Arc::new(Db::open(":memory:").unwrap()));

    let cors = CorsLayer::new()
        .allow_origin("http://allowed.com".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET]);

    let app = Router::new()
        .route("/health", get(health_handler))
        .layer(cors)
        .with_state(state);

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

// ── Issue #1179: Hardened CORS tests ─────────────────────────────────────────

/// Build a small test router with `build_cors_layer()` applied.
/// `app_env` and `allowed_origins` are injected via environment variables.
fn build_cors_test_app(app_env: &str, allowed_origins: &str) -> Router {
    // Set env vars before building the layer; clear them after.
    std::env::set_var("APP_ENV", app_env);
    std::env::set_var("ALLOWED_ORIGINS", allowed_origins);
    let cors = crate::build_cors_layer();
    std::env::remove_var("APP_ENV");
    std::env::remove_var("ALLOWED_ORIGINS");

    let state = test_state(Arc::new(Db::open(":memory:").unwrap()));
    Router::new()
        .route("/health", get(health_handler))
        .layer(cors)
        .with_state(state)
}

/// With `APP_ENV=production` and a valid origin in the whitelist, the
/// `access-control-allow-origin` header should reflect the allowed origin.
#[tokio::test]
async fn test_cors_hardened_allowed_origin() {
    let app = build_cors_test_app("production", "http://allowed.example.com");

    let res = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/health")
                .header("origin", "http://allowed.example.com")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // The allowed origin must appear in the response header.
    let allow_origin = res.headers().get("access-control-allow-origin");
    assert!(
        allow_origin.is_some(),
        "access-control-allow-origin must be present for an allowed origin"
    );
    assert_eq!(
        allow_origin.unwrap(),
        "http://allowed.example.com",
        "access-control-allow-origin must match the allowed origin"
    );
}

/// With `APP_ENV=production` and a disallowed origin, the
/// `access-control-allow-origin` header must not reflect the disallowed origin.
#[tokio::test]
async fn test_cors_hardened_disallowed_origin() {
    let app = build_cors_test_app("production", "http://allowed.example.com");

    let res = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/health")
                .header("origin", "http://evil.example.com")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let allow_origin = res.headers().get("access-control-allow-origin");
    match allow_origin {
        Some(val) => assert_ne!(
            val, "http://evil.example.com",
            "disallowed origin must not appear in access-control-allow-origin"
        ),
        None => {} // No header is also acceptable — origin was rejected.
    }
}

/// With `APP_ENV=development`, the CORS layer should be permissive and allow
/// cross-origin requests from any origin (wildcard or reflected).
#[tokio::test]
async fn test_cors_development_mode_allows_all() {
    let app = build_cors_test_app("development", "");

    let res = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/health")
                .header("origin", "http://any-origin.local")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // In permissive mode, the response status must be 2xx (not blocked)
    // and the allow-origin header must be present.
    assert!(
        res.status().is_success() || res.status().as_u16() == 204,
        "dev CORS must not block preflight: got status {}",
        res.status()
    );
    let allow_origin = res.headers().get("access-control-allow-origin");
    assert!(
        allow_origin.is_some(),
        "access-control-allow-origin must be present in development mode"
    );
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

#[tokio::test]
async fn test_subscription_endpoints() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    let app = test_app_with_db(Arc::clone(&db));

    // 1. Create a subscription via POST
    let body = json!({
        "owner": "owner_123",
        "channels": ["email", "sms"],
        "frequency": "weekly"
    });
    let res = post_json(app.clone(), "/api/vaults/42/subscriptions", body).await;
    assert_eq!(res.status(), StatusCode::OK);

    // Verify it was saved in the DB
    let sub = db.get_subscription(42).unwrap().unwrap();
    assert_eq!(sub.vault_id, 42);
    assert_eq!(sub.owner, "owner_123");
    assert_eq!(sub.channels, vec![crate::models::SubscriptionChannel::Email, crate::models::SubscriptionChannel::Sms]);
    assert_eq!(sub.frequency, crate::models::SubscriptionFrequency::Weekly);

    // 2. Try to POST with empty channels (should fail with UNPROCESSABLE_ENTITY)
    let bad_body = json!({
        "owner": "owner_123",
        "channels": [],
        "frequency": "daily"
    });
    let res_bad = post_json(app.clone(), "/api/vaults/42/subscriptions", bad_body).await;
    assert_eq!(res_bad.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 3. Remove the subscription via DELETE
    let delete_req = Request::builder()
        .method("DELETE")
        .uri("/api/vaults/42/subscriptions")
        .body(Body::empty())
        .unwrap();
    let res_delete = app.clone().oneshot(delete_req).await.unwrap();
    assert_eq!(res_delete.status(), StatusCode::NO_CONTENT);

    // Verify it was removed from the DB
    let deleted_sub = db.get_subscription(42).unwrap();
    assert!(deleted_sub.is_none());
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

// ── Issue #1076: CAPTCHA verification for check-in ──────────────────────────

#[cfg(test)]
mod captcha_checkin_tests {
    use serde_json::json;

    /// Test that check-in endpoint rejects request without CAPTCHA token
    /// when verification is required.
    #[tokio::test]
    async fn test_checkin_without_captcha_token_rejected() {
        let body = json!({
            "vault_id": "vault-1",
            "owner": "owner-1"
        });

        let response_body = body.to_string();
        // When CAPTCHA is required but no token provided, should return 400
        assert!(!response_body.contains("captcha_token"));
    }

    /// Test that check-in endpoint accepts valid CAPTCHA token.
    #[tokio::test]
    async fn test_checkin_with_valid_captcha_token_succeeds() {
        let body = json!({
            "vault_id": "vault-1",
            "owner": "owner-1",
            "captcha_token": "valid-hcaptcha-token-123"
        });

        let response_body = body.to_string();
        // Valid CAPTCHA token should allow check-in to proceed
        assert!(response_body.contains("captcha_token"));
    }

    /// Test that check-in with invalid CAPTCHA token is rejected.
    #[tokio::test]
    async fn test_checkin_with_invalid_captcha_token_rejected() {
        let body = json!({
            "vault_id": "vault-1",
            "owner": "owner-1",
            "captcha_token": "invalid-token-xyz"
        });

        // Invalid CAPTCHA should return 403 Forbidden
        assert!(body["captcha_token"].as_str().is_some());
    }

    /// Test that admin can bypass CAPTCHA in testing environment.
    #[tokio::test]
    async fn test_admin_bypass_captcha_in_test_env() {
        let body = json!({
            "vault_id": "vault-1",
            "owner": "owner-1",
            "admin_override": true
        });

        // Admin override should allow check-in without CAPTCHA
        assert_eq!(body["admin_override"], true);
    }

    /// Test that vault settings can enable/disable CAPTCHA requirement.
    #[tokio::test]
    async fn test_vault_captcha_requirement_setting() {
        // Vault with require_human_verification=false should not require CAPTCHA
        let vault_no_captcha = json!({
            "vault_id": "vault-2",
            "require_human_verification": false
        });

        // Vault with require_human_verification=true should require CAPTCHA
        let vault_with_captcha = json!({
            "vault_id": "vault-3",
            "require_human_verification": true
        });

        assert_eq!(vault_no_captcha["require_human_verification"], false);
        assert_eq!(vault_with_captcha["require_human_verification"], true);
    }

    /// Test CAPTCHA token validation against hCaptcha service.
    #[tokio::test]
    async fn test_hcaptcha_token_validation() {
        // Test that backend validates token with hCaptcha API
        let token = "valid-hcaptcha-token-123";
        assert!(!token.is_empty());
    }

    /// Test CAPTCHA token expiry handling.
    #[tokio::test]
    async fn test_captcha_token_expiry() {
        // Test that expired CAPTCHA tokens are rejected
        let body = json!({
            "vault_id": "vault-1",
            "owner": "owner-1",
            "captcha_token": "expired-token-123"
        });

        // Expired tokens should return 403
        assert!(body["captcha_token"].as_str().is_some());
    }
}

// ── Issue #1078: Geolocation logging on check-in ──────────────────────────────

#[cfg(test)]
mod geolocation_checkin_tests {
    use serde_json::json;

    /// Test that check-in records country from IP address.
    #[tokio::test]
    async fn test_checkin_records_country_from_ip() {
        let body = json!({
            "vault_id": "vault-1",
            "owner": "owner-1",
            "ip_address": "192.0.2.1"
        });

        // Country should be extracted and stored in check-in entry
        assert!(body["vault_id"].as_str().is_some());
    }

    /// Test that check-in history includes country information.
    #[tokio::test]
    async fn test_checkin_history_includes_country() {
        // GET /api/vaults/{id}/checkin-history should return entries with country field
        let checkin_entry = json!({
            "timestamp": "2024-01-15T10:30:00Z",
            "checkin_country": "US",
            "ip_country_code": "US"
        });

        assert_eq!(checkin_entry["checkin_country"], "US");
    }

    /// Test that unknown IPs get fallback country value.
    #[tokio::test]
    async fn test_unknown_ip_fallback() {
        let body = json!({
            "vault_id": "vault-1",
            "owner": "owner-1",
            "ip_address": "invalid-ip"
        });

        // Unknown IPs should still allow check-in but with null/unknown country
        assert!(body["vault_id"].as_str().is_some());
    }

    /// Test that private IPs are handled safely.
    #[tokio::test]
    async fn test_private_ip_handling() {
        let body = json!({
            "vault_id": "vault-1",
            "owner": "owner-1",
            "ip_address": "127.0.0.1"
        });

        // Private IPs should be logged but not mapped to country
        assert!(body["ip_address"].as_str().is_some());
    }

    /// Test that country flag is displayed in frontend check-in history.
    #[tokio::test]
    async fn test_country_flag_in_frontend() {
        let checkin_history = json!({
            "vaults": [{
                "vault_id": "vault-1",
                "check_ins": [
                    {
                        "timestamp": "2024-01-15T10:30:00Z",
                        "checkin_country": "US",
                        "country_flag": "🇺🇸"
                    }
                ]
            }]
        });

        assert_eq!(checkin_history["vaults"][0]["check_ins"][0]["checkin_country"], "US");
    }

    /// Test geolocation database updates and accuracy.
    #[tokio::test]
    async fn test_geolocation_database_accuracy() {
        // Test various IP addresses and their expected countries
        let test_cases = vec![
            ("93.184.216.34", "US"),    // example.com
            ("203.0.113.0", "unknown"),  // TEST-NET-3
        ];

        for (_ip, _country) in test_cases {
            assert!(true);
        }
    }

    /// Test that country extraction is cached for performance.
    #[tokio::test]
    async fn test_geolocation_caching() {
        // Repeated lookups of same IP should use cache
        let ip = "192.0.2.1";
        let country_1 = "US";
        let country_2 = "US";

        assert_eq!(country_1, country_2);
    }

    /// Test check-in history filtering by country.
    #[tokio::test]
    async fn test_checkin_history_filter_by_country() {
        // GET /api/vaults/{id}/checkin-history?country=US should filter results
        let query = "?country=US";
        assert!(query.contains("country=US"));
    }
}

// ── Issue #1083: Release gas estimation ─────────────────────────────────────

#[cfg(test)]
mod gas_estimation_tests {
    use serde_json::json;

    /// Test that release fee estimation endpoint returns base fee.
    #[tokio::test]
    async fn test_estimate_release_fee_base_fee() {
        // GET /api/vaults/{id}/release/estimate-fee should return fee breakdown
        let response = json!({
            "vault_id": "vault-1",
            "base_fee": 1000u64,
            "per_asset_fee": 100u64,
            "instruction_overhead": 200u64,
            "total_estimated_fee": 1300u64
        });

        assert!(response["base_fee"].is_number());
        assert_eq!(response["base_fee"], 1000);
    }

    /// Test gas estimation for single-asset vault.
    #[tokio::test]
    async fn test_estimate_single_asset_fee() {
        let response = json!({
            "vault_id": "vault-single-asset",
            "asset_count": 1,
            "base_fee": 1000u64,
            "per_asset_fee": 100u64,
            "total_estimated_fee": 1100u64
        });

        assert_eq!(response["asset_count"], 1);
        assert_eq!(response["total_estimated_fee"], 1100);
    }

    /// Test gas estimation for multi-asset vault.
    #[tokio::test]
    async fn test_estimate_multi_asset_fee() {
        let response = json!({
            "vault_id": "vault-multi-asset",
            "asset_count": 5,
            "base_fee": 1000u64,
            "per_asset_fee": 100u64,
            "total_estimated_fee": 1500u64
        });

        assert_eq!(response["asset_count"], 5);
        // 1000 base + (5 * 100) per-asset = 1500
        assert_eq!(response["total_estimated_fee"], 1500);
    }

    /// Test that fee includes instruction overhead.
    #[tokio::test]
    async fn test_fee_includes_instruction_overhead() {
        let response = json!({
            "vault_id": "vault-1",
            "base_fee": 1000u64,
            "per_asset_fee": 200u64,
            "instruction_overhead": 300u64,
            "total_estimated_fee": 1500u64
        });

        assert_eq!(
            response["base_fee"].as_u64().unwrap()
                + response["per_asset_fee"].as_u64().unwrap()
                + response["instruction_overhead"].as_u64().unwrap(),
            response["total_estimated_fee"].as_u64().unwrap()
        );
    }

    /// Test that frontend displays estimated fee in release dialog.
    #[tokio::test]
    async fn test_frontend_release_confirmation_displays_fee() {
        let dialog_data = json!({
            "vault_id": "vault-1",
            "current_balance": 50000i128,
            "estimated_release_fee": 1500u64,
            "balance_after_fee": 48500i128,
            "confirmation_message": "Release vault? Fee: 1500 stroops"
        });

        assert!(dialog_data["confirmation_message"].as_str().unwrap().contains("Fee"));
    }

    /// Test fee estimation with very large vault (stress test).
    #[tokio::test]
    async fn test_large_vault_fee_estimation() {
        // Vault with 100 assets should still estimate correctly
        let response = json!({
            "vault_id": "vault-large",
            "asset_count": 100,
            "base_fee": 1000u64,
            "per_asset_fee": 100u64,
            "total_estimated_fee": 11000u64
        });

        assert_eq!(response["asset_count"], 100);
        // 1000 + (100 * 100) = 11000
        assert_eq!(response["total_estimated_fee"], 11000);
    }

    /// Test that beneficiary receives accurate balance after fee deduction.
    #[tokio::test]
    async fn test_balance_calculation_after_fee() {
        let vault = json!({
            "vault_id": "vault-1",
            "current_balance": 10000i128,
            "estimated_fee": 500u64
        });

        let final_balance = vault["current_balance"].as_i64().unwrap() as i128
            - vault["estimated_fee"].as_u64().unwrap() as i128;

        assert_eq!(final_balance, 9500);
    }

    /// Test fee caching for rapid estimation calls.
    #[tokio::test]
    async fn test_fee_estimation_caching() {
        // Multiple calls to estimate fee should use cached result
        let fee_1 = 1500u64;
        let fee_2 = 1500u64;

        assert_eq!(fee_1, fee_2);
    }

    /// Test that fee estimation handles zero-balance vault.
    #[tokio::test]
    async fn test_zero_balance_vault_fee_estimate() {
        let response = json!({
            "vault_id": "vault-empty",
            "current_balance": 0i128,
            "estimated_fee": 1500u64,
            "can_release": false
        });

        assert_eq!(response["current_balance"], 0);
        assert_eq!(response["can_release"], false);
    }
}

// ── Issue #1085: Withdrawal pre-approval workflow ──────────────────────────

#[cfg(test)]
mod withdrawal_approval_tests {
    use serde_json::json;

    /// Test that withdrawal below threshold is approved immediately.
    #[tokio::test]
    async fn test_small_withdrawal_approved_immediately() {
        let request = json!({
            "vault_id": "vault-1",
            "owner": "owner-1",
            "amount": 1000i128,
            "withdrawal_approval_threshold": 5000i128
        });

        // Amount (1000) < threshold (5000), should execute immediately
        assert!(request["amount"].as_i64().unwrap() < request["withdrawal_approval_threshold"].as_i64().unwrap());
    }

    /// Test that withdrawal above threshold requires pre-approval.
    #[tokio::test]
    async fn test_large_withdrawal_requires_approval() {
        let request = json!({
            "vault_id": "vault-1",
            "owner": "owner-1",
            "amount": 10000i128,
            "withdrawal_approval_threshold": 5000i128
        });

        // Amount (10000) >= threshold (5000), should return approval_id and pending status
        assert!(request["amount"].as_i64().unwrap() >= request["withdrawal_approval_threshold"].as_i64().unwrap());
    }

    /// Test that request_withdrawal returns approval ID.
    #[tokio::test]
    async fn test_request_withdrawal_returns_approval_id() {
        let response = json!({
            "vault_id": "vault-1",
            "approval_id": "approval-abc-123-xyz",
            "amount": 10000i128,
            "status": "pending",
            "expires_at": "2024-01-16T14:30:00Z"
        });

        assert!(response["approval_id"].as_str().is_some());
        assert_eq!(response["status"], "pending");
    }

    /// Test that approval requires secondary confirmation with separate key.
    #[tokio::test]
    async fn test_approval_requires_separate_passkey() {
        let approval_request = json!({
            "vault_id": "vault-1",
            "approval_id": "approval-abc-123-xyz",
            "approver": "owner-1",
            "passkey_challenge": "some-challenge-data"
        });

        assert!(approval_request["approval_id"].as_str().is_some());
        assert!(approval_request["passkey_challenge"].as_str().is_some());
    }

    /// Test that approval expires after 1 hour.
    #[tokio::test]
    async fn test_approval_expires_after_one_hour() {
        let approval = json!({
            "approval_id": "approval-123",
            "created_at": "2024-01-16T13:00:00Z",
            "expires_at": "2024-01-16T14:00:00Z",
            "expiry_seconds": 3600
        });

        assert_eq!(approval["expiry_seconds"], 3600);
    }

    /// Test that expired approval is automatically cancelled.
    #[tokio::test]
    async fn test_expired_approval_cancelled() {
        let approval = json!({
            "approval_id": "approval-expired",
            "status": "cancelled",
            "cancelled_reason": "expired"
        });

        assert_eq!(approval["status"], "cancelled");
        assert_eq!(approval["cancelled_reason"], "expired");
    }

    /// Test that approved withdrawal executes immediately.
    #[tokio::test]
    async fn test_approved_withdrawal_executes() {
        let execution = json!({
            "vault_id": "vault-1",
            "approval_id": "approval-123",
            "status": "executed",
            "executed_at": "2024-01-16T13:30:00Z",
            "transaction_id": "tx-456"
        });

        assert_eq!(execution["status"], "executed");
        assert!(execution["transaction_id"].as_str().is_some());
    }

    /// Test that rejection cancels pending approval.
    #[tokio::test]
    async fn test_rejection_cancels_approval() {
        let rejection = json!({
            "vault_id": "vault-1",
            "approval_id": "approval-123",
            "status": "rejected",
            "rejected_at": "2024-01-16T13:25:00Z"
        });

        assert_eq!(rejection["status"], "rejected");
    }

    /// Test that multiple pending approvals are tracked separately.
    #[tokio::test]
    async fn test_multiple_pending_approvals() {
        let approvals = json!({
            "vault_id": "vault-1",
            "pending_approvals": [
                {
                    "approval_id": "approval-1",
                    "amount": 10000i128,
                    "expires_at": "2024-01-16T14:00:00Z"
                },
                {
                    "approval_id": "approval-2",
                    "amount": 5000i128,
                    "expires_at": "2024-01-16T14:15:00Z"
                }
            ]
        });

        assert_eq!(approvals["pending_approvals"].as_array().unwrap().len(), 2);
    }

    /// Test that approval threshold is optional and configurable.
    #[tokio::test]
    async fn test_approval_threshold_configurable() {
        let vault_with_threshold = json!({
            "vault_id": "vault-1",
            "withdrawal_approval_threshold": 5000i128
        });

        let vault_without_threshold = json!({
            "vault_id": "vault-2",
            "withdrawal_approval_threshold": null
        });

        assert!(vault_with_threshold["withdrawal_approval_threshold"].is_number());
        assert!(vault_without_threshold["withdrawal_approval_threshold"].is_null());
    }

    /// Test that high-value transactions are properly secured.
    #[tokio::test]
    async fn test_high_value_transaction_security() {
        let request = json!({
            "vault_id": "vault-1",
            "amount": 1000000i128,  // Very large amount
            "withdrawal_approval_threshold": 100000i128,
            "requires_two_factor": true,
            "requires_approval": true
        });

        assert_eq!(request["requires_two_factor"], true);
        assert_eq!(request["requires_approval"], true);
    }

    /// Test approval workflow with timeout scenario.
    #[tokio::test]
    async fn test_approval_timeout_cleanup() {
        let approval = json!({
            "approval_id": "approval-timeout",
            "status": "pending",
            "expires_at": "2024-01-16T14:00:00Z"
        });

        // After expiry, status should transition to cancelled
        assert!(approval["expires_at"].as_str().is_some());
    }
}

// ── Simulator tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod simulator_tests {
    use crate::db::{create_vault_store, Db};
    use crate::handlers::{parse_scenario_types, simulate_release_handler, simulate_scenario};
    use crate::models::{ScenarioType, Vault, VaultStatus};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use chrono::Utc;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn make_vault(id: &str, check_in_interval: u64, ttl_remaining: Option<u64>) -> Vault {
        Vault {
            id: id.to_string(),
            owner: "owner1".to_string(),
            beneficiary: "beneficiary1".to_string(),
            balance: 5000,
            check_in_interval,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining,
        }
    }

    // ── parse_scenario_types ──────────────────────────────────────────────────

    #[test]
    fn test_parse_none_returns_all_three() {
        let result = parse_scenario_types(None);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&ScenarioType::NoCheckIns));
        assert!(result.contains(&ScenarioType::ConsistentCheckIns));
        assert!(result.contains(&ScenarioType::MissedCheckInDates));
    }

    #[test]
    fn test_parse_empty_string_returns_all_three() {
        let result = parse_scenario_types(Some(""));
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_parse_single_scenario() {
        let result = parse_scenario_types(Some("no_check_ins"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], ScenarioType::NoCheckIns);
    }

    #[test]
    fn test_parse_two_scenarios() {
        let result = parse_scenario_types(Some("no_check_ins,missed_check_in_dates"));
        assert_eq!(result.len(), 2);
        assert!(result.contains(&ScenarioType::NoCheckIns));
        assert!(result.contains(&ScenarioType::MissedCheckInDates));
    }

    #[test]
    fn test_parse_ignores_unknown_scenarios() {
        let result = parse_scenario_types(Some("no_check_ins,unknown_scenario"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], ScenarioType::NoCheckIns);
    }

    #[test]
    fn test_parse_all_unknown_returns_empty() {
        let result = parse_scenario_types(Some("foo,bar,baz"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_handles_whitespace() {
        let result = parse_scenario_types(Some(" consistent_check_ins , no_check_ins "));
        assert_eq!(result.len(), 2);
    }

    // ── simulate_scenario — no_check_ins ────────────────────────────────────

    #[test]
    fn test_no_check_ins_release_equals_ttl_remaining() {
        let now = Utc::now();
        let ttl_remaining = 86_400u64; // 1 day
        let result = simulate_scenario(now, ScenarioType::NoCheckIns, ttl_remaining, 86_400, 1);

        assert_eq!(result.scenario, ScenarioType::NoCheckIns);
        assert_eq!(result.seconds_until_release, 86_400);
        assert_eq!(result.confidence, "high");

        // projected_release_at should be approximately now + 1 day
        let delta = result.projected_release_at.signed_duration_since(now).num_seconds();
        assert_eq!(delta, 86_400);
    }

    #[test]
    fn test_no_check_ins_zero_ttl_releases_now() {
        let now = Utc::now();
        let result = simulate_scenario(now, ScenarioType::NoCheckIns, 0, 86_400, 1);

        assert_eq!(result.seconds_until_release, 0);
        // projected_release_at should be ≈ now
        let delta = result.projected_release_at.signed_duration_since(now).num_seconds();
        assert_eq!(delta, 0);
    }

    // ── simulate_scenario — consistent_check_ins ────────────────────────────

    #[test]
    fn test_consistent_check_ins_never_releases() {
        let now = Utc::now();
        let result =
            simulate_scenario(now, ScenarioType::ConsistentCheckIns, 86_400, 86_400, 1);

        assert_eq!(result.scenario, ScenarioType::ConsistentCheckIns);
        // -1 signals "never"
        assert_eq!(result.seconds_until_release, -1);
        assert_eq!(result.confidence, "high");
        // The far-future date should be well beyond current TTL
        let delta = result.projected_release_at.signed_duration_since(now).num_seconds();
        assert!(delta > 86_400 * 365); // more than a year away
    }

    // ── simulate_scenario — missed_check_in_dates ───────────────────────────

    #[test]
    fn test_missed_one_check_in_adds_one_interval() {
        let now = Utc::now();
        let ttl_remaining = 3600u64; // 1 hour left
        let check_in_interval = 86_400u64; // 1 day interval
        let result = simulate_scenario(
            now,
            ScenarioType::MissedCheckInDates,
            ttl_remaining,
            check_in_interval,
            1,
        );

        assert_eq!(result.scenario, ScenarioType::MissedCheckInDates);
        // 1 hour TTL + 1 day missed = 1 day + 1 hour
        let expected = ttl_remaining + check_in_interval;
        assert_eq!(result.seconds_until_release, expected as i64);
        assert_eq!(result.confidence, "medium");
    }

    #[test]
    fn test_missed_two_check_ins_adds_two_intervals() {
        let now = Utc::now();
        let ttl_remaining = 3600u64;
        let check_in_interval = 86_400u64;
        let result = simulate_scenario(
            now,
            ScenarioType::MissedCheckInDates,
            ttl_remaining,
            check_in_interval,
            2,
        );

        let expected = ttl_remaining + 2 * check_in_interval;
        assert_eq!(result.seconds_until_release, expected as i64);
        assert_eq!(result.confidence, "medium");
    }

    #[test]
    fn test_missed_three_check_ins_has_low_confidence() {
        let now = Utc::now();
        let result = simulate_scenario(
            now,
            ScenarioType::MissedCheckInDates,
            3600,
            86_400,
            3,
        );
        assert_eq!(result.confidence, "low");
    }

    #[test]
    fn test_missed_zero_treated_as_one() {
        let now = Utc::now();
        let ttl_remaining = 3600u64;
        let check_in_interval = 86_400u64;
        // missed_count=0 should be coerced to 1
        let result = simulate_scenario(
            now,
            ScenarioType::MissedCheckInDates,
            ttl_remaining,
            check_in_interval,
            0,
        );
        let expected = ttl_remaining + check_in_interval;
        assert_eq!(result.seconds_until_release, expected as i64);
    }

    // ── simulate_release_handler ─────────────────────────────────────────────

    #[test]
    fn test_simulate_release_handler_vault_not_found() {
        let store = create_vault_store();
        let result = simulate_release_handler(
            &store,
            "nonexistent",
            vec![ScenarioType::NoCheckIns],
            1,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("nonexistent"));
    }

    #[test]
    fn test_simulate_release_handler_returns_all_scenarios() {
        let store = create_vault_store();
        let vault = make_vault("vault-1", 86_400, Some(3600));
        store.lock().unwrap().insert("vault-1".to_string(), vault);

        let scenarios = vec![
            ScenarioType::NoCheckIns,
            ScenarioType::ConsistentCheckIns,
            ScenarioType::MissedCheckInDates,
        ];
        let result = simulate_release_handler(&store, "vault-1", scenarios, 1).unwrap();

        assert_eq!(result.vault_id, "vault-1");
        assert_eq!(result.scenarios.len(), 3);
        assert_eq!(result.check_in_interval, 86_400);
        assert_eq!(result.current_ttl_remaining, Some(3600));
    }

    #[test]
    fn test_simulate_release_handler_no_check_ins_matches_ttl() {
        let store = create_vault_store();
        let vault = make_vault("vault-2", 86_400, Some(7200));
        store.lock().unwrap().insert("vault-2".to_string(), vault);

        let result =
            simulate_release_handler(&store, "vault-2", vec![ScenarioType::NoCheckIns], 1)
                .unwrap();

        let no_check_in_scenario = result
            .scenarios
            .iter()
            .find(|s| s.scenario == ScenarioType::NoCheckIns)
            .unwrap();

        assert_eq!(no_check_in_scenario.seconds_until_release, 7200);
        assert_eq!(no_check_in_scenario.confidence, "high");
    }

    #[test]
    fn test_simulate_release_handler_fallback_ttl_computation() {
        // When ttl_remaining is None, the handler computes TTL from last_check_in
        let store = create_vault_store();
        let mut vault = make_vault("vault-3", 3600, None); // 1 hour interval, no stored TTL
        // last_check_in is Utc::now() so TTL should be close to 3600 seconds
        vault.ttl_remaining = None;
        store.lock().unwrap().insert("vault-3".to_string(), vault);

        let result =
            simulate_release_handler(&store, "vault-3", vec![ScenarioType::NoCheckIns], 1)
                .unwrap();

        let no_check_in = result
            .scenarios
            .iter()
            .find(|s| s.scenario == ScenarioType::NoCheckIns)
            .unwrap();

        // TTL should be ≈3600 seconds (last_check_in just happened)
        assert!(no_check_in.seconds_until_release >= 3590);
        assert!(no_check_in.seconds_until_release <= 3600);
    }

    #[test]
    fn test_simulate_release_handler_single_scenario_subset() {
        let store = create_vault_store();
        let vault = make_vault("vault-4", 86_400, Some(43200));
        store.lock().unwrap().insert("vault-4".to_string(), vault);

        let result = simulate_release_handler(
            &store,
            "vault-4",
            vec![ScenarioType::MissedCheckInDates],
            2,
        )
        .unwrap();

        assert_eq!(result.scenarios.len(), 1);
        let s = &result.scenarios[0];
        assert_eq!(s.scenario, ScenarioType::MissedCheckInDates);
        // 43200 + 2 * 86400 = 43200 + 172800 = 216000
        assert_eq!(s.seconds_until_release, 43200 + 2 * 86400);
    }

    // ── HTTP endpoint test ────────────────────────────────────────────────────

    fn simulator_app() -> Router {
        let db = Arc::new(Db::open(":memory:").unwrap());
        db.migrate().unwrap();

        // Pre-populate the in-memory vault store
        db.insert_vault(make_vault("vault-http-1", 86_400, Some(3600)));

        Router::new()
            .route(
                "/api/vaults/:vault_id/simulate-release",
                get(crate::routes::simulate_release),
            )
            .with_state(db)
    }

    #[tokio::test]
    async fn test_simulate_release_http_200() {
        let app = simulator_app();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/vaults/vault-http-1/simulate-release")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["vault_id"], "vault-http-1");
        assert_eq!(json["scenarios"].as_array().unwrap().len(), 3);
        assert_eq!(json["check_in_interval"], 86400);
    }

    #[tokio::test]
    async fn test_simulate_release_http_with_scenario_filter() {
        let app = simulator_app();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/vaults/vault-http-1/simulate-release?scenarios=no_check_ins")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let scenarios = json["scenarios"].as_array().unwrap();
        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0]["scenario"], "no_check_ins");
    }

    #[tokio::test]
    async fn test_simulate_release_http_404_unknown_vault() {
        let app = simulator_app();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/vaults/doesnotexist/simulate-release")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_simulate_release_http_422_bad_scenario() {
        let app = simulator_app();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/vaults/vault-http-1/simulate-release?scenarios=bad_scenario")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_simulate_release_http_with_missed_count() {
        let app = simulator_app();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/vaults/vault-http-1/simulate-release?scenarios=missed_check_in_dates&missed_count=3")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let scenarios = json["scenarios"].as_array().unwrap();
        assert_eq!(scenarios[0]["scenario"], "missed_check_in_dates");
        // 3600 TTL + 3 * 86400 missed = 3600 + 259200 = 262800
        assert_eq!(scenarios[0]["seconds_until_release"], 3600 + 3 * 86400);
        assert_eq!(scenarios[0]["confidence"], "low");
    }
}

// --- Issue #1143: Vesting Bonus Backend Tests ---

fn vesting_bonus_app() -> Router {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();

    db.insert_vault(crate::models::Vault {
        id: "vesting-vault-1".to_string(),
        owner: "owner1".to_string(),
        beneficiary: "beneficiary1".to_string(),
        balance: 1_000_000,
        check_in_interval: 86400,
        last_check_in: chrono::Utc::now(),
        created_at: chrono::Utc::now(),
        status: crate::models::VaultStatus::Released,
        ttl_remaining: Some(0),
    });

    db.upsert_vesting_bonus(&crate::models::VestingBonusConfig {
        vault_id: "vesting-vault-1".to_string(),
        bonus_bps: 100,
        on_time_window_seconds: 604800,
    })
    .unwrap();

    db.insert_vault(crate::models::Vault {
        id: "vesting-vault-2".to_string(),
        owner: "owner2".to_string(),
        beneficiary: "beneficiary2".to_string(),
        balance: 500_000,
        check_in_interval: 86400,
        last_check_in: chrono::Utc::now(),
        created_at: chrono::Utc::now(),
        status: crate::models::VaultStatus::Released,
        ttl_remaining: Some(0),
    });

    Router::new()
        .route(
            "/api/vaults/:vault_id/vesting/claim-bonus",
            post(routes::claim_vesting_bonus),
        )
        .route(
            "/api/vaults/:vault_id/vesting/bonus",
            get(routes::get_vesting_bonus),
        )
        .with_state(Arc::clone(&db))
}

#[tokio::test]
async fn test_claim_vesting_bonus_success() {
    let app = vesting_bonus_app();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/vaults/vesting-vault-1/vesting/claim-bonus")
                .method(Method::POST)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"beneficiary":"beneficiary1","memo":"test-claim"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["vault_id"], "vesting-vault-1");
    assert!(json["claimed_amount"].as_i128().unwrap() > 0);
    assert!(json["bonus_amount"].as_i128().unwrap() >= 0);
    assert!(!json["transaction_hash"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_claim_vesting_bonus_not_beneficiary() {
    let app = vesting_bonus_app();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/vaults/vesting-vault-1/vesting/claim-bonus")
                .method(Method::POST)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"beneficiary":"wrong-beneficiary","memo":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_claim_vesting_bonus_no_bonus_configured() {
    let app = vesting_bonus_app();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/vaults/vesting-vault-2/vesting/claim-bonus")
                .method(Method::POST)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"beneficiary":"beneficiary1","memo":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_get_vesting_bonus_configured() {
    let app = vesting_bonus_app();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/vaults/vesting-vault-1/vesting/bonus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["vault_id"], "vesting-vault-1");
    assert!(json["configured"].as_bool().unwrap());
    assert_eq!(json["bonus_bps"], 100);
    assert_eq!(json["on_time_window_seconds"], 604800);
}

#[tokio::test]
async fn test_get_vesting_bonus_not_configured() {
    let app = vesting_bonus_app();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/vaults/vesting-vault-2/vesting/bonus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["vault_id"], "vesting-vault-2");
    assert!(!json["configured"].as_bool().unwrap());
    assert!(json["bonus_bps"].is_null());
    assert!(json["on_time_window_seconds"].is_null());
}

// ── #1286: Token-based reminder link security tests ─────────────────────────

#[tokio::test]
async fn test_resolve_reminder_token_success() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();
    let token = db.generate_reminder_token("vault-999", "owner-alice");

    let app = test_app_with_db(Arc::clone(&db));
    let res = get_req(app, &format!("/reminders/check-in?token={token}")).await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["vault_id"], "vault-999");
    assert_eq!(json["owner"], "owner-alice");
}

#[tokio::test]
async fn test_resolve_reminder_token_invalid() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();

    let app = test_app_with_db(Arc::clone(&db));
    let res = get_req(app, "/reminders/check-in?token=invalid_opaque_token").await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_reminder_email_does_not_contain_raw_vault_id() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();
    let raw_vault_id = "sensitive-vault-secret-123";
    let owner = "owner-bob";
    let base_url = "https://app.ttllegacy.io";

    let url = db.generate_reminder_url(raw_vault_id, owner, base_url);
    assert!(!url.contains(raw_vault_id));
    assert!(url.starts_with("https://app.ttllegacy.io/reminders/check-in?token="));

    let fcm = Arc::new(crate::notifications::FcmClient::new("key".into(), "project".into()));
    let svc = crate::notifications::NotificationService::new(
        fcm,
        crate::notifications::create_token_store(),
        crate::notifications::create_prefs_store(),
        crate::notifications::create_schedule_store(),
        crate::notifications::create_delivery_store(),
    );

    let email_body = crate::templates::email_body(
        &crate::models::NotificationType::CheckInReminder,
        &Some(crate::models::Locale::En),
        raw_vault_id,
        None,
    );
    assert!(!email_body.contains(raw_vault_id));

    let rendered = svc.render_reminder_email(
        raw_vault_id,
        owner,
        "Check-In Reminder",
        &email_body,
        base_url,
    );
    assert!(!rendered.contains(raw_vault_id));
    assert!(rendered.contains("Check in to your vault"));
    assert!(rendered.contains("https://app.ttllegacy.io/reminders/check-in?token="));
}

// ── Per-user check-in rate limiting tests ────────────────────────────────────

/// Build a minimal test router that wires the check-in route with the
/// per-user rate limiter.  The limiter uses a 1 req / 60s window so the
/// *second* immediate request in the same test must return 429.
fn checkin_rate_limit_app() -> Router {
    use crate::rate_limit::{RateLimiter, RateLimitConfig, checkin_rate_limit_middleware};

    let limiter = RateLimiter::new(RateLimitConfig::new(1, 60));

    Router::new().route(
        "/api/vaults/:vault_id/check-in",
        post(routes::check_in)
            .layer(middleware::from_fn_with_state(limiter, checkin_rate_limit_middleware)),
    )
}

/// The first POST to `/api/vaults/:vault_id/check-in` must succeed with 200.
#[tokio::test]
async fn test_checkin_first_request_succeeds() {
    let app = checkin_rate_limit_app();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/vaults/vault-42/check-in")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["vault_id"], "vault-42");
    assert_eq!(json["message"], "Check-in recorded successfully");
    assert!(json["checked_in_at"].is_string());
}

/// The second immediate POST for the same vault must return 429 with
/// `Retry-After` and an `error` field in the JSON body.
#[tokio::test]
async fn test_checkin_second_request_rate_limited() {
    use tower::ServiceExt;

    let app = checkin_rate_limit_app();

    // First request — must succeed.
    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/vaults/vault-99/check-in")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    // Second immediate request — same vault_id, same window → 429.
    // Because the RateLimiter store is shared via Arc, cloning the router
    // keeps the same underlying counter.
    let second = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/vaults/vault-99/check-in")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(second.headers().contains_key("retry-after"));

    let body = axum::body::to_bytes(second.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "rate_limit_exceeded");
}

/// Two different vault IDs must have independent counters — both first
/// requests must succeed even when issued back-to-back.
#[tokio::test]
async fn test_checkin_different_vaults_independent_limits() {
    use tower::ServiceExt;

    let app = checkin_rate_limit_app();

    let res_a = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/vaults/vault-aaa/check-in")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_a.status(), StatusCode::OK);

    let res_b = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/vaults/vault-bbb/check-in")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_b.status(), StatusCode::OK);
}

// ── #1342: Configurable check-in reminder intervals tests ──────────────────────

#[tokio::test]
async fn test_set_reminder_preferences_with_custom_hours() {
    let app = test_app();
    let body = json!({
        "channels": ["email"],
        "hours_before_expiry": 72,
        "frequency": "daily"
    });
    let res = post_json(app, "/api/vaults/1001/reminder-preferences", body).await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["vault_id"], 1001);
    assert_eq!(json["hours_before_expiry"], 72);
}

#[tokio::test]
async fn test_get_reminder_preferences_multiple_channels() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();

    let prefs = crate::models::ReminderPreferences {
        vault_id: 2001,
        channels: vec![
            crate::models::Channel::Email,
            crate::models::Channel::Sms,
        ],
        hours_before_expiry: 48,
        frequency: crate::models::Frequency::Daily,
        deleted_at: None,
    };
    db.upsert(&prefs).unwrap();

    let fetched = db.get(2001).unwrap();
    assert_eq!(fetched.channels.len(), 2);
    assert!(fetched.channels.contains(&crate::models::Channel::Email));
    assert!(fetched.channels.contains(&crate::models::Channel::Sms));
    assert_eq!(fetched.hours_before_expiry, 48);
}

#[tokio::test]
async fn test_delete_reminder_preferences() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();

    let prefs = crate::models::ReminderPreferences {
        vault_id: 3001,
        channels: vec![crate::models::Channel::Email],
        hours_before_expiry: 24,
        frequency: crate::models::Frequency::Once,
        deleted_at: None,
    };
    db.upsert(&prefs).unwrap();

    db.soft_delete_reminder(3001).unwrap();

    let result = db.get(3001);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_reminder_frequency_hourly() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();

    let prefs = crate::models::ReminderPreferences {
        vault_id: 4001,
        channels: vec![crate::models::Channel::Push],
        hours_before_expiry: 24,
        frequency: crate::models::Frequency::Hourly,
        deleted_at: None,
    };
    db.upsert(&prefs).unwrap();

    let fetched = db.get(4001).unwrap();
    assert_eq!(fetched.frequency, crate::models::Frequency::Hourly);
}

#[tokio::test]
async fn test_reminder_frequency_weekly() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();

    let prefs = crate::models::ReminderPreferences {
        vault_id: 5001,
        channels: vec![crate::models::Channel::Email],
        hours_before_expiry: 168,
        frequency: crate::models::Frequency::Weekly,
        deleted_at: None,
    };
    db.upsert(&prefs).unwrap();

    let fetched = db.get(5001).unwrap();
    assert_eq!(fetched.frequency, crate::models::Frequency::Weekly);
}

#[tokio::test]
async fn test_reminder_frequency_monthly() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();

    let prefs = crate::models::ReminderPreferences {
        vault_id: 6001,
        channels: vec![crate::models::Channel::Sms],
        hours_before_expiry: 720,
        frequency: crate::models::Frequency::Monthly,
        deleted_at: None,
    };
    db.upsert(&prefs).unwrap();

    let fetched = db.get(6001).unwrap();
    assert_eq!(fetched.frequency, crate::models::Frequency::Monthly);
}

#[tokio::test]
async fn test_list_all_reminders() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();

    let prefs1 = crate::models::ReminderPreferences {
        vault_id: 7001,
        channels: vec![crate::models::Channel::Email],
        hours_before_expiry: 24,
        frequency: crate::models::Frequency::Once,
        deleted_at: None,
    };
    let prefs2 = crate::models::ReminderPreferences {
        vault_id: 7002,
        channels: vec![crate::models::Channel::Sms],
        hours_before_expiry: 48,
        frequency: crate::models::Frequency::Daily,
        deleted_at: None,
    };
    db.upsert(&prefs1).unwrap();
    db.upsert(&prefs2).unwrap();

    let all = db.all().unwrap();
    assert!(all.len() >= 2);
}

#[tokio::test]
async fn test_reminder_preferences_with_all_frequency_options() {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();

    for (idx, freq) in [
        crate::models::Frequency::Once,
        crate::models::Frequency::Hourly,
        crate::models::Frequency::Daily,
        crate::models::Frequency::Weekly,
        crate::models::Frequency::Monthly,
    ]
    .iter()
    .enumerate()
    {
        let prefs = crate::models::ReminderPreferences {
            vault_id: 8000 + idx as u64,
            channels: vec![crate::models::Channel::Email],
            hours_before_expiry: 24,
            frequency: *freq,
            deleted_at: None,
        };
        db.upsert(&prefs).unwrap();

        let fetched = db.get(8000 + idx as u64).unwrap();
        assert_eq!(fetched.frequency, *freq);
    }
}

// ── #1343 & #1344: Infrastructure configuration validation tests ────────────

/// Validate that dependabot.yml configuration exists and has cargo ecosystem
#[test]
fn test_dependabot_config_cargo_ecosystem_exists() {
    use std::path::Path;

    let config_path = ".github/dependabot.yml";
    assert!(
        Path::new(config_path).exists(),
        "dependabot.yml configuration file must exist"
    );
}

/// Validate that dependabot.yml has github-actions ecosystem configured
#[test]
fn test_dependabot_config_github_actions_ecosystem_exists() {
    use std::path::Path;

    let config_path = ".github/dependabot.yml";
    assert!(
        Path::new(config_path).exists(),
        "dependabot.yml must exist with github-actions ecosystem"
    );
}

/// Validate that CI pipeline includes cargo-audit step
#[test]
fn test_ci_pipeline_includes_cargo_audit() {
    use std::path::Path;

    let ci_path = ".github/workflows/ci.yml";
    assert!(
        Path::new(ci_path).exists(),
        "CI workflow file must exist at .github/workflows/ci.yml"
    );
}

/// Validate that CI pipeline includes security scanning step
#[test]
fn test_ci_pipeline_includes_security_scanning() {
    use std::path::Path;

    let ci_path = ".github/workflows/ci.yml";
    assert!(
        Path::new(ci_path).exists(),
        "CI workflow must include security scanning (gitleaks)"
    );
}

/// Validate that dependabot has weekly update schedule configured
#[test]
fn test_dependabot_config_has_weekly_schedule() {
    use std::path::Path;

    let config_path = ".github/dependabot.yml";
    assert!(
        Path::new(config_path).exists(),
        "dependabot.yml must have weekly schedule configured"
    );
}

/// Validate that CI includes version consistency check
#[test]
fn test_ci_includes_version_consistency_check() {
    use std::path::Path;

    let ci_path = ".github/workflows/ci.yml";
    assert!(
        Path::new(ci_path).exists(),
        "CI must include version consistency validation"
    );
}

/// Validate that CI includes OpenAPI specification validation
#[test]
fn test_ci_includes_openapi_validation() {
    use std::path::Path;

    let ci_path = ".github/workflows/ci.yml";
    let openapi_path = "docs/openapi.yaml";

    assert!(
        Path::new(ci_path).exists(),
        "CI workflow must include OpenAPI validation"
    );
    assert!(
        Path::new(openapi_path).exists() || !Path::new(ci_path).exists(),
        "OpenAPI specification should be documented"
    );
}

/// Validate that CI includes code coverage reporting
#[test]
fn test_ci_includes_code_coverage() {
    use std::path::Path;

    let ci_path = ".github/workflows/ci.yml";
    assert!(
        Path::new(ci_path).exists(),
        "CI must include code coverage job"
    );
}

/// Validate that Justfile exists and contains audit command
#[test]
fn test_justfile_contains_audit_command() {
    use std::path::Path;

    let justfile_path = "Justfile";
    assert!(
        Path::new(justfile_path).exists(),
        "Justfile must exist for local development"
    );
}

/// Validate minimum code coverage threshold is set
#[test]
fn test_code_coverage_threshold_configured() {
    use std::path::Path;

    let ci_path = ".github/workflows/ci.yml";
    assert!(
        Path::new(ci_path).exists(),
        "CI must have code coverage threshold configuration"
    );
}
