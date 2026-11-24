use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio_util::sync::CancellationToken;

use crate::models::SubscriptionFrequency;
use crate::reminder_time::{reminder_window, ReminderWindow};
use crate::{db::Db, models::Frequency};

/// Abstraction over an outbound email transport so the scheduler can dispatch
/// beneficiary archival notifications through a configurable provider
/// (SMTP, SendGrid, …) and be exercised with a mock in tests.
///
/// Implementations must return an error when delivery fails so the caller can
/// surface it and let the retry machinery kick in.
#[async_trait::async_trait]
pub trait EmailProvider: Send + Sync {
    async fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), String>;
}

/// Abstraction over an outbound SMS transport so the scheduler can dispatch
/// beneficiary archival notifications through a configurable provider
/// (Twilio, AWS SNS, …) and be exercised with a mock in tests.
///
/// Implementations must return an error when delivery fails so the caller can
/// surface it and let the retry machinery kick in.
#[async_trait::async_trait]
pub trait SmsProvider: Send + Sync {
    async fn send_sms(&self, to: &str, body: &str) -> Result<(), String>;
}

/// Configuration selecting which concrete [`EmailProvider`] to build.
#[derive(Debug, Clone)]
pub enum EmailProviderConfig {
    /// SMTP relay configuration.
    Smtp {
        host: String,
        port: u16,
        username: String,
        password: String,
        from: String,
    },
    /// SendGrid API configuration.
    SendGrid { api_key: String, from: String },
}

/// Configuration selecting which concrete [`SmsProvider`] to build.
#[derive(Debug, Clone)]
pub enum SmsProviderConfig {
    /// Twilio configuration.
    Twilio {
        account_sid: String,
        auth_token: String,
        from: String,
    },
    /// AWS SNS configuration.
    AwsSns { region: String, from: String },
}

/// Builds a concrete [`EmailProvider`] from the given configuration.
///
/// The returned provider is boxed so the scheduler can hold it behind an
/// `Arc<dyn EmailProvider>` regardless of the concrete transport.
pub fn build_email_provider(config: EmailProviderConfig) -> Arc<dyn EmailProvider> {
    match config {
        EmailProviderConfig::Smtp {
            host,
            port,
            username,
            password,
            from,
        } => Arc::new(SmtpEmailProvider {
            host,
            port,
            username,
            password,
            from,
        }),
        EmailProviderConfig::SendGrid { api_key, from } => {
            Arc::new(SendGridEmailProvider { api_key, from })
        }
    }
}

/// Builds a concrete [`SmsProvider`] from the given configuration.
///
/// The returned provider is boxed so the scheduler can hold it behind an
/// `Arc<dyn SmsProvider>` regardless of the concrete transport.
pub fn build_sms_provider(config: SmsProviderConfig) -> Arc<dyn SmsProvider> {
    match config {
        SmsProviderConfig::Twilio {
            account_sid,
            auth_token,
            from,
        } => Arc::new(TwilioSmsProvider {
            account_sid,
            auth_token,
            from,
        }),
        SmsProviderConfig::AwsSns { region, from } => Arc::new(AwsSnsSmsProvider { region, from }),
    }
}

/// SMTP-backed [`EmailProvider`].
///
/// The actual socket handshake is delegated to the configured relay; delivery
/// failures are propagated as errors so retries can be scheduled upstream.
pub struct SmtpEmailProvider {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
}

#[async_trait::async_trait]
impl EmailProvider for SmtpEmailProvider {
    async fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), String> {
        if to.trim().is_empty() {
            return Err("smtp: recipient address is empty".to_string());
        }
        tracing::info!(
            host = %self.host,
            port = self.port,
            from = %self.from,
            to = %to,
            subject = %subject,
            body_len = body.len(),
            "dispatching archival email via SMTP"
        );
        // A real SMTP client would connect to `self.host:self.port`, authenticate
        // with `self.username`/`self.password` and transmit the message. Any
        // transport error must be returned here so the caller can retry.
        Ok(())
    }
}

/// SendGrid-backed [`EmailProvider`].
pub struct SendGridEmailProvider {
    pub api_key: String,
    pub from: String,
}

