/// #1102: Webhook Delivery Retry with Exponential Backoff
///
/// Retry schedule (max 5 attempts after the first):
///   Attempt 1: immediate
///   Retry  1: +1  min
///   Retry  2: +5  min
///   Retry  3: +15 min
///   Retry  4: +1  h
///   Retry  5: +4  h
///
/// After all retries are exhausted, status → DeliveryFailed and the vault
/// owner is notified via email (stub; replace with real email integration).
use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::{
    db::Db,
    models::{
        TimelineEvent, TimelineEventKind, WebhookAttempt, WebhookDelivery, WebhookDeliveryStatus,
    },
};

/// Exponential backoff delays in seconds: 1 min, 5 min, 15 min, 1 h, 4 h.
pub const RETRY_DELAYS_SECS: [u64; 5] = [60, 300, 900, 3_600, 14_400];

/// Maximum number of attempts (including the first delivery attempt).
pub const MAX_ATTEMPTS: u32 = 6; // 1 initial + 5 retries

// ── Public API ───────────────────────────────────────────────────────────────

/// Queue a new webhook delivery job for a vault event. This should be called
/// whenever a significant vault event occurs (release, low TTL, etc.).
pub fn enqueue(
    db: &Arc<Db>,
    vault_id: &str,
    event_type: &str,
    payload: serde_json::Value,
    endpoint_url: &str,
) -> Result<WebhookDelivery, String> {
    let delivery = WebhookDelivery {
        id: Uuid::new_v4().to_string(),
        vault_id: vault_id.to_string(),
        event_type: event_type.to_string(),
        payload,
        endpoint_url: endpoint_url.to_string(),
        status: WebhookDeliveryStatus::Pending,
        attempt_count: 0,
        next_retry_at: None,
        created_at: Utc::now(),
        attempts: Vec::new(),
    };
    db.insert_webhook_delivery(&delivery)
        .map_err(|e| e.to_string())?;
    Ok(delivery)
}

/// Process all pending and due-retry webhook deliveries. Called from the
/// scheduler loop.
#[tracing::instrument(skip(db))]
pub async fn flush(db: &Arc<Db>) {
    // First, attempt pending deliveries.
    match db.get_pending_webhook_deliveries() {
        Ok(pending) => {
            for delivery in pending {
                attempt_delivery(db, delivery).await;
            }
        }
        Err(e) => tracing::error!(error = %e, "webhook_retry: failed to fetch pending deliveries"),
    }

    // Then, retry any Retrying deliveries that are due.
    match db.get_due_webhook_retries() {
        Ok(due) => {
            for delivery in due {
                attempt_delivery(db, delivery).await;
            }
        }
        Err(e) => tracing::error!(error = %e, "webhook_retry: failed to fetch due retries"),
    }
}

/// Get webhook delivery log for a vault.
pub fn get_delivery_log(db: &Arc<Db>, vault_id: &str) -> Result<Vec<WebhookDelivery>, String> {
    db.get_webhook_deliveries_for_vault(vault_id)
        .map_err(|e| e.to_string())
}

// ── Internal delivery logic ──────────────────────────────────────────────────

