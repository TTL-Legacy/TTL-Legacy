use crate::db::*;
use crate::models::*;
use tracing::instrument;

// ── Task 4: Notification Preferences ─────────────────────────────────────────

/// POST /vaults/{id}/notification-preferences
///
/// Instrumented with an OpenTelemetry span (Issue #1145).
#[instrument(skip(store, notif_store), fields(vault_id = %vault_id))]
pub fn set_notification_preferences_handler(
    store: &VaultStore,
    notif_store: &NotificationStore,
    vault_id: &str,
    request: NotificationPreferencesRequest,
) -> Result<VaultNotificationPreferences, String> {
    if request.channels.is_empty() {
        return Err("At least one notification channel is required".to_string());
    }

    // Verify vault exists
    store
        .lock()
        .unwrap()
        .get(vault_id)
        .ok_or_else(|| "Vault not found".to_string())?;

    // Map HTTP channels into legacy boolean flags.
    let preferred = request.channels.first().cloned();
    let fallback = request.channels.get(1).cloned();
    let prefs = NotificationPreferences {
        owner: vault_id.to_string(),
        expiry_warning_enabled: request
            .channels
            .iter()
            .any(|c| matches!(c, NotificationChannel::Email | NotificationChannel::Push)),
        check_in_reminder_enabled: request
            .channels
            .iter()
            .any(|c| matches!(c, NotificationChannel::Sms | NotificationChannel::Push)),
        vault_released_enabled: request
            .channels
            .iter()
            .any(|c| matches!(c, NotificationChannel::Push)),
        warning_hours_before: 24,
        locale: None,
        preferred_channel: preferred,
        fallback_channel: fallback,
        unsubscribed: false,
    };

    set_notification_preferences(notif_store, prefs.clone());
    Ok(prefs)
}

/// GET /vaults/{id}/notification-preferences
pub fn get_notification_preferences_handler(
    notif_store: &NotificationStore,
    vault_id: &str,
) -> Option<VaultNotificationPreferences> {
    get_notification_preferences(notif_store, vault_id)
}