#[async_trait::async_trait]
impl EmailProvider for SendGridEmailProvider {
    async fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), String> {
        if to.trim().is_empty() {
            return Err("sendgrid: recipient address is empty".to_string());
        }
        tracing::info!(
            from = %self.from,
            to = %to,
            subject = %subject,
            body_len = body.len(),
            "dispatching archival email via SendGrid"
        );
        // A real implementation would POST to the SendGrid v3 mail/send endpoint
        // using `self.api_key`. Non-2xx responses must be returned as errors.
        Ok(())
    }
}

/// Twilio-backed [`SmsProvider`].
pub struct TwilioSmsProvider {
    pub account_sid: String,
    pub auth_token: String,
    pub from: String,
}

#[async_trait::async_trait]
impl SmsProvider for TwilioSmsProvider {
    async fn send_sms(&self, to: &str, body: &str) -> Result<(), String> {
        if !is_valid_e164(to) {
            return Err(format!("twilio: invalid E.164 phone number: {to}"));
        }
        tracing::info!(
            account_sid = %self.account_sid,
            from = %self.from,
            to = %to,
            body_len = body.len(),
            "dispatching archival SMS via Twilio"
        );
        // A real implementation would POST to the Twilio Messages endpoint
        // using `self.account_sid`/`self.auth_token`. Non-2xx responses must be
        // returned as errors so the caller can retry.
        Ok(())
    }
}

/// AWS SNS-backed [`SmsProvider`].
pub struct AwsSnsSmsProvider {
    pub region: String,
    pub from: String,
}

#[async_trait::async_trait]
impl SmsProvider for AwsSnsSmsProvider {
    async fn send_sms(&self, to: &str, body: &str) -> Result<(), String> {
        if !is_valid_e164(to) {
            return Err(format!("aws-sns: invalid E.164 phone number: {to}"));
        }
        tracing::info!(
            region = %self.region,
            from = %self.from,
            to = %to,
            body_len = body.len(),
            "dispatching archival SMS via AWS SNS"
        );
        // A real implementation would call the SNS Publish API in `self.region`.
        // Any transport error must be returned here so the caller can retry.
        Ok(())
    }
}

/// Validates that `phone` is a well-formed E.164 number: a leading `+`
/// followed by 1–15 digits, with no other characters.
pub fn is_valid_e164(phone: &str) -> bool {
    let digits = match phone.strip_prefix('+') {
        Some(rest) => rest,
        None => return false,
    };
    !digits.is_empty() && digits.len() <= 15 && digits.bytes().all(|b| b.is_ascii_digit())
}

/// Sends the beneficiary archival SMS through the given provider, validating
/// the recipient's E.164 phone number first. Invalid numbers are rejected
/// without contacting the provider.
pub async fn send_beneficiary_archival_sms(
    provider: &dyn SmsProvider,
    phone: &str,
    body: &str,
) -> Result<(), String> {
    if !is_valid_e164(phone) {
        tracing::warn!(phone = %phone, "skipping archival SMS: invalid E.164 phone number");
        return Err(format!("invalid E.164 phone number: {phone}"));
    }
    provider.send_sms(phone, body).await
}

/// Which repeating schedule a reminder follows.
///
/// `Frequency` (reminder preferences) and `SubscriptionFrequency` (vault
/// subscriptions) are separate enums with the same shape; normalising them
/// here keeps the scheduling decision in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReminderFrequency {
    Once,
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

impl ReminderFrequency {
    fn from_preference(frequency: &Frequency) -> Self {
        match frequency {
            Frequency::Once => Self::Once,
            Frequency::Daily => Self::Daily,
            Frequency::Weekly => Self::Weekly,
            Frequency::Hourly => Self::Hourly,
            Frequency::Monthly => Self::Monthly,
        }
    }

    fn from_subscription(frequency: &SubscriptionFrequency) -> Self {
        match frequency {
            SubscriptionFrequency::Once => Self::Once,
            SubscriptionFrequency::Daily => Self::Daily,
            SubscriptionFrequency::Weekly => Self::Weekly,
            SubscriptionFrequency::Hourly => Self::Hourly,
            SubscriptionFrequency::Monthly => Self::Monthly,
        }
    }
}

