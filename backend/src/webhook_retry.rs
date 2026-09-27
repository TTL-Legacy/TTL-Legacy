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
/// owner is notified via email through the configured email provider.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    db::Db,
    email::EmailProvider,
    models::{
        TimelineEvent, TimelineEventKind, WebhookAttempt, WebhookDelivery, WebhookDeliveryStatus,
    },
};

/// Exponential backoff delays in seconds: 1 min, 5 min, 15 min, 1 h, 4 h.
pub const RETRY_DELAYS_SECS: [u64; 5] = [60, 300, 900, 3_600, 14_400];

/// Maximum number of attempts (including the first delivery attempt).
pub const MAX_ATTEMPTS: u32 = 6; // 1 initial + 5 retries

/// Minimum interval between permanent-failure emails for the same owner.
pub const FAILURE_EMAIL_RATE_LIMIT: Duration = Duration::from_secs(3_600);

/// Tracks the last time a permanent-failure email was sent per owner so we
/// don't spam an owner when many webhooks fail at once.
static FAILURE_EMAIL_LAST_SENT: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);

const _: () = assert!(MAX_ATTEMPTS as usize == RETRY_DELAYS_SECS.len() + 1);

// ── Clock ────────────────────────────────────────────────────────────────────

/// Source of the current time for retry scheduling. Injected so the backoff
/// schedule can be driven deterministically in tests (#1595).
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// Wall-clock time; used in production.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

// ── Backoff schedule ─────────────────────────────────────────────────────────

/// Result of applying one delivery attempt to a delivery's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptOutcome {
    Delivered,
    /// Failed; the next attempt is scheduled `delay_secs` after this one.
    RetryScheduled {
        delay_secs: u64,
    },
    /// Failed and `MAX_ATTEMPTS` has been reached.
    Exhausted,
}

/// Backoff delay after the `attempt_number`-th (1-based) failed attempt, or
/// `None` when no further retry is allowed.
pub fn retry_delay_secs(attempt_number: u32) -> Option<u64> {
    if attempt_number == 0 || attempt_number >= MAX_ATTEMPTS {
        return None;
    }
    RETRY_DELAYS_SECS
        .get((attempt_number - 1) as usize)
        .copied()
}

/// Record the outcome of an attempt made at `clock.now()`: bumps
/// `attempt_count` and sets `status` / `next_retry_at` per the backoff schedule.
pub fn apply_attempt_outcome(
    delivery: &mut WebhookDelivery,
    success: bool,
    clock: &dyn Clock,
) -> AttemptOutcome {
    delivery.attempt_count += 1;

    if success {
        delivery.status = WebhookDeliveryStatus::Delivered;
        delivery.next_retry_at = None;
        return AttemptOutcome::Delivered;
    }

    let Some(delay_secs) = retry_delay_secs(delivery.attempt_count) else {
        delivery.status = WebhookDeliveryStatus::DeliveryFailed;
        delivery.next_retry_at = None;
        return AttemptOutcome::Exhausted;
    };

    // Delays are small compile-time constants, so the cast cannot wrap.
    #[allow(clippy::cast_possible_wrap)]
    let delay = chrono::Duration::seconds(delay_secs as i64);
    delivery.status = WebhookDeliveryStatus::Retrying;
    delivery.next_retry_at = Some(clock.now() + delay);
    AttemptOutcome::RetryScheduled { delay_secs }
}

/// Whether a `Retrying` delivery is due for another attempt at `clock.now()`.
pub fn is_retry_due(delivery: &WebhookDelivery, clock: &dyn Clock) -> bool {
    delivery.status == WebhookDeliveryStatus::Retrying
        && delivery.next_retry_at.is_some_and(|at| at <= clock.now())
}

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
pub async fn flush(db: &Arc<Db>) {
    flush_with_clock(db, &SystemClock).await;
}

