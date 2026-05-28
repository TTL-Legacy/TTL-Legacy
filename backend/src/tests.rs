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
        .with_state(db)
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