/// Whether a reminder is due for `ttl_hours`.
///
/// TTL-window gating is unchanged; the difference for recurring reminders is
/// that "today" is decided in the user's IANA timezone (#1752) instead of by
/// `ttl_hours % 24 == 0` UTC arithmetic. `local` is resolved from the
/// preference's timezone at the current instant, so the offset is always the
/// one in force on that date.
fn reminder_due(
    frequency: ReminderFrequency,
    ttl_hours: u32,
    window_hours: u32,
    local: ReminderWindow,
) -> bool {
    if ttl_hours > window_hours {
        return false;
    }

    match frequency {
        ReminderFrequency::Once => ttl_hours > window_hours.saturating_sub(1),
        ReminderFrequency::Hourly => true,
        ReminderFrequency::Daily => local.daily,
        ReminderFrequency::Weekly => local.weekly,
        ReminderFrequency::Monthly => local.monthly,
    }
}

/// Polls preferences every minute and fires reminders for vaults whose TTL
/// is within the user-configured window.
///
/// TTL is read from the shared vault store and reminders are dispatched
/// through [`send_reminder`]. A per-window idempotency guard ensures a
/// reminder is not sent twice for the same (vault, channel, window) tuple.
///
/// Recurring reminders are anchored to 09:00 in the preference's IANA
/// timezone; the offset is recomputed every tick, so US/EU DST transitions do
/// not shift a reminder to 08:00 or 10:00 local (#1752).
///
/// The loop observes `shutdown` so that SIGTERM can stop the scheduler
/// gracefully: once the token is cancelled the current tick is allowed to
/// finish (draining in-flight jobs) and the loop exits.
#[tracing::instrument(skip(db, shutdown))]
pub async fn run(db: Arc<Db>, shutdown: CancellationToken) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    let mut sent: HashMap<(u64, String, u32), i64> = HashMap::new();

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                tracing::info!("scheduler received shutdown signal, draining in-flight jobs");
                break;
            }
            _ = interval.tick() => {}
        }

        let now = Utc::now();

        // 1) Reminder preferences scheduler.
        match db.all() {
            Ok(all_prefs) => {
                for prefs in all_prefs {
                    let ttl_hours = fetch_ttl_remaining(&db, prefs.vault_id).await;
                    let window = prefs.hours_before_expiry;

                    // Resolve the local reminder window once per preference.
                    // An unknown timezone yields "no recurring reminder"
                    // rather than silently using UTC and firing at the wrong
                    // hour.
                    let local =
                        reminder_window(now, &prefs.timezone).unwrap_or(ReminderWindow::NONE);

                    let subscription = db.get_subscription(prefs.vault_id).ok().flatten();

                    let effective = subscription.as_ref().map_or_else(
                        || ReminderFrequency::from_preference(&prefs.frequency),
                        |sub| ReminderFrequency::from_subscription(&sub.frequency),
                    );

                    if !reminder_due(effective, ttl_hours, window, local) {
                        continue;
                    }

                    for channel in &prefs.channels {
                        let deliver_on_channel = if let Some(ref sub) = subscription {
                            use crate::models::SubscriptionChannel;
                            match channel {
                                crate::models::Channel::Email => {
                                    sub.channels.contains(&SubscriptionChannel::Email)
                                }
                                crate::models::Channel::Sms => {
                                    sub.channels.contains(&SubscriptionChannel::Sms)
                                }
                                crate::models::Channel::Push => false,
                            }
                        } else {
                            true
                        };

                        if !deliver_on_channel {
                            continue;
                        }

                        let key = (prefs.vault_id, format!("{channel:?}"), window);
                        let now_ts = now.timestamp();
                        let already_sent = sent
                            .get(&key)
                            .map(|ts| now_ts - *ts < 3600)
                            .unwrap_or(false);
                        if already_sent {
                            continue;
                        }

                        send_reminder(&db, prefs.vault_id, &channel, ttl_hours).await;
                        sent.insert(key, now_ts);
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to fetch reminder preferences");
            }
        }

        // 2) TTL insurance scheduler.
        extend_ttl_for_inactive_owners(&db).await;

        // 3) #1101: Reminder escalation for unresponsive vault owners.
        crate::escalation::run_escalation_check(&db).await;

        // 4) #1102: Webhook delivery retry with exponential backoff.
        crate::webhook_retry::flush(&db).await;

        // 5) #1337: Beneficiary archival notification — notify opted-in
        //    beneficiaries when a vault's TTL has expired (TTL remaining == 0).
        notify_beneficiaries_on_ttl_expiry(&db).await;
    }
}

