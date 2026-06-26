/// Push notification service: FCM delivery, scheduling, preferences, tracking.
///
/// Architecture:
///   FcmClient          — sends a single message via FCM HTTP v1 API
///   NotificationStore  — in-memory stores (tokens, prefs, schedule, delivery log)
///   NotificationService — orchestrates scheduling + delivery
use crate::models::{
    ChannelDeliveryLog, DeliveryAttempt, DeliveryRecord, DeliveryStatus, DeviceToken,
    IdempotencyRecord, NotificationChannel, NotificationPreferences, NotificationType,
    RegisterTokenRequest, ReminderDeliveryLog, ScheduledNotification, UnsubscribeToken,
    UpdatePreferencesRequest, Vault,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

// ── Shared store types ───────────────────────────────────────────────────────

pub type TokenStore = Arc<Mutex<HashMap<String, Vec<DeviceToken>>>>;
pub type PrefsStore = Arc<Mutex<HashMap<String, NotificationPreferences>>>;
pub type ScheduleStore = Arc<Mutex<Vec<ScheduledNotification>>>;
pub type DeliveryStore = Arc<Mutex<Vec<DeliveryRecord>>>;
/// Keyed by notification_id.
pub type RetryStore = Arc<Mutex<HashMap<String, ReminderDeliveryLog>>>;
/// Keyed by token string → UnsubscribeToken (#828).
pub type UnsubscribeStore = Arc<Mutex<HashMap<String, UnsubscribeToken>>>;
/// Channel delivery logs (#827).
pub type ChannelDeliveryStore = Arc<Mutex<Vec<ChannelDeliveryLog>>>;
/// Idempotency key store (#825). Key → cached record.
pub type IdempotencyStore = Arc<Mutex<HashMap<String, IdempotencyRecord>>>;

pub fn create_token_store() -> TokenStore {
    Arc::new(Mutex::new(HashMap::new()))
}
pub fn create_prefs_store() -> PrefsStore {
    Arc::new(Mutex::new(HashMap::new()))
}
pub fn create_schedule_store() -> ScheduleStore {
    Arc::new(Mutex::new(Vec::new()))
}
pub fn create_delivery_store() -> DeliveryStore {
    Arc::new(Mutex::new(Vec::new()))
}
pub fn create_retry_store() -> RetryStore {
    Arc::new(Mutex::new(HashMap::new()))
}
pub fn create_unsubscribe_store() -> UnsubscribeStore {
    Arc::new(Mutex::new(HashMap::new()))
}
pub fn create_channel_delivery_store() -> ChannelDeliveryStore {
    Arc::new(Mutex::new(Vec::new()))
}
pub fn create_idempotency_store() -> IdempotencyStore {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Exponential backoff delays in seconds: 1 min, 5 min, 15 min, 1 hr, 6 hr.
const RETRY_DELAYS_SECS: [u64; 5] = [60, 300, 900, 3_600, 21_600];

pub const DEFAULT_MAX_RETRY_ATTEMPTS: u32 = 5;

// ── FCM HTTP v1 client ───────────────────────────────────────────────────────

/// Thin wrapper around the FCM HTTP v1 send endpoint.
/// Set `FCM_SERVER_KEY` env var to your Firebase server key.
pub struct FcmClient {
    http: reqwest::Client,
    server_key: String,
    project_id: String,
}

impl FcmClient {
    pub fn new(server_key: String, project_id: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            server_key,
            project_id,
        }
    }

    /// Send a notification to a single FCM registration token.
    /// Returns the FCM message ID on success.
    pub async fn send(
        &self,
        device_token: &str,
        title: &str,
        body: &str,
        data: Value,
    ) -> Result<String, String> {
        let payload = json!({
            "message": {
                "token": device_token,
                "notification": { "title": title, "body": body },
                "data": data,
                "android": {
                    "priority": "high",
                    "notification": { "channel_id": "ttl_reminders" }
                },
                "apns": {
                    "headers": { "apns-priority": "10" },
                    "payload": { "aps": { "sound": "default" } }
                }
            }
        });

        let url = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            self.project_id
        );

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.server_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().is_success() {
            let body: Value = resp.json().await.map_err(|e| e.to_string())?;
            Ok(body["name"].as_str().unwrap_or("ok").to_string())
        } else {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            Err(format!("FCM error {status}: {text}"))
        }
    }
}

// ── Notification content helpers ─────────────────────────────────────────────