/// `flush` with an injected clock.
#[tracing::instrument(skip(db, clock))]
pub async fn flush_with_clock(db: &Arc<Db>, clock: &dyn Clock) {
    // First, attempt pending deliveries.
    match db.get_pending_webhook_deliveries() {
        Ok(pending) => {
            for delivery in pending {
                attempt_delivery(db, delivery, clock).await;
            }
        }
        Err(e) => tracing::error!(error = %e, "webhook_retry: failed to fetch pending deliveries"),
    }

    // Then, retry any Retrying deliveries that are due.
    match db.get_due_webhook_retries() {
        Ok(due) => {
            for delivery in due.into_iter().filter(|d| is_retry_due(d, clock)) {
                attempt_delivery(db, delivery, clock).await;
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

async fn attempt_delivery(db: &Arc<Db>, mut delivery: WebhookDelivery, clock: &dyn Clock) {
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

    let now = clock.now();
    let attempt_log = WebhookAttempt {
        attempted_at: now,
        http_status,
        response_body: response_body.clone(),
        error: error.clone(),
    };
    delivery.attempts.push(attempt_log);

    let success = http_status.map_or(false, |s| (200..300).contains(&s));

    match apply_attempt_outcome(&mut delivery, success, clock) {
        AttemptOutcome::Delivered => {
            tracing::info!(
                delivery_id = %delivery.id,
                vault_id = %delivery.vault_id,
                attempt = attempt_number,
                "webhook_retry: delivered successfully"
            );

            record_timeline_event(db, &delivery, true).await;
        }
        AttemptOutcome::RetryScheduled { delay_secs } => {
            tracing::warn!(
                delivery_id = %delivery.id,
                vault_id = %delivery.vault_id,
                attempt = attempt_number,
                retry_in_secs = delay_secs,
                error = ?error,
                "webhook_retry: delivery failed, scheduling retry"
            );
        }
        AttemptOutcome::Exhausted => {
            tracing::error!(
                delivery_id = %delivery.id,
                vault_id = %delivery.vault_id,
                total_attempts = attempt_number,
                "webhook_retry: all retries exhausted — delivery permanently failed"
            );

            // Notify vault owner via email through the configured provider.
            notify_owner_delivery_failed(db, &delivery, http_status).await;
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

/// Returns true if a permanent-failure email may be sent for `owner` now,
/// recording the send time when allowed. Enforces `FAILURE_EMAIL_RATE_LIMIT`.
fn failure_email_allowed(owner: &str) -> bool {
    let now = Instant::now();
    let mut guard = match FAILURE_EMAIL_LAST_SENT.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    let map = guard.get_or_insert_with(HashMap::new);
    match map.get(owner) {
        Some(last) if now.duration_since(*last) < FAILURE_EMAIL_RATE_LIMIT => false,
        _ => {
            map.insert(owner.to_string(), now);
            true
        }
    }
}

/// Sends a permanent-failure notification email to the vault owner through the
/// configured email provider. Rate-limited per owner.
async fn notify_owner_delivery_failed(
    db: &Arc<Db>,
    delivery: &WebhookDelivery,
    last_status: Option<u16>,
) {
    let owner = match db.get_vault_owner_email(&delivery.vault_id) {
        Ok(Some(email)) => email,
        Ok(None) => {
            tracing::warn!(
                vault_id = %delivery.vault_id,
                "webhook_retry: no owner email on file, skipping failure notification"
            );
            return;
        }
        Err(e) => {
            tracing::error!(
                vault_id = %delivery.vault_id,
                error = %e,
                "webhook_retry: failed to resolve owner email"
            );
            return;
        }
    };

    if !failure_email_allowed(&owner) {
        tracing::info!(
            vault_id = %delivery.vault_id,
            owner = %owner,
            "webhook_retry: failure email rate-limited, skipping notification"
        );
        return;
    }

    let status_text = last_status
        .map(|s| s.to_string())
        .unwrap_or_else(|| "no response".to_string());
    let subject = format!(
        "Webhook delivery permanently failed for vault {}",
        delivery.vault_id
    );
    let body = format!(
        "A webhook for vault {} permanently failed after {} attempts.\n\n\
         Webhook URL: {}\n\
         Event type: {}\n\
         Last HTTP status: {}\n\
         Attempts: {}\n",
        delivery.vault_id,
        delivery.attempt_count,
        delivery.endpoint_url,
        delivery.event_type,
        status_text,
        delivery.attempt_count,
    );

    let provider = EmailProvider::from_env();
    if let Err(e) = provider.send(&owner, &subject, &body).await {
        tracing::error!(
            vault_id = %delivery.vault_id,
            owner = %owner,
            error = %e,
            "webhook_retry: failed to send permanent-failure email"
        );
    } else {
        tracing::info!(
            vault_id = %delivery.vault_id,
            owner = %owner,
            attempts = delivery.attempt_count,
            "webhook_retry: permanent-failure email sent to owner"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
<<<<<<< HEAD
    fn failure_email_rate_limit_allows_first_then_blocks() {
        let owner = format!("owner-{}", Uuid::new_v4());
        assert!(failure_email_allowed(&owner), "first email should be allowed");
        assert!(
            !failure_email_allowed(&owner),
            "second email within the window should be rate-limited"
=======
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

    // ── Backoff timing with a controllable clock (#1595) ─────────────────────

    struct MockClock(std::sync::Mutex<DateTime<Utc>>);

    impl MockClock {
        fn at(t: DateTime<Utc>) -> Self {
            Self(std::sync::Mutex::new(t))
        }
        fn set(&self, t: DateTime<Utc>) {
            *self.0.lock().unwrap() = t;
        }
    }

    impl Clock for MockClock {
        fn now(&self) -> DateTime<Utc> {
            *self.0.lock().unwrap()
        }
    }

    fn start_time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn pending_delivery() -> WebhookDelivery {
        WebhookDelivery {
            id: "d-clock".to_string(),
            vault_id: "v-clock".to_string(),
            event_type: "vault_released".to_string(),
            payload: serde_json::json!({}),
            endpoint_url: "https://example.com/hook".to_string(),
            status: WebhookDeliveryStatus::Pending,
            attempt_count: 0,
            next_retry_at: None,
            created_at: start_time(),
            attempts: Vec::new(),
        }
    }

    /// Fail every attempt, advancing the clock exactly to each scheduled retry.
    /// Returns the observed gap between consecutive attempts.
    fn drive_failures(delivery: &mut WebhookDelivery, clock: &MockClock) -> Vec<i64> {
        let mut gaps = Vec::new();
        let mut last_attempt_at = clock.now();
        loop {
            let outcome = apply_attempt_outcome(delivery, false, clock);
            let AttemptOutcome::RetryScheduled { .. } = outcome else {
                return gaps;
            };
            let next = delivery.next_retry_at.expect("retry must be scheduled");

            // Not due one second early; due exactly on schedule.
            clock.set(next - chrono::Duration::seconds(1));
            assert!(!is_retry_due(delivery, clock), "retry fired early");
            clock.set(next);
            assert!(is_retry_due(delivery, clock), "retry not due on schedule");

            gaps.push((next - last_attempt_at).num_seconds());
            last_attempt_at = next;
        }
    }

    #[test]
    fn test_backoff_intervals_follow_schedule() {
        let clock = MockClock::at(start_time());
        let mut delivery = pending_delivery();

        let gaps = drive_failures(&mut delivery, &clock);

        let expected: Vec<i64> = RETRY_DELAYS_SECS
            .iter()
            .map(|&d| i64::try_from(d).unwrap())
            .collect();
        assert_eq!(gaps, expected, "intervals must be 1m, 5m, 15m, 1h, 4h");
        assert!(
            gaps.windows(2).all(|w| w[1] > w[0]),
            "backoff intervals must strictly increase"
        );
        // Final retry lands 1m+5m+15m+1h+4h = 19_260s after the first attempt.
        assert_eq!((clock.now() - start_time()).num_seconds(), 19_260);
    }

    #[test]
    fn test_next_retry_is_relative_to_attempt_time() {
        let clock = MockClock::at(start_time());
        let mut delivery = pending_delivery();

        apply_attempt_outcome(&mut delivery, false, &clock);
        // A late flush (retry runs 10 min after it was due) shifts the next slot.
        let late = delivery.next_retry_at.unwrap() + chrono::Duration::minutes(10);
        clock.set(late);
        let outcome = apply_attempt_outcome(&mut delivery, false, &clock);

        assert_eq!(outcome, AttemptOutcome::RetryScheduled { delay_secs: 300 });
        assert_eq!(
            delivery.next_retry_at,
            Some(late + chrono::Duration::seconds(300))
        );
    }

    #[test]
    fn test_max_attempts_respected() {
        let clock = MockClock::at(start_time());
        let mut delivery = pending_delivery();

        let retries = drive_failures(&mut delivery, &clock).len();

        assert_eq!(
            u32::try_from(retries).unwrap(),
            MAX_ATTEMPTS - 1,
            "exactly 5 retries"
        );
        assert_eq!(delivery.attempt_count, MAX_ATTEMPTS);
        assert_eq!(delivery.status, WebhookDeliveryStatus::DeliveryFailed);
        assert!(delivery.next_retry_at.is_none());

        // Never becomes due again, however far the clock advances.
        clock.set(start_time() + chrono::Duration::days(365));
        assert!(!is_retry_due(&delivery, &clock));
    }

    #[test]
    fn test_retry_delay_secs_bounds() {
        assert_eq!(retry_delay_secs(0), None);
        for (i, &delay) in RETRY_DELAYS_SECS.iter().enumerate() {
            assert_eq!(retry_delay_secs(u32::try_from(i).unwrap() + 1), Some(delay));
        }
        assert_eq!(retry_delay_secs(MAX_ATTEMPTS), None);
        assert_eq!(retry_delay_secs(MAX_ATTEMPTS + 1), None);
    }

    #[test]
    fn test_success_mid_backoff_stops_retries() {
        let clock = MockClock::at(start_time());
        let mut delivery = pending_delivery();

        apply_attempt_outcome(&mut delivery, false, &clock);
        apply_attempt_outcome(&mut delivery, false, &clock);
        clock.set(delivery.next_retry_at.unwrap());
        let outcome = apply_attempt_outcome(&mut delivery, true, &clock);

        assert_eq!(outcome, AttemptOutcome::Delivered);
        assert_eq!(delivery.attempt_count, 3);
        assert_eq!(delivery.status, WebhookDeliveryStatus::Delivered);
        assert!(delivery.next_retry_at.is_none());
        assert!(!is_retry_due(&delivery, &clock));
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
>>>>>>> origin/main
        );
    }

    #[test]
    fn failure_email_rate_limit_is_per_owner() {
        let a = format!("owner-a-{}", Uuid::new_v4());
        let b = format!("owner-b-{}", Uuid::new_v4());
        assert!(failure_email_allowed(&a));
        assert!(failure_email_allowed(&b), "distinct owners are independent");
    }

    #[test]
    fn permanent_failure_email_body_includes_details() {
        let delivery = WebhookDelivery {
            id: Uuid::new_v4().to_string(),
            vault_id: "vault-1".to_string(),
            event_type: "release".to_string(),
            payload: serde_json::json!({}),
            endpoint_url: "https://example.com/hook".to_string(),
            status: WebhookDeliveryStatus::DeliveryFailed,
            attempt_count: MAX_ATTEMPTS,
            next_retry_at: None,
            created_at: Utc::now(),
            attempts: Vec::new(),
        };
        let status_text = Some(500u16).map(|s| s.to_string()).unwrap_or_default();
        let body = format!(
            "Webhook URL: {}\nLast HTTP status: {}\nAttempts: {}",
            delivery.endpoint_url, status_text, delivery.attempt_count
        );
        assert!(body.contains("https://example.com/hook"));
        assert!(body.contains("500"));
        assert!(body.contains(&MAX_ATTEMPTS.to_string()));
    }
}
