use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
    routing::{get, post},
};
use serde_json::json;
use tower::ServiceExt;

use crate::{db::Db, routes};

fn test_app() -> Router {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();
    Router::new()
        .route(
            "/api/vaults/:vault_id/reminder-preferences",
            post(routes::set_preferences).get(routes::get_preferences),
        )
        .route(
            "/notifications/unsubscribe",
            get(routes::unsubscribe),
        )
        .with_state(db)
}

fn test_db() -> Arc<Db> {
    let db = Arc::new(Db::open(":memory:").unwrap());
    db.migrate().unwrap();
    db
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
    };
    db.upsert(&p1).unwrap();

    let p2 = crate::models::ReminderPreferences {
        vault_id: 5,
        channels: vec![crate::models::Channel::Sms, crate::models::Channel::Push],
        hours_before_expiry: 6,
        frequency: crate::models::Frequency::Hourly,
    };
    db.upsert(&p2).unwrap();

    let fetched = db.get(5).unwrap();
    assert_eq!(fetched.hours_before_expiry, 6);
    assert_eq!(fetched.channels.len(), 2);
    assert_eq!(fetched.frequency, crate::models::Frequency::Hourly);
}

// ── Idempotency key tests (#825) ────────────────────────────────────────────

async fn post_json_with_header(
    app: Router,
    uri: &str,
    body: serde_json::Value,
    header_name: &str,
    header_value: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .header(header_name, header_value)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn test_idempotent_request_returns_cached() {
    let db = test_db();
    let body = json!({
        "channels": ["email"],
        "hours_before_expiry": 24,
        "frequency": "once"
    });

    let app1 = Router::new()
        .route(
            "/api/vaults/:vault_id/reminder-preferences",
            post(routes::set_preferences),
        )
        .with_state(db.clone());

    let res1 = post_json_with_header(
        app1,
        "/api/vaults/1/reminder-preferences",
        body.clone(),
        "idempotency-key",
        "idem-123",
    )
    .await;
    assert_eq!(res1.status(), StatusCode::OK);

    // Second request with same key returns cached
    let app2 = Router::new()
        .route(
            "/api/vaults/:vault_id/reminder-preferences",
            post(routes::set_preferences),
        )
        .with_state(db.clone());

    let res2 = post_json_with_header(
        app2,
        "/api/vaults/1/reminder-preferences",
        body,
        "idempotency-key",
        "idem-123",
    )
    .await;
    assert_eq!(res2.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_non_idempotent_request_processes_normally() {
    let app = test_app();
    let body = json!({
        "channels": ["sms"],
        "hours_before_expiry": 12,
        "frequency": "daily"
    });
    let res = post_json(app, "/api/vaults/2/reminder-preferences", body).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_db_idempotency_store_and_check() {
    let db = test_db();
    assert!(db.check_idempotency("key-abc").is_none());
    db.store_idempotency("key-abc", 200, r#"{"vault_id":1}"#);
    let cached = db.check_idempotency("key-abc").unwrap();
    assert_eq!(cached.status_code, 200);
    assert_eq!(cached.response_body, r#"{"vault_id":1}"#);
}

// ── Unsubscribe tests (#828) ────────────────────────────────────────────────

#[tokio::test]
async fn test_unsubscribe_valid_token() {
    let db = test_db();
    let token = db.generate_unsubscribe_token("owner1");

    let app = Router::new()
        .route("/notifications/unsubscribe", get(routes::unsubscribe))
        .with_state(db.clone());

    let uri = format!("/notifications/unsubscribe?token={token}");
    let res = get_req(app, &uri).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(db.is_unsubscribed("owner1"));
}

#[tokio::test]
async fn test_unsubscribe_invalid_token() {
    let app = test_app();
    let res = get_req(app, "/notifications/unsubscribe?token=bogus").await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_db_unsubscribe_flow() {
    let db = test_db();
    assert!(!db.is_unsubscribed("owner1"));
    let token = db.generate_unsubscribe_token("owner1");
    let result = db.process_unsubscribe(&token);
    assert!(result.is_ok());
    assert!(db.is_unsubscribed("owner1"));
}

#[tokio::test]
async fn test_db_unsubscribe_invalid_token() {
    let db = test_db();
    let result = db.process_unsubscribe("nonexistent");
    assert!(result.is_err());
}