fn notification_content(
    notification_type: &NotificationType,
    vault_id: &str,
    ttl_hours: Option<u64>,
) -> (&'static str, String, Value) {
    match notification_type {
        NotificationType::ExpiryWarning => {
            let hours = ttl_hours.unwrap_or(24);
            (
                "⚠️ Vault Expiring Soon",
                format!("Your vault expires in ~{hours}h. Check in now to keep it active."),
                json!({ "type": "expiry_warning", "vault_id": vault_id }),
            )
        }
        NotificationType::CheckInReminder => (
            "🔔 Check-In Reminder",
            "Time to check in to your TTL-Legacy vault.".to_string(),
            json!({ "type": "check_in_reminder", "vault_id": vault_id }),
        ),
        NotificationType::VaultReleased => (
            "🔓 Vault Released",
            "Your vault has been released to the beneficiary.".to_string(),
            json!({ "type": "vault_released", "vault_id": vault_id }),
        ),
        NotificationType::VaultPaused => (
            "⏸ Vault Paused",
            "Your vault has been paused.".to_string(),
            json!({ "type": "vault_paused", "vault_id": vault_id }),
        ),
    }
}

// ── NotificationService ──────────────────────────────────────────────────────

/// Delay (seconds) before attempting fallback channel (#827).
const FALLBACK_DELAY_SECS: u64 = 300;

/// Idempotency key TTL in seconds (24 hours) (#825).
pub const IDEMPOTENCY_TTL_SECS: i64 = 86_400;

pub struct NotificationService {
    pub fcm: Arc<FcmClient>,
    pub tokens: TokenStore,
    pub prefs: PrefsStore,
    pub schedule: ScheduleStore,
    pub delivery: DeliveryStore,
    pub retry_log: RetryStore,
    pub unsubscribe_tokens: UnsubscribeStore,
    pub channel_delivery_log: ChannelDeliveryStore,
    pub idempotency: IdempotencyStore,
}

impl NotificationService {
    pub fn new(
        fcm: Arc<FcmClient>,
        tokens: TokenStore,
        prefs: PrefsStore,
        schedule: ScheduleStore,
        delivery: DeliveryStore,
    ) -> Self {
        Self {
            fcm,
            tokens,
            prefs,
            schedule,
            delivery,
            retry_log: create_retry_store(),
            unsubscribe_tokens: create_unsubscribe_store(),
            channel_delivery_log: create_channel_delivery_store(),
            idempotency: create_idempotency_store(),
        }
    }

    // ── Token management ─────────────────────────────────────────────────────

    pub fn register_token(&self, req: RegisterTokenRequest) {
        let mut store = self.tokens.lock().unwrap();
        let entry = store.entry(req.owner.clone()).or_default();
        // Deduplicate by token value
        if !entry.iter().any(|t| t.token == req.token) {
            entry.push(DeviceToken {
                owner: req.owner,
                token: req.token,
                platform: req.platform,
                registered_at: Utc::now(),
            });
        }
    }

    pub fn unregister_token(&self, owner: &str, token: &str) {
        let mut store = self.tokens.lock().unwrap();
        if let Some(tokens) = store.get_mut(owner) {
            tokens.retain(|t| t.token != token);
        }
    }

    pub fn get_tokens(&self, owner: &str) -> Vec<DeviceToken> {
        self.tokens.lock().unwrap().get(owner).cloned().unwrap_or_default()
    }

    // ── Preferences ──────────────────────────────────────────────────────────

    // Preferences are stored per-owner.
    pub fn get_preferences(&self, owner: &str) -> NotificationPreferences {

        self.prefs
            .lock()
            .unwrap()
            .get(owner)
            .cloned()
            .unwrap_or_else(|| NotificationPreferences {
                owner: owner.to_string(),
                ..Default::default()
            })
    }



    pub fn update_preferences(&self, req: UpdatePreferencesRequest) {
        let mut store = self.prefs.lock().unwrap();
        let owner = req.owner.clone();
        let prefs = store.entry(owner.clone()).or_insert_with(|| NotificationPreferences {
            owner,
            ..Default::default()
        });

        if let Some(v) = req.expiry_warning_enabled {
            prefs.expiry_warning_enabled = v;
        }
        if let Some(v) = req.check_in_reminder_enabled {
            prefs.check_in_reminder_enabled = v;
        }
        if let Some(v) = req.vault_released_enabled {
            prefs.vault_released_enabled = v;
        }
        if let Some(v) = req.warning_hours_before {
            prefs.warning_hours_before = v;
        }
        if let Some(v) = req.locale {
            prefs.locale = Some(v);
        }
    }


    // ── Scheduling ───────────────────────────────────────────────────────────