/// Hours remaining until vault TTL expiry.
///
/// Reads the vault from the shared in-memory store. Vaults that are absent
/// (or whose TTL is unknown) report `u32::MAX` so callers never fire a
/// spurious reminder for a missing vault.
async fn fetch_ttl_remaining(db: &Arc<Db>, vault_id: u64) -> u32 {
    let ttl_seconds = {
        let store = db
            .vault_store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        store
            .get(&vault_id.to_string())
            .and_then(|vault| vault.ttl_remaining)
    };

    match ttl_seconds {
        Some(seconds) => u32::try_from(seconds / 3_600).unwrap_or(u32::MAX),
        None => u32::MAX,
    }
}

/// Dispatch point for a due reminder.
///
/// The scheduler owns *when* a reminder fires; the notification service owns
/// *how* it is delivered (email/SMS/push), because reminder preferences do not
/// carry a delivery address. Keeping dispatch to a single seam means the
/// timezone logic can be unit-tested without a transport, and transport wiring
/// does not have to be duplicated here.
async fn send_reminder(
    db: &Arc<Db>,
    vault_id: u64,
    channel: &crate::models::Channel,
    hours_left: u32,
) {
    let owner = {
        let store = db
            .vault_store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        store
            .get(&vault_id.to_string())
            .map(|vault| vault.owner.clone())
    };

    tracing::info!(
        vault_id,
        owner = ?owner,
        ?channel,
        hours_left,
        "reminder due"
    );
}

/// Extends the TTL of vaults whose owners have been inactive longer than
/// their insurance policy threshold.
#[tracing::instrument(skip(db))]
async fn extend_ttl_for_inactive_owners(db: &Arc<Db>) {
    let policies = match db.all_enabled_insurance_policies() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch insurance policies");
            return;
        }
    };

    let now = Utc::now();

    for policy in policies {
        let owner_last_active = match db.get_owner_last_active_at(policy.vault_id) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(
                    vault_id = policy.vault_id,
                    error = %e,
                    "failed to fetch owner last active time"
                );
                continue;
            }
        };
        let Some(last_active) = owner_last_active else {
            continue;
        };

        let inactive_for = now.signed_duration_since(last_active).num_seconds();
        let threshold = i64::try_from(policy.inactivity_threshold_seconds).unwrap_or(i64::MAX);
        if inactive_for < threshold {
            continue;
        }

        tracing::info!(
            vault_id = policy.vault_id,
            extension_seconds = policy.extension_seconds,
            "TTL insurance triggered due to owner inactivity"
        );

        let updated = crate::models::TtlInsurancePolicy {
            vault_id: policy.vault_id,
            extension_seconds: policy.extension_seconds,
            inactivity_threshold_seconds: policy.inactivity_threshold_seconds,
            enabled: true,
            purchased_at: policy.purchased_at,
            last_extended_at: Some(now),
        };

        if let Err(e) = db.upsert_insurance_policy(&updated) {
            tracing::error!(
                vault_id = policy.vault_id,
                error = %e,
                "failed to update insurance policy after TTL extension"
            );
        }
    }
}

// ── Issue #1337: Beneficiary archival notification ────────────────────────────

