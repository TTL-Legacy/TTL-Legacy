use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio_util::sync::CancellationToken;

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
        SmsProviderConfig::AwsSns { region, from } => {
            Arc::new(AwsSnsSmsProvider { region, from })
        }
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

/// Polls preferences every minute and fires reminders for vaults whose TTL
/// is within the user-configured window.
///
/// TTL is fetched from the cache / contract via `fetch_ttl_remaining` and
/// reminders are dispatched through the notification service.  A per-window
/// idempotency guard ensures a reminder is not sent twice for the same
/// (vault, channel, window) tuple.
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

        // 1) Existing reminder preferences scheduler.
        match db.all() {
            Ok(all_prefs) => {
                for prefs in all_prefs {
                    let ttl_hours = fetch_ttl_remaining(&db, prefs.vault_id).await;
                    let window = prefs.hours_before_expiry;

                    let subscription = db.get_subscription(prefs.vault_id).ok().flatten();

                    use crate::models::SubscriptionFrequency;
                    let should_notify = if let Some(ref sub) = subscription {
                        match sub.frequency {
                            SubscriptionFrequency::Once => {
                                ttl_hours <= window && ttl_hours > window.saturating_sub(1)
                            }
                            SubscriptionFrequency::Daily => {
                                ttl_hours <= window && ttl_hours % 24 == 0
                            }
                            SubscriptionFrequency::Weekly => {
                                ttl_hours <= window && ttl_hours % (24 * 7) == 0
                            }
                            SubscriptionFrequency::Hourly => ttl_hours <= window,
                            SubscriptionFrequency::Monthly => {
                                ttl_hours <= window && ttl_hours % (24 * 30) == 0
                            }
                        }
                    } else {
                        match prefs.frequency {
                            Frequency::Once => {
                                ttl_hours <= window && ttl_hours > window.saturating_sub(1)
                            }
                            Frequency::Daily => ttl_hours <= window && ttl_hours % 24 == 0,
                            Frequency::Weekly => ttl_hours <= window && ttl_hours % (24 * 7) == 0,
                            Frequency::Hourly => ttl_hours <= window,
                            Frequency::Monthly => ttl_hours <= window && ttl_hours % (24 * 30) == 0,
                        }
                    };

                    if should_notify {
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

                            if deliver_on_channel {
                                let key = (prefs.vault_id, format!("{:?}", channel), window);
                                let now = Utc::now().timestamp();
                                let already_sent = sent
                                    .get(&key)
                                    .map(|ts| now - *ts < 3600)
                                    .unwrap_or(false);
                                if already_sent {
                                    continue;
                                }
                                send_reminder(&db, prefs.vault_id, channel, ttl_hours).await;
                                sent.insert(key, now);
                            }
                        }
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
        notify_beneficiaries_

/* … truncated 8861 chars — edit only what you need near the top … */