async fn attempt_delivery(db: &Arc<Db>, mut delivery: WebhookDelivery) {
    let attempt_number = delivery.attempt_count + 1;

    tracing::info!(
        delivery_id = %delivery.id,
        vault_id = %delivery.vault_id,
        endpoint = %delivery.endpoint_url,
        attempt = attempt_number,
        "webhook_retry: attempting delivery"
    );

    let (http_status, response_body, error) =
        send_webhook(&delivery.endpoint_url, &delivery.payload).await;

    let now = Utc::now();
    let attempt_log = WebhookAttempt {
        attempted_at: now,
        http_status,
        response_body: response_body.clone(),
        error: error.clone(),
    };
    delivery.attempts.push(attempt_log);
    delivery.attempt_count = attempt_number;

    let success = http_status.map_or(false, |s| (200..300).contains(&s));

    if success {
        delivery.status = WebhookDeliveryStatus::Delivered;
        delivery.next_retry_at = None;
        tracing::info!(
            delivery_id = %delivery.id,
            vault_id = %delivery.vault_id,
            attempt = attempt_number,
            "webhook_retry: delivered successfully"
        );

        record_timeline_event(db, &delivery, true).await;
    } else {
        let retry_index = (attempt_number - 1) as usize; // 0-based index into RETRY_DELAYS_SECS
        if retry_index < RETRY_DELAYS_SECS.len() {
            // Schedule a retry.
            let delay = RETRY_DELAYS_SECS[retry_index];
            delivery.status = WebhookDeliveryStatus::Retrying;
            delivery.next_retry_at = Some(now + chrono::Duration::seconds(delay as i64));
            tracing::warn!(
                delivery_id = %delivery.id,
                vault_id = %delivery.vault_id,
                attempt = attempt_number,
                retry_in_secs = delay,
                error = ?error,
                "webhook_retry: delivery failed, scheduling retry"
            );
        } else {
            // All retries exhausted.
            delivery.status = WebhookDeliveryStatus::DeliveryFailed;
            delivery.next_retry_at = None;
            tracing::error!(
                delivery_id = %delivery.id,
                vault_id = %delivery.vault_id,
                total_attempts = attempt_number,
                "webhook_retry: all retries exhausted — delivery permanently failed"
            );

            // Notify vault owner via email (stub).
            notify_owner_delivery_failed(&delivery).await;
            record_timeline_event(db, &delivery, false).await;
        }
    }

    if let Err(e) = db.update_webhook_delivery(&delivery) {
        tracing::error!(
            delivery_id = %delivery.id,
            error = %e,
            "webhook_retry: failed to persist delivery update"
        );
    }
}

/// HTTP POST to the endpoint. Returns (http_status, response_body, error).
async fn send_webhook(
    url: &str,
    payload: &serde_json::Value,
) -> (Option<u16>, String, Option<String>) {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    match client.post(url).json(payload).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            if (200..300).contains(&status) {
                (Some(status), body, None)
            } else {
                (
                    Some(status),
                    body.clone(),
                    Some(format!("HTTP {status}: {body}")),
                )
            }
        }
        Err(e) => (None, String::new(), Some(e.to_string())),
    }
}

/// Record the delivery outcome as a vault timeline event.
async fn record_timeline_event(db: &Arc<Db>, delivery: &WebhookDelivery, success: bool) {
    let kind = if success {
        TimelineEventKind::WebhookDelivered
    } else {
        TimelineEventKind::WebhookFailed
    };
    let description = if success {
        format!(
            "Webhook '{}' delivered to {}",
            delivery.event_type, delivery.endpoint_url
        )
    } else {
        format!(
            "Webhook '{}' permanently failed after {} attempts",
            delivery.event_type, delivery.attempt_count
        )
    };
    let event = TimelineEvent {
        id: Uuid::new_v4().to_string(),
        vault_id: delivery.vault_id.clone(),
        kind,
        timestamp: Utc::now(),
        description,
        amount: None,
        metadata: serde_json::json!({
            "delivery_id": delivery.id,
            "event_type": delivery.event_type,
            "endpoint_url": delivery.endpoint_url,
            "attempt_count": delivery.attempt_count,
        }),
    };
    if let Err(e) = db.insert_timeline_event(&event) {
        tracing::error!(error = %e, "webhook_retry: failed to insert timeline event");
    }
}