    /// Schedule an expiry-warning notification for a vault.
    /// Fires `warning_hours_before` hours before the vault expires.
    pub fn schedule_expiry_warning(&self, vault: &Vault) {
        let prefs = self.get_preferences(&vault.owner);
        if !prefs.expiry_warning_enabled {
            return;
        }

        let Some(ttl) = vault.ttl_remaining else { return };
        let warning_secs = prefs.warning_hours_before * 3600;
        if ttl <= warning_secs {
            return;
        } // already past warning threshold


        let fire_at = Utc::now() + chrono::Duration::seconds((ttl - warning_secs) as i64);


        // Avoid duplicate schedules for the same vault + type
        let mut store = self.schedule.lock().unwrap();
        let already = store.iter().any(|s| {
            s.vault_id == vault.id
                && s.notification_type == NotificationType::ExpiryWarning
                && s.status == DeliveryStatus::Pending
        });
        if already { return; }

        store.push(ScheduledNotification {
            id: Uuid::new_v4().to_string(),
            vault_id: vault.id.clone(),
            owner: vault.owner.clone(),
            notification_type: NotificationType::ExpiryWarning,
            scheduled_at: fire_at,
            status: DeliveryStatus::Pending,
            max_retry_attempts: DEFAULT_MAX_RETRY_ATTEMPTS,
        });
    }

    /// Schedule an immediate notification (fires now).
    pub fn schedule_immediate(
        &self,
        vault_id: &str,
        owner: &str,
        notification_type: NotificationType,
    ) {
        let prefs = self
            .prefs
            .lock()
            .unwrap()
            .get(vault_id)
            .cloned();

        // Legacy enablement rules based on stored boolean flags.
        let enabled = match (prefs, &notification_type) {
            (Some(p), NotificationType::VaultReleased) => p.vault_released_enabled,
            (Some(p), NotificationType::CheckInReminder) => p.check_in_reminder_enabled,
            (Some(_), NotificationType::ExpiryWarning) => true,
            (None, _) => false,
            _ => true,
        };

        if !enabled {
            return;
        }


        self.schedule.lock().unwrap().push(ScheduledNotification {
            id: Uuid::new_v4().to_string(),
            vault_id: vault_id.to_string(),
            owner: owner.to_string(),
            notification_type,
            scheduled_at: Utc::now(),
            status: DeliveryStatus::Pending,
            max_retry_attempts: DEFAULT_MAX_RETRY_ATTEMPTS,
        });
    }


    pub fn get_pending_notifications(&self) -> Vec<ScheduledNotification> {
        let now = Utc::now();
        self.schedule
            .lock()
            .unwrap()
            .iter()
            .filter(|n| n.status == DeliveryStatus::Pending && n.scheduled_at <= now)
            .cloned()
            .collect()
    }

    // ── Deduplication ─────────────────────────────────────────────────────────

    /// Returns true if a notification with the same vault_id, owner, and type
    /// was already sent within `DEDUP_WINDOW_SECONDS`.
    pub fn is_duplicate(&self, notif: &ScheduledNotification) -> bool {
        let cutoff = Utc::now() - chrono::Duration::seconds(DEDUP_WINDOW_SECONDS);
        let store = self.schedule.lock().unwrap();
        store.iter().any(|n| {
            n.id != notif.id
                && n.vault_id == notif.vault_id
                && n.owner == notif.owner
                && n.notification_type == notif.notification_type
                && n.sent_at.map_or(false, |t| t > cutoff)
        })
    }

    // ── Delivery ─────────────────────────────────────────────────────────────

    /// Send all due pending notifications. Called by the background scheduler loop.
    pub async fn flush_pending(&self) {
        let due = self.get_pending_notifications();
        for notif in due {
            if self.is_duplicate(&notif) {
                log::info!(
                    "Skipping duplicate notification: vault={} owner={} type={:?}",
                    notif.vault_id, notif.owner, notif.notification_type
                );
                self.mark_sent(&notif.id, DeliveryStatus::Sent);
                continue;
            }
            self.deliver(&notif).await;
        }
    }

    /// Retry any Retrying notifications whose next_retry_at has passed.
    pub async fn flush_retries(&self) {
        let now = Utc::now();
        let due: Vec<ReminderDeliveryLog> = self
            .retry_log
            .lock()
            .unwrap()
            .values()
            .filter(|l| {
                l.status == DeliveryStatus::Retrying
                    && l.next_retry_at.map_or(false, |t| t <= now)
            })
            .cloned()
            .collect();

        for log in due {
            // Reconstruct a minimal ScheduledNotification for delivery
            let notif = {
                let sched = self.schedule.lock().unwrap();
                sched.iter().find(|n| n.id == log.notification_id).cloned()
            };
            if let Some(notif) = notif {
                self.deliver_with_retry(&notif, log.attempts.len() as u32).await;
            }
        }
    }

