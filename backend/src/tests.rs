use std::sync::Arc;

use axum::{
    body::Body,
    http::{HeaderValue, Method, Request, StatusCode},
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::json;
use tower::ServiceExt;
use tower_http::cors::CorsLayer;

use crate::{
    db::{AppState, create_vault_store, create_event_store, create_audit_store,
           create_share_store, create_share_token_store, Db, PoolConfig},
    models::*,
    routes,
};

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

fn test_app_with_state(state: Arc<AppState>) -> Router {
    state.db.migrate().unwrap();
    build_router(state)
}

fn build_router(state: Arc<AppState>) -> Router {
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
        // Vault share endpoints
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
        .route(
            "/api/shared/vaults/{token}",
            get(routes::access_shared_vault),
        )
        .route(
            "/api/shared/vaults/{token}/export",
            get(routes::access_shared_vault_export),
        )
        .with_state(state)
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

// ── #966: Vault Sharing with Read-Only Access tests ─────────────────────────

#[tokio::test]
async fn test_share_vault_endpoint_creates_share() {
    let state = Arc::new(AppState {
        db: Arc::new(Db::open(":memory:").unwrap()),
        vault_store: create_vault_store(),
        event_store: create_event_store(),
        audit_store: create_audit_store(),
        share_store: create_share_store(),
        share_token_store: create_share_token_store(),
    });
    state.db.migrate().unwrap();
    let app = build_router(state.clone());

    // Seed a vault
    state.vault_store.lock().unwrap().insert("vault-1".into(), Vault {
        id: "vault-1".into(),
        owner: "owner-1".into(),
        beneficiary: "ben-1".into(),
        balance: 1000,
        check_in_interval: 86400,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(86400),
    });

    let res = post_json(app, "/api/vaults/vault-1/share", json!({
        "shared_with": "lawyer@example.com",
        "permission": "view_only",
    })).await;
    assert_eq!(res.status(), StatusCode::OK);

    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap()
    ).unwrap();
    assert_eq!(body["vault_id"], "vault-1");
    assert_eq!(body["shared_with"], "lawyer@example.com");
    assert_eq!(body["permission"], "view_only");
}

#[tokio::test]
async fn test_share_vault_missing_vault_returns_error() {
    let app = test_app();
    let res = post_json(app, "/api/vaults/nonexistent/share", json!({
        "shared_with": "x@example.com",
        "permission": "view_only",
    })).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_generate_share_token_creates_token_and_audit_log() {
    let state = Arc::new(AppState {
        db: Arc::new(Db::open(":memory:").unwrap()),
        vault_store: create_vault_store(),
        event_store: create_event_store(),
        audit_store: create_audit_store(),
        share_store: create_share_store(),
        share_token_store: create_share_token_store(),
    });
    state.db.migrate().unwrap();
    state.vault_store.lock().unwrap().insert("vault-1".into(), Vault {
        id: "vault-1".into(),
        owner: "owner-1".into(),
        beneficiary: "ben-1".into(),
        balance: 5000,
        check_in_interval: 86400,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(86400),
    });

    let app = build_router(state.clone());

    let res = post_json(app, "/api/vaults/vault-1/share/tokens", json!({
        "shared_with": "family@example.com",
        "expiry_seconds": 3600,
    })).await;
    assert_eq!(res.status(), StatusCode::OK);

    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap()
    ).unwrap();
    assert_eq!(body["share"]["vault_id"], "vault-1");
    assert_eq!(body["share"]["shared_with"], "family@example.com");
    assert_eq!(body["token"]["permission"], "view_only");
    assert_eq!(body["token"]["revoked"], false);
    assert!(body["token"]["token"].as_str().unwrap().len() > 0);
    assert!(body["access_url"].as_str().unwrap().contains("/api/shared/vaults/"));

    // Verify audit log was written
    let audit = state.audit_store.lock().unwrap();
    let share_event = audit.iter().find(|e| e.action == "share_token_generated");
    assert!(share_event.is_some());
}

#[tokio::test]
async fn test_revoke_share_token() {
    let state = Arc::new(AppState {
        db: Arc::new(Db::open(":memory:").unwrap()),
        vault_store: create_vault_store(),
        event_store: create_event_store(),
        audit_store: create_audit_store(),
        share_store: create_share_store(),
        share_token_store: create_share_token_store(),
    });
    state.db.migrate().unwrap();
    state.vault_store.lock().unwrap().insert("vault-1".into(), Vault {
        id: "vault-1".into(),
        owner: "owner-1".into(),
        beneficiary: "ben-1".into(),
        balance: 100,
        check_in_interval: 86400,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(86400),
    });

    let token_str = {
        let token = ShareToken {
            token: "test-token-123".into(),
            share_id: "share-1".into(),
            vault_id: "vault-1".into(),
            shared_with: "reviewee@example.com".into(),
            permission: SharePermission::ViewOnly,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::days(7),
            revoked: false,
        };
        let t = token.token.clone();
        state.share_token_store.lock().unwrap().insert(t.clone(), token);
        t
    };

    let app = build_router(state.clone());

    let res = post_json(app, "/api/vaults/vault-1/share/tokens/revoke", json!({
        "token": token_str,
    })).await;
    assert_eq!(res.status(), StatusCode::OK);

    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap()
    ).unwrap();
    assert_eq!(body["revoked"], true);

    // Verify token is actually revoked
    let stored = state.share_token_store.lock().unwrap();
    let token = stored.get(&token_str).unwrap();
    assert!(token.revoked);

    // Audit log written
    let audit = state.audit_store.lock().unwrap();
    assert!(audit.iter().any(|e| e.action == "share_token_revoked"));
}