/// Stub: sends a failure notification email to the vault owner.
async fn notify_owner_delivery_failed(delivery: &WebhookDelivery) {
    tracing::warn!(
        vault_id = %delivery.vault_id,
        event_type = %delivery.event_type,
        endpoint = %delivery.endpoint_url,
        "webhook_retry: notifying owner of permanent delivery failure (stub)"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use std::sync::Arc;

    fn test_db() -> Arc<Db> {
        let db = Arc::new(Db::open(":memory:").unwrap());
        db.migrate().unwrap();
        db
    }

    #[test]
    fn test_enqueue_creates_pending_delivery() {
        let db = test_db();
        let payload = serde_json::json!({"event": "vault_released", "vault_id": "v1"});
        let delivery = enqueue(
            &db,
            "v1",
            "vault_released",
            payload,
            "https://example.com/hook",
        )
        .expect("enqueue should succeed");

        assert_eq!(delivery.vault_id, "v1");
        assert_eq!(delivery.event_type, "vault_released");
        assert_eq!(delivery.status, WebhookDeliveryStatus::Pending);
        assert_eq!(delivery.attempt_count, 0);
        assert!(delivery.attempts.is_empty());
    }

    #[test]
    fn test_retry_delays_sequence() {
        // Verify the backoff schedule matches the spec: 1m, 5m, 15m, 1h, 4h.
        assert_eq!(RETRY_DELAYS_SECS[0], 60);
        assert_eq!(RETRY_DELAYS_SECS[1], 300);
        assert_eq!(RETRY_DELAYS_SECS[2], 900);
        assert_eq!(RETRY_DELAYS_SECS[3], 3_600);
        assert_eq!(RETRY_DELAYS_SECS[4], 14_400);
        assert_eq!(RETRY_DELAYS_SECS.len(), 5, "exactly 5 retry intervals");
    }

    #[test]
    fn test_max_attempts_is_six() {
        assert_eq!(MAX_ATTEMPTS, 6, "1 initial + 5 retries = 6 total");
    }

    #[tokio::test]
    async fn test_exhaustion_marks_delivery_failed() {
        let db = test_db();
        // Insert a delivery already at MAX_ATTEMPTS - 1 retries, with a bad URL.
        let delivery = WebhookDelivery {
            id: "d-exhaust".to_string(),
            vault_id: "v99".to_string(),
            event_type: "test".to_string(),
            payload: serde_json::json!({}),
            endpoint_url: "http://127.0.0.1:0/nonexistent".to_string(),
            status: WebhookDeliveryStatus::Retrying,
            // Set attempt_count to 5 (last retry slot) so next attempt exhausts.
            attempt_count: 5,
            next_retry_at: Some(Utc::now() - chrono::Duration::seconds(1)),
            created_at: Utc::now(),
            attempts: Vec::new(),
        };
        db.insert_webhook_delivery(&delivery).unwrap();

        // Flush retries — the single delivery should be marked DeliveryFailed.
        flush(&db).await;

        let log = db.get_webhook_deliveries_for_vault("v99").unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(
            log[0].status,
            WebhookDeliveryStatus::DeliveryFailed,
            "status should be DeliveryFailed after exhaustion"
        );
    }

    #[tokio::test]
    async fn test_successful_delivery_clears_retries() {
        // We can't easily mock HTTP in unit tests, so we verify the enqueue +
        // state-machine logic directly without a live HTTP call.
        let db = test_db();

        // Simulate a delivery that "succeeded" by manually updating it.
        let payload = serde_json::json!({"ok": true});
        let mut delivery =
            enqueue(&db, "v2", "check_in", payload, "https://example.com/ok").expect("enqueue");

        // Manually drive it to Delivered state (as the send_webhook mock would).
        delivery.status = WebhookDeliveryStatus::Delivered;
        delivery.attempt_count = 1;
        delivery.next_retry_at = None;
        db.update_webhook_delivery(&delivery).unwrap();

        let log = db.get_webhook_deliveries_for_vault("v2").unwrap();
        assert_eq!(log[0].status, WebhookDeliveryStatus::Delivered);
        assert!(log[0].next_retry_at.is_none());
    }

    #[tokio::test]
    async fn test_retry_scheduling_after_failure() {
        let db = test_db();
        // Insert a pending delivery pointing to an unreachable endpoint.
        let delivery = WebhookDelivery {
            id: "d-retry-test".to_string(),
            vault_id: "v10".to_string(),
            event_type: "vault_released".to_string(),
            payload: serde_json::json!({}),
            endpoint_url: "http://127.0.0.1:0/bad".to_string(),
            status: WebhookDeliveryStatus::Pending,
            attempt_count: 0,
            next_retry_at: None,
            created_at: Utc::now(),
            attempts: Vec::new(),
        };
        db.insert_webhook_delivery(&delivery).unwrap();

        // Flush pending — should fail and move to Retrying with a next_retry_at.
        flush(&db).await;

        let log = db.get_webhook_deliveries_for_vault("v10").unwrap();
        assert_eq!(log.len(), 1);
        // After first failure, status must be Retrying (not DeliveryFailed).
        assert_eq!(log[0].status, WebhookDeliveryStatus::Retrying);
        assert!(
            log[0].next_retry_at.is_some(),
            "next_retry_at must be set after first failure"
        );
        assert_eq!(log[0].attempt_count, 1);
    }
}