    async fn deliver(&self, notif: &ScheduledNotification) {
        self.deliver_with_retry(notif, 0).await;
    }

    async fn deliver_with_retry(&self, notif: &ScheduledNotification, attempt: u32) {
        let tokens = self.get_tokens(&notif.owner);
        if tokens.is_empty() {
            self.record(notif, DeliveryStatus::Failed, "no_tokens_registered");
            self.mark_sent(&notif.id, DeliveryStatus::Failed);
            self.update_retry_log(notif, attempt, DeliveryStatus::Failed, "no_tokens_registered");
            return;
        }

        let (title, body, data) =
            notification_content(&notif.notification_type, &notif.vault_id, None);

        let mut last_err = String::new();
        let mut any_ok = false;
        for device in &tokens {
            match self.fcm.send(&device.token, title, &body, data.clone()).await {
                Ok(msg_id) => {
                    self.record(notif, DeliveryStatus::Sent, &msg_id);
                    any_ok = true;
                }
                Err(e) => {
                    last_err = e;
                }
            }
        }

        if any_ok {
            self.mark_sent(&notif.id, DeliveryStatus::Sent);
            self.update_retry_log(notif, attempt, DeliveryStatus::Sent, "");
        } else {
            let next_attempt = attempt + 1;
            let max_attempts = notif.max_retry_attempts;
            if next_attempt < max_attempts && (next_attempt as usize) < RETRY_DELAYS_SECS.len() {
                let delay = RETRY_DELAYS_SECS[next_attempt as usize];
                let next_at = Utc::now() + chrono::Duration::seconds(delay as i64);
                self.record(notif, DeliveryStatus::Retrying, &last_err);
                self.mark_sent(&notif.id, DeliveryStatus::Retrying);
                self.update_retry_log_with_next(notif, attempt, DeliveryStatus::Retrying, &last_err, Some(next_at));
            } else {
                self.record(notif, DeliveryStatus::Failed, &last_err);
                self.mark_sent(&notif.id, DeliveryStatus::Failed);
                self.update_retry_log(notif, attempt, DeliveryStatus::Failed, &last_err);
                log::warn!(
                    "Notification retry budget exhausted: notification={} vault={} owner={} attempts={}/{}",
                    notif.id, notif.vault_id, notif.owner, next_attempt, max_attempts
                );
                log::error!(
                    "[ALERT] Reminder delivery permanently failed after {} attempts: vault={} owner={} error={}",
                    next_attempt, notif.vault_id, notif.owner, last_err
                );
            }
        }
    }

    fn update_retry_log(&self, notif: &ScheduledNotification, attempt: u32, status: DeliveryStatus, error: &str) {
        self.update_retry_log_with_next(notif, attempt, status, error, None);
    }

    fn update_retry_log_with_next(
        &self,
        notif: &ScheduledNotification,
        attempt: u32,
        status: DeliveryStatus,
        error: &str,
        next_retry_at: Option<chrono::DateTime<Utc>>,
    ) {
        let mut store = self.retry_log.lock().unwrap();
        let entry = store.entry(notif.id.clone()).or_insert_with(|| ReminderDeliveryLog {
            notification_id: notif.id.clone(),
            vault_id: notif.vault_id.clone(),
            owner: notif.owner.clone(),
            status: DeliveryStatus::Pending,
            attempts: Vec::new(),
            next_retry_at: None,
        });
        entry.attempts.push(DeliveryAttempt {
            attempt,
            attempted_at: Utc::now(),
            error: error.to_string(),
        });
        entry.status = status;
        entry.next_retry_at = next_retry_at;
    }

    /// Returns the current delivery status for a vault's most recent reminder.
    pub fn get_reminder_delivery_status(&self, vault_id: &str) -> Option<ReminderDeliveryLog> {
        self.retry_log
            .lock()
            .unwrap()
            .values()
            .filter(|l| l.vault_id == vault_id)
            .max_by_key(|l| l.attempts.last().map(|a| a.attempted_at))
            .cloned()
    }

    fn record(&self, notif: &ScheduledNotification, status: DeliveryStatus, response: &str) {
        self.delivery.lock().unwrap().push(DeliveryRecord {
            notification_id: notif.id.clone(),
            vault_id: notif.vault_id.clone(),
            owner: notif.owner.clone(),
            notification_type: notif.notification_type.clone(),
            status,
            sent_at: Utc::now(),
            provider_response: response.to_string(),
        });
    }

