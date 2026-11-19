//! Shared notification provider abstraction (issue #1482).
//!
//! Email/SMS delivery used to be duplicated as ad-hoc stubs across
//! `scheduler.rs`, `escalation.rs`, `notifications.rs` and `webhook_retry.rs`.
//! This module centralises the delivery surface behind two traits so call
//! sites depend on the abstraction instead of re-implementing stubs.

use std::sync::Arc;

/// Delivers transactional email messages.
pub trait EmailProvider: Send + Sync {
    /// Sends an email to `to` with the given `subject` and `body`.
    fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), String>;
}

/// Delivers transactional SMS messages.
pub trait SmsProvider: Send + Sync {
    /// Sends an SMS to `to` with the given `body`.
    fn send_sms(&self, to: &str, body: &str) -> Result<(), String>;
}

/// No-op email provider used for local development and tests.
///
/// Logs the message at debug level and reports success so callers can run
/// without a configured mail transport.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopEmailProvider;

impl EmailProvider for NoopEmailProvider {
    fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), String> {
        tracing::debug!(to, subject, body, "noop email provider: message dropped");
        Ok(())
    }
}

/// No-op SMS provider used for local development and tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSmsProvider;

impl SmsProvider for NoopSmsProvider {
    fn send_sms(&self, to: &str, body: &str) -> Result<(), String> {
        tracing::debug!(to, body, "noop sms provider: message dropped");
        Ok(())
    }
}

/// Bundles the configured notification providers so they can be injected
/// through application state.
#[derive(Clone)]
pub struct NotificationProviders {
    pub email: Arc<dyn EmailProvider>,
    pub sms: Arc<dyn SmsProvider>,
}

impl NotificationProviders {
    /// Builds the default provider set: no-op implementations suitable for
    /// local development. Production wiring can swap in real transports.
    pub fn noop() -> Self {
        Self {
            email: Arc::new(NoopEmailProvider),
            sms: Arc::new(NoopSmsProvider),
        }
    }
}

impl Default for NotificationProviders {
    fn default() -> Self {
        Self::noop()
    }
}