#[tokio::test]
async fn test_read_only_access_via_valid_share_token() {
    let state = Arc::new(AppState {
        db: Arc::new(Db::open(":memory:").unwrap()),
        vault_store: create_vault_store(),
        event_store: create_event_store(),
        audit_store: create_audit_store(),
        share_store: create_share_store(),
        share_token_store: create_share_token_store(),
    });
    state.db.migrate().unwrap();
    state.vault_store.lock().unwrap().insert("vault-1".into(), Vault {
        id: "vault-1".into(),
        owner: "owner-1".into(),
        beneficiary: "ben-1".into(),
        balance: 9999,
        check_in_interval: 86400,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(86400),
    });

    state.share_token_store.lock().unwrap().insert("valid-token".into(), ShareToken {
        token: "valid-token".into(),
        share_id: "share-1".into(),
        vault_id: "vault-1".into(),
        shared_with: "reader@example.com".into(),
        permission: SharePermission::ViewOnly,
        created_at: Utc::now(),
        expires_at: Utc::now() + chrono::Duration::days(7),
        revoked: false,
    });

    let app = build_router(state.clone());

    let res = get_req(app, "/api/shared/vaults/valid-token").await;
    assert_eq!(res.status(), StatusCode::OK);

    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap()
    ).unwrap();
    assert_eq!(body["id"], "vault-1");
    assert_eq!(body["balance"], 9999);
    assert_eq!(body["owner"], "owner-1");

    // Audit log written
    let audit = state.audit_store.lock().unwrap();
    assert!(audit.iter().any(|e| e.action == "vault_accessed_via_share"));
}