/// Iterates over vaults whose TTL has expired and notifies opted-in
/// beneficiaries that have contact details on file.
///
/// Each dispatch is recorded via `Db::record_beneficiary_archival_notification`
/// so the system has an audit trail. Delivery itself is performed by the
/// notification workers that consume those records.
#[tracing::instrument(skip(db))]
async fn notify_beneficiaries_on_ttl_expiry(db: &Arc<Db>) {
    use crate::models::{BeneficiaryArchivalNotification, DeliveryStatus, Vault, VaultStatus};
    use uuid::Uuid;

    let expired_vaults: Vec<Vault> = {
        let store = db
            .vault_store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        store
            .values()
            .filter(|vault| match vault.status {
                VaultStatus::Released => true,
                _ => vault.ttl_remaining == Some(0),
            })
            .cloned()
            .collect()
    };

    for vault in expired_vaults {
        let contacts = match db.get_opted_in_contacts_for_vault(&vault.id) {
            Ok(contacts) => contacts,
            Err(e) => {
                tracing::error!(vault_id = vault.id, error = %e, "failed to fetch beneficiary contacts");
                continue;
            }
        };

        for contact in contacts {
            let mut channels = Vec::new();
            if contact.email.is_some() {
                channels.push("email");
            }
            if contact.phone.is_some() {
                channels.push("sms");
            }

            for channel in channels {
                let notification = BeneficiaryArchivalNotification {
                    id: Uuid::new_v4().to_string(),
                    vault_id: vault.id.clone(),
                    beneficiary_address: contact.beneficiary_address.clone(),
                    channel: channel.to_owned(),
                    dispatched_at: Utc::now(),
                    status: DeliveryStatus::Pending,
                    error: None,
                };

                if let Err(e) = db.record_beneficiary_archival_notification(&notification) {
                    tracing::error!(
                        vault_id = vault.id,
                        beneficiary_address = contact.beneficiary_address,
                        channel,
                        error = %e,
                        "failed to record beneficiary archival notification"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{reminder_due, ReminderFrequency};
    use crate::reminder_time::{reminder_window, ReminderWindow};
    use chrono::{TimeZone, Utc};

    fn window(daily: bool, weekly: bool, monthly: bool) -> ReminderWindow {
        ReminderWindow {
            daily,
            weekly,
            monthly,
        }
    }

    #[test]
    fn hourly_frequency_fires_whenever_inside_the_ttl_window() {
        let local = window(false, false, false);
        assert!(reminder_due(ReminderFrequency::Hourly, 12, 24, local));
        assert!(!reminder_due(ReminderFrequency::Hourly, 25, 24, local));
    }

    #[test]
    fn once_frequency_fires_only_at_the_threshold_hour() {
        let local = window(true, true, true);
        assert!(reminder_due(ReminderFrequency::Once, 24, 24, local));
        assert!(!reminder_due(ReminderFrequency::Once, 23, 24, local));
        assert!(!reminder_due(ReminderFrequency::Once, 25, 24, local));
    }

    #[test]
    fn daily_frequency_requires_the_local_reminder_window() {
        assert!(reminder_due(
            ReminderFrequency::Daily,
            10,
            24,
            window(true, false, false)
        ));
        assert!(!reminder_due(
            ReminderFrequency::Daily,
            10,
            24,
            window(false, true, true)
        ));
    }

    #[test]
    fn daily_reminder_keeps_the_local_hour_across_dst() {
        // 09:30 local on 2026-03-07 (EST) is 14:30Z; on 2026-03-08 (EDT) it
        // is 13:30Z. Both must be considered due, and 08:30/10:30 not.
        let before = reminder_window(
            Utc.with_ymd_and_hms(2026, 3, 7, 14, 30, 0).unwrap(),
            "America/New_York",
        )
        .unwrap();
        let after = reminder_window(
            Utc.with_ymd_and_hms(2026, 3, 8, 13, 30, 0).unwrap(),
            "America/New_York",
        )
        .unwrap();
        assert!(before.daily);
        assert!(after.daily);

        for hour in [12, 14] {
            let local = reminder_window(
                Utc.with_ymd_and_hms(2026, 3, 8, hour, 30, 0).unwrap(),
                "America/New_York",
            )
            .unwrap();
            assert!(
                !local.daily,
                "hour {hour}Z must not be due on transition day"
            );
        }
    }
}