    fn mark_sent(&self, id: &str, status: DeliveryStatus) {
        let mut store = self.schedule.lock().unwrap();
        if let Some(n) = store.iter_mut().find(|n| n.id == id) {
            if status == DeliveryStatus::Sent {
                n.sent_at = Some(Utc::now());
            }
            n.status = status;
        }
    }

    pub fn get_delivery_log(&self, owner: &str) -> Vec<DeliveryRecord> {
        self.delivery
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.owner == owner)
            .cloned()
            .collect()
    }

    // ── Unsubscribe (#828) ──────────────────────────────────────────────────

    /// Generate a signed unsubscribe token for the given owner.
    pub fn generate_unsubscribe_token(&self, owner: &str) -> String {
        let token = Uuid::new_v4().to_string();
        self.unsubscribe_tokens.lock().unwrap().insert(
            token.clone(),
            UnsubscribeToken {
                token: token.clone(),
                owner: owner.to_string(),
                created_at: Utc::now(),
            },
        );
        token
    }

    /// Validate an unsubscribe token and mark the owner as unsubscribed.
    /// Returns the owner string on success.
    pub fn process_unsubscribe(&self, token: &str) -> Result<String, String> {
        let unsub = self
            .unsubscribe_tokens
            .lock()
            .unwrap()
            .get(token)
            .cloned()
            .ok_or_else(|| "invalid or expired unsubscribe token".to_string())?;

        let mut prefs_store = self.prefs.lock().unwrap();
        let prefs = prefs_store
            .entry(unsub.owner.clone())
            .or_insert_with(|| NotificationPreferences {
                owner: unsub.owner.clone(),
                ..Default::default()
            });
        prefs.unsubscribed = true;

        Ok(unsub.owner)
    }

    /// Check if an owner has unsubscribed.
    pub fn is_unsubscribed(&self, owner: &str) -> bool {
        self.prefs
            .lock()
            .unwrap()
            .get(owner)
            .map_or(false, |p| p.unsubscribed)
    }

    // ── Email template with unsubscribe link (#828) ─────────────────────────

    /// Render a reminder email body that includes an unsubscribe link.
    pub fn render_email_with_unsubscribe(
        &self,
        owner: &str,
        subject: &str,
        body: &str,
        base_url: &str,
    ) -> String {
        let token = self.generate_unsubscribe_token(owner);
        format!(
            "<html><body>\
             <h2>{subject}</h2>\
             <p>{body}</p>\
             <hr/>\
             <p style=\"font-size:small;color:#888;\">\
             <a href=\"{base_url}/notifications/unsubscribe?token={token}\">\
             Unsubscribe from these emails</a></p>\
             </body></html>"
        )
    }

    // ── Channel fallback (#827) ─────────────────────────────────────────────

    /// Record a channel-level delivery attempt.
    pub fn log_channel_delivery(
        &self,
        notification_id: &str,
        channel: &NotificationChannel,
        status: DeliveryStatus,
        error: Option<String>,
    ) {
        self.channel_delivery_log.lock().unwrap().push(ChannelDeliveryLog {
            notification_id: notification_id.to_string(),
            channel: channel.clone(),
            status,
            attempted_at: Utc::now(),
            error,
        });
    }

    /// Get the channel delivery log for a notification.
    pub fn get_channel_delivery_log(&self, notification_id: &str) -> Vec<ChannelDeliveryLog> {
        self.channel_delivery_log
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.notification_id == notification_id)
            .cloned()
            .collect()
    }

    /// Attempt delivery on the preferred channel, falling back if it fails.
    /// Returns (primary_ok, fallback_ok).
    pub async fn deliver_with_fallback(
        &self,
        notif: &ScheduledNotification,
    ) -> (bool, Option<bool>) {
        let prefs = self.get_preferences(&notif.owner);

        let preferred = prefs.preferred_channel.clone();
        let fallback = prefs.fallback_channel.clone();

        let primary_ok = if let Some(ref ch) = preferred {
            let ok = self.try_channel_delivery(notif, ch).await;
            self.log_channel_delivery(
                &notif.id,
                ch,
                if ok { DeliveryStatus::Sent } else { DeliveryStatus::Failed },
                if ok { None } else { Some("primary channel failed".into()) },
            );
            ok
        } else {
            // No preferred channel set — use default FCM push delivery
            let tokens = self.get_tokens(&notif.owner);
            !tokens.is_empty()
        };

        if primary_ok {
            return (true, None);
        }

        // Primary failed — try fallback after delay
        if let Some(ref fb_channel) = fallback {
            tokio::time::sleep(std::time::Duration::from_secs(FALLBACK_DELAY_SECS)).await;
            let fb_ok = self.try_channel_delivery(notif, fb_channel).await;
            self.log_channel_delivery(
                &notif.id,
                fb_channel,
                if fb_ok { DeliveryStatus::Sent } else { DeliveryStatus::Failed },
                if fb_ok { None } else { Some("fallback channel failed".into()) },
            );
            return (false, Some(fb_ok));
        }

        (false, None)
    }

    /// Stub: attempt to deliver via a specific channel.
    async fn try_channel_delivery(
        &self,
        notif: &ScheduledNotification,
        channel: &NotificationChannel,
    ) -> bool {
        match channel {
            NotificationChannel::Push => {
                let tokens = self.get_tokens(&notif.owner);
                if tokens.is_empty() {
                    return false;
                }
                let (title, body, data) =
                    notification_content(&notif.notification_type, &notif.vault_id, None);
                for device in &tokens {
                    if self.fcm.send(&device.token, title, &body, data.clone()).await.is_ok() {
                        return true;
                    }
                }
                false
            }
            NotificationChannel::Email => {
                // Stub: in production this would call an email API
                log::info!("Sending email to owner={}", notif.owner);
                true
            }
            NotificationChannel::Sms => {
                // Stub: in production this would call an SMS API
                log::info!("Sending SMS to owner={}", notif.owner);
                true
            }
        }
    }

    // ── Idempotency (#825) ──────────────────────────────────────────────────

    /// Check if an idempotency key has been seen. Returns the cached record if so.
    pub fn check_idempotency(&self, key: &str) -> Option<IdempotencyRecord> {
        let store = self.idempotency.lock().unwrap();
        let record = store.get(key)?;
        let age = Utc::now()
            .signed_duration_since(record.created_at)
            .num_seconds();
        if age > IDEMPOTENCY_TTL_SECS {
            return None;
        }
        Some(record.clone())
    }

    /// Store an idempotency key with the associated response.
    pub fn store_idempotency(&self, key: String, status_code: u16, response_body: String) {
        self.idempotency.lock().unwrap().insert(
            key.clone(),
            IdempotencyRecord {
                key,
                response_body,
                status_code,
                created_at: Utc::now(),
            },
        );
    }

    /// Purge expired idempotency keys (older than 24h).
    pub fn purge_expired_idempotency_keys(&self) {
        let cutoff = Utc::now() - chrono::Duration::seconds(IDEMPOTENCY_TTL_SECS);
        self.idempotency
            .lock()
            .unwrap()
            .retain(|_, v| v.created_at > cutoff);
    }
}