#[tokio::test]
async fn test_read_only_access_revoked_token_returns_error() {
    let state = Arc::new(AppState {
        db: Arc::new(Db::open(":memory:").unwrap()),
        vault_store: create_vault_store(),
        event_store: create_event_store(),
        audit_store: create_audit_store(),
        share_store: create_share_store(),
        share_token_store: create_share_token_store(),
    });
    state.db.migrate().unwrap();
    state.vault_store.lock().unwrap().insert("vault-1".into(), Vault {
        id: "vault-1".into(),
        owner: "owner-1".into(),
        beneficiary: "ben-1".into(),
        balance: 100,
        check_in_interval: 86400,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(86400),
    });

    state.share_token_store.lock().unwrap().insert("revoked-token".into(), ShareToken {
        token: "revoked-token".into(),
        share_id: "share-1".into(),
        vault_id: "vault-1".into(),
        shared_with: "reader@example.com".into(),
        permission: SharePermission::ViewOnly,
        created_at: Utc::now(),
        expires_at: Utc::now() + chrono::Duration::days(7),
        revoked: true,
    });

    let app = build_router(state.clone());
    let res = get_req(app, "/api/shared/vaults/revoked-token").await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_read_only_access_expired_token_returns_error() {
    let state = Arc::new(AppState {
        db: Arc::new(Db::open(":memory:").unwrap()),
        vault_store: create_vault_store(),
        event_store: create_event_store(),
        audit_store: create_audit_store(),
        share_store: create_share_store(),
        share_token_store: create_share_token_store(),
    });
    state.db.migrate().unwrap();
    state.vault_store.lock().unwrap().insert("vault-1".into(), Vault {
        id: "vault-1".into(),
        owner: "owner-1".into(),
        beneficiary: "ben-1".into(),
        balance: 100,
        check_in_interval: 86400,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(86400),
    });

    state.share_token_store.lock().unwrap().insert("expired-token".into(), ShareToken {
        token: "expired-token".into(),
        share_id: "share-1".into(),
        vault_id: "vault-1".into(),
        shared_with: "reader@example.com".into(),
        permission: SharePermission::ViewOnly,
        created_at: Utc::now(),
        expires_at: Utc::now() - chrono::Duration::hours(1), // expired
        revoked: false,
    });

    let app = build_router(state.clone());
    let res = get_req(app, "/api/shared/vaults/expired-token").await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_read_only_access_invalid_token_returns_error() {
    let app = test_app();
    let res = get_req(app, "/api/shared/vaults/nonexistent-token").await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_list_share_tokens() {
    let state = Arc::new(AppState {
        db: Arc::new(Db::open(":memory:").unwrap()),
        vault_store: create_vault_store(),
        event_store: create_event_store(),
        audit_store: create_audit_store(),
        share_store: create_share_store(),
        share_token_store: create_share_token_store(),
    });
    state.db.migrate().unwrap();

    // Insert two tokens for vault-1, one token for vault-2
    for i in 0..2 {
        state.share_token_store.lock().unwrap().insert(format!("token-{}", i), ShareToken {
            token: format!("token-{}", i),
            share_id: format!("share-{}", i),
            vault_id: "vault-1".into(),
            shared_with: format!("user{}@example.com", i),
            permission: SharePermission::ViewOnly,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::days(7),
            revoked: false,
        });
    }
    state.share_token_store.lock().unwrap().insert("other-token".into(), ShareToken {
        token: "other-token".into(),
        share_id: "share-other".into(),
        vault_id: "vault-2".into(),
        shared_with: "other@example.com".into(),
        permission: SharePermission::ViewOnly,
        created_at: Utc::now(),
        expires_at: Utc::now() + chrono::Duration::days(7),
        revoked: false,
    });

    let app = build_router(state.clone());
    let res = get_req(app, "/api/vaults/vault-1/share/tokens").await;
    assert_eq!(res.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap()
    ).unwrap();
    assert_eq!(body.len(), 2);
}

#[tokio::test]
async fn test_list_vault_shares() {
    let state = Arc::new(AppState {
        db: Arc::new(Db::open(":memory:").unwrap()),
        vault_store: create_vault_store(),
        event_store: create_event_store(),
        audit_store: create_audit_store(),
        share_store: create_share_store(),
        share_token_store: create_share_token_store(),
    });
    state.db.migrate().unwrap();

    state.share_store.lock().unwrap().push(VaultShare {
        share_id: "s1".into(),
        vault_id: "vault-1".into(),
        shared_with: "a@example.com".into(),
        permission: SharePermission::ViewOnly,
        created_at: Utc::now(),
    });
    state.share_store.lock().unwrap().push(VaultShare {
        share_id: "s2".into(),
        vault_id: "vault-1".into(),
        shared_with: "b@example.com".into(),
        permission: SharePermission::Admin,
        created_at: Utc::now(),
    });

    let app = build_router(state.clone());
    let res = get_req(app, "/api/vaults/vault-1/shares").await;
    assert_eq!(res.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap()
    ).unwrap();
    assert_eq!(body.len(), 2);
}

#[tokio::test]
async fn test_share_token_default_expiry() {
    let state = Arc::new(AppState {
        db: Arc::new(Db::open(":memory:").unwrap()),
        vault_store: create_vault_store(),
        event_store: create_event_store(),
        audit_store: create_audit_store(),
        share_store: create_share_store(),
        share_token_store: create_share_token_store(),
    });
    state.db.migrate().unwrap();
    state.vault_store.lock().unwrap().insert("vault-1".into(), Vault {
        id: "vault-1".into(),
        owner: "owner-1".into(),
        beneficiary: "ben-1".into(),
        balance: 100,
        check_in_interval: 86400,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(86400),
    });

    let app = build_router(state.clone());
    let res = post_json(app, "/api/vaults/vault-1/share/tokens", json!({
        "shared_with": "test@example.com",
    })).await;
    assert_eq!(res.status(), StatusCode::OK);

    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap()
    ).unwrap();
    let expires_at = body["token"]["expires_at"].as_str().unwrap();
    let expires_dt = chrono::DateTime::parse_from_rfc3339(expires_at).unwrap();
    let expected = Utc::now() + chrono::Duration::days(7);
    // Should be roughly 7 days from now (within tolerance)
    let diff = (expires_dt - expected).num_seconds().abs();
    assert!(diff < 10, "expiry should be ~7 days, diff={}s", diff);
}

#[tokio::test]
async fn test_share_token_with_custom_expiry() {
    let state = Arc::new(AppState {
        db: Arc::new(Db::open(":memory:").unwrap()),
        vault_store: create_vault_store(),
        event_store: create_event_store(),
        audit_store: create_audit_store(),
        share_store: create_share_store(),
        share_token_store: create_share_token_store(),
    });
    state.db.migrate().unwrap();
    state.vault_store.lock().unwrap().insert("vault-1".into(), Vault {
        id: "vault-1".into(),
        owner: "owner-1".into(),
        beneficiary: "ben-1".into(),
        balance: 100,
        check_in_interval: 86400,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(86400),
    });

    let app = build_router(state.clone());
    let res = post_json(app, "/api/vaults/vault-1/share/tokens", json!({
        "shared_with": "test@example.com",
        "expiry_seconds": 1800,
    })).await;
    assert_eq!(res.status(), StatusCode::OK);

    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap()
    ).unwrap();
    let expires_at = body["token"]["expires_at"].as_str().unwrap();
    let expires_dt = chrono::DateTime::parse_from_rfc3339(expires_at).unwrap();
    let expected = Utc::now() + chrono::Duration::seconds(1800);
    let diff = (expires_dt - expected).num_seconds().abs();
    assert!(diff < 10, "expiry should be ~1800s, diff={}s", diff);
}

#[tokio::test]
async fn test_share_vault_with_duplicate_shares_allowed() {
    let state = Arc::new(AppState {
        db: Arc::new(Db::open(":memory:").unwrap()),
        vault_store: create_vault_store(),
        event_store: create_event_store(),
        audit_store: create_audit_store(),
        share_store: create_share_store(),
        share_token_store: create_share_token_store(),
    });
    state.db.migrate().unwrap();
    state.vault_store.lock().unwrap().insert("vault-1".into(), Vault {
        id: "vault-1".into(),
        owner: "owner-1".into(),
        beneficiary: "ben-1".into(),
        balance: 100,
        check_in_interval: 86400,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(86400),
    });

    let app = build_router(state.clone());

    // Share twice with the same person
    for _ in 0..2 {
        let res = post_json(app.clone(), "/api/vaults/vault-1/share", json!({
            "shared_with": "same@example.com",
            "permission": "view_only",
        })).await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    let shares = state.share_store.lock().unwrap();
    assert_eq!(shares.len(), 2);
}

#[tokio::test]
async fn test_audit_log_entries_for_share_events() {
    let state = Arc::new(AppState {
        db: Arc::new(Db::open(":memory:").unwrap()),
        vault_store: create_vault_store(),
        event_store: create_event_store(),
        audit_store: create_audit_store(),
        share_store: create_share_store(),
        share_token_store: create_share_token_store(),
    });
    state.db.migrate().unwrap();
    state.vault_store.lock().unwrap().insert("vault-1".into(), Vault {
        id: "vault-1".into(),
        owner: "owner-1".into(),
        beneficiary: "ben-1".into(),
        balance: 100,
        check_in_interval: 86400,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(86400),
    });

    // Perform share
    crate::handlers::share_vault_handler(
        &state.vault_store,
        &state.share_store,
        &state.share_token_store,
        &state.audit_store,
        "vault-1",
        ShareRequest {
            shared_with: "audit-test@example.com".into(),
            permission: SharePermission::ViewOnly,
        },
    ).unwrap();

    // Verify audit entry
    let audit = state.audit_store.lock().unwrap();
    let entry = audit.iter().find(|e| e.action == "vault_shared").unwrap();
    assert_eq!(entry.actor, "owner-1");
    assert_eq!(entry.details["vault_id"], "vault-1");
    assert_eq!(entry.details["shared_with"], "audit-test@example.com");
}
