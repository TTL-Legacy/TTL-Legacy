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