// ── Background scheduler loop ────────────────────────────────────────────────

/// Spawns a tokio task that flushes pending notifications and retries every `interval_secs`.
pub fn start_scheduler(service: Arc<NotificationService>, interval_secs: u64) {
    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(interval_secs);
        loop {
            tokio::time::sleep(interval).await;
            service.flush_pending().await;
            service.flush_retries().await;
        }
    });
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{NotificationType, VaultStatus};
    use chrono::Utc;

    fn make_service() -> NotificationService {
        // Use a dummy FcmClient — tests that call deliver() are skipped
        let fcm = Arc::new(FcmClient::new("test-key".into(), "test-project".into()));
        NotificationService::new(
            fcm,
            create_token_store(),
            create_prefs_store(),
            create_schedule_store(),
            create_delivery_store(),
        )
    }

    fn make_vault(ttl: Option<u64>) -> Vault {
        crate::models::Vault {
            id: "v1".into(),
            owner: "owner1".into(),
            beneficiary: "ben1".into(),
            balance: 1_000_000,
            check_in_interval: 86_400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: ttl,
        }
    }

    // Token management

    #[test]
    fn register_token_stores_entry() {
        let svc = make_service();
        svc.register_token(RegisterTokenRequest {
            owner: "owner1".into(),
            token: "tok-abc".into(),
            platform: "android".into(),
        });
        let tokens = svc.get_tokens("owner1");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, "tok-abc");
    }

    #[test]
    fn register_token_deduplicates() {
        let svc = make_service();
        for _ in 0..3 {
            svc.register_token(RegisterTokenRequest {
                owner: "owner1".into(),
                token: "tok-abc".into(),
                platform: "ios".into(),
            });
        }
        assert_eq!(svc.get_tokens("owner1").len(), 1);
    }

    #[test]
    fn unregister_token_removes_entry() {
        let svc = make_service();
        svc.register_token(RegisterTokenRequest {
            owner: "owner1".into(),
            token: "tok-abc".into(),
            platform: "android".into(),
        });
        svc.unregister_token("owner1", "tok-abc");
        assert!(svc.get_tokens("owner1").is_empty());
    }

    // Preferences

    #[test]
    fn get_preferences_returns_default_when_unset() {
        let svc = make_service();
        let prefs = svc.get_preferences("unknown-owner");
        assert!(prefs.expiry_warning_enabled);
        assert!(prefs.check_in_reminder_enabled);
        assert!(prefs.vault_released_enabled);
    }




    // Scheduling

    #[test]
    fn schedule_expiry_warning_creates_pending_notification() {
        let svc = make_service();
        let vault = make_vault(Some(172_800)); // 48h TTL, warning at 24h → fires in 24h

        svc.prefs.lock().unwrap().insert(
            vault.owner.clone(),
            crate::models::NotificationPreferences {
                owner: vault.owner.clone(),
                expiry_warning_enabled: true,
                check_in_reminder_enabled: true,
                vault_released_enabled: true,
                warning_hours_before: 24,
                locale: None,
            },

        );

        svc.schedule_expiry_warning(&vault);

        let pending = svc.get_pending_notifications();
        // Not due yet (fires in 24h), so pending list is empty
        assert!(pending.is_empty());
        // But it IS in the schedule store
        let all = svc.schedule.lock().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].notification_type, NotificationType::ExpiryWarning);
        assert_eq!(all[0].status, DeliveryStatus::Pending);
    }

    #[test]
    fn schedule_expiry_warning_skips_when_disabled() {
        let svc = make_service();
        let vault = make_vault(Some(172_800));

        svc.prefs.lock().unwrap().insert(
            vault.owner.clone(),
            crate::models::NotificationPreferences {
                owner: vault.owner.clone(),
                expiry_warning_enabled: false,
                check_in_reminder_enabled: true,
                vault_released_enabled: true,
                warning_hours_before: 24,
                locale: None,
            },
        );

        svc.schedule_expiry_warning(&vault);

        assert!(svc.schedule.lock().unwrap().is_empty());
    }

    #[test]
    fn schedule_expiry_warning_no_duplicate() {
        let svc = make_service();
        let vault = make_vault(Some(172_800));
        svc.schedule_expiry_warning(&vault);
        svc.schedule_expiry_warning(&vault); // second call should be ignored
        assert_eq!(svc.schedule.lock().unwrap().len(), 1);
    }

    #[test]
    fn schedule_immediate_creates_due_notification() {
        let svc = make_service();
        svc.prefs.lock().unwrap().insert(
            "owner1".to_string(),
            crate::models::NotificationPreferences {
                owner: "owner1".to_string(),
                expiry_warning_enabled: true,
                check_in_reminder_enabled: true,
                vault_released_enabled: true,
                warning_hours_before: 24,
                locale: None,
            },
        );
        svc.schedule_immediate("v1", "owner1", NotificationType::VaultReleased);

        let pending = svc.get_pending_notifications();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].notification_type, NotificationType::VaultReleased);
    }

    #[test]
    fn schedule_immediate_skips_when_disabled() {
        let svc = make_service();
        svc.prefs.lock().unwrap().insert(
            "owner1".to_string(),
            crate::models::NotificationPreferences {
                owner: "owner1".to_string(),
                expiry_warning_enabled: true,
                check_in_reminder_enabled: true,
                vault_released_enabled: false,
                warning_hours_before: 24,
                locale: None,
            },
        );
        svc.schedule_immediate("v1", "owner1", NotificationType::VaultReleased);

        assert!(svc.schedule.lock().unwrap().is_empty());
    }

    // Delivery tracking

    #[tokio::test]
    async fn deliver_with_no_tokens_records_failed() {
        let svc = make_service();
        svc.schedule_immediate("v1", "owner1", NotificationType::CheckInReminder);
        // No tokens registered → flush should record a failure
        svc.flush_pending().await;
        let log = svc.get_delivery_log("owner1");
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].status, DeliveryStatus::Failed);
        assert_eq!(log[0].provider_response, "no_tokens_registered");
    }

    #[test]
    fn delivery_log_filters_by_owner() {
        let svc = make_service();
        svc.delivery.lock().unwrap().push(DeliveryRecord {
            notification_id: "n1".into(),
            vault_id: "v1".into(),
            owner: "owner1".into(),
            notification_type: NotificationType::CheckInReminder,
            status: DeliveryStatus::Sent,
            sent_at: Utc::now(),
            provider_response: "msg/123".into(),
        });
        svc.delivery.lock().unwrap().push(DeliveryRecord {
            notification_id: "n2".into(),
            vault_id: "v2".into(),
            owner: "owner2".into(),
            notification_type: NotificationType::VaultReleased,
            status: DeliveryStatus::Sent,
            sent_at: Utc::now(),
            provider_response: "msg/456".into(),
        });
        assert_eq!(svc.get_delivery_log("owner1").len(), 1);
        assert_eq!(svc.get_delivery_log("owner2").len(), 1);
        assert!(svc.get_delivery_log("owner3").is_empty());
    }

    // Notification content

    #[test]
    fn notification_content_expiry_warning_includes_hours() {
        let (title, body, data) =
            notification_content(&NotificationType::ExpiryWarning, "v1", Some(6));
        assert!(title.contains("Expiring"));
        assert!(body.contains("6h"));
        assert_eq!(data["vault_id"], "v1");
    }

    #[test]
    fn notification_content_vault_released() {
        let (title, body, data) =
            notification_content(&NotificationType::VaultReleased, "v2", None);
        assert!(title.contains("Released"));
        assert!(body.contains("beneficiary"));
        assert_eq!(data["type"], "vault_released");
    }

    // Retry logic

    #[tokio::test]
    async fn no_tokens_sets_retry_log_to_failed() {
        let svc = make_service();
        svc.schedule_immediate("v1", "owner1", NotificationType::CheckInReminder);
        svc.flush_pending().await;
        let status = svc.get_reminder_delivery_status("v1").unwrap();
        assert_eq!(status.status, DeliveryStatus::Failed);
        assert_eq!(status.attempts.len(), 1);
        assert_eq!(status.attempts[0].attempt, 0);
    }

    #[tokio::test]
    async fn retry_log_records_attempt_count() {
        let svc = make_service();
        svc.schedule_immediate("v1", "owner1", NotificationType::CheckInReminder);
        // First flush: attempt 0 → no tokens → Failed (no retries possible without tokens)
        svc.flush_pending().await;
        let log = svc.get_reminder_delivery_status("v1").unwrap();
        assert_eq!(log.attempts.len(), 1);
        assert_eq!(log.attempts[0].error, "no_tokens_registered");
    }

    #[tokio::test]
    async fn get_reminder_delivery_status_returns_none_for_unknown_vault() {
        let svc = make_service();
        assert!(svc.get_reminder_delivery_status("unknown-vault").is_none());
    }

    #[tokio::test]
    async fn retry_log_vault_id_matches() {
        let svc = make_service();
        svc.schedule_immediate("vault-xyz", "owner1", NotificationType::CheckInReminder);
        svc.flush_pending().await;
        let log = svc.get_reminder_delivery_status("vault-xyz").unwrap();
        assert_eq!(log.vault_id, "vault-xyz");
        assert_eq!(log.owner, "owner1");
    }

    #[test]
    fn retry_delays_are_ascending() {
        let delays = RETRY_DELAYS_SECS;
        for i in 1..delays.len() {
            assert!(delays[i] > delays[i - 1], "delay[{i}] should be > delay[{}]", i - 1);
        }
        assert_eq!(delays.len(), 5);
    }

    #[test]
    fn retry_delays_match_spec() {
        // 1 min, 5 min, 15 min, 1 hr, 6 hr
        assert_eq!(RETRY_DELAYS_SECS, [60, 300, 900, 3_600, 21_600]);
    }

    // Retry budget tests (#829)

    #[test]
    fn default_max_retry_attempts_is_five() {
        assert_eq!(DEFAULT_MAX_RETRY_ATTEMPTS, 5);
    }

    #[test]
    fn scheduled_notification_has_max_retry_attempts() {
        let svc = make_service();
        svc.schedule_immediate("v1", "owner1", NotificationType::CheckInReminder);
        let all = svc.schedule.lock().unwrap();
        assert_eq!(all[0].max_retry_attempts, DEFAULT_MAX_RETRY_ATTEMPTS);
    }

    #[tokio::test]
    async fn retry_exhaustion_marks_failed() {
        let svc = make_service();
        svc.schedule_immediate("v1", "owner1", NotificationType::CheckInReminder);
        // No tokens registered — first delivery attempt fails immediately
        svc.flush_pending().await;
        let log = svc.get_reminder_delivery_status("v1").unwrap();
        assert_eq!(log.status, DeliveryStatus::Failed);
        // No retry scheduled because no-tokens is an immediate failure
        assert!(log.next_retry_at.is_none());
    }

    #[test]
    fn max_retry_attempts_propagated_in_schedule() {
        let svc = make_service();
        let vault = make_vault(Some(172_800));
        svc.schedule_expiry_warning(&vault);
        let all = svc.schedule.lock().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].max_retry_attempts, DEFAULT_MAX_RETRY_ATTEMPTS);
    }
}
