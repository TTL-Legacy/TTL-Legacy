/// #1596: Vault owner notification when the on-chain storage TTL nears archival.
///
/// A background task polls the storage TTL of every tracked vault and, once it
/// drops to or below a configurable threshold, sends the owner a single
/// warning through the `NotificationService`. The warning re-arms once the TTL
/// is extended back above the threshold, so each approach to archival produces
/// exactly one warning.
///
/// Configuration (environment):
///   `TTL_ARCHIVAL_WARNING_THRESHOLD_SECS` — warn at or below this TTL (default 7 days)
///   `TTL_WATCH_POLL_INTERVAL_SECS`        — polling interval (default 5 minutes)
use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use crate::{
    db::Db,
    models::{Vault, VaultStatus},
    notifications::NotificationService,
};

pub const DEFAULT_WARNING_THRESHOLD_SECS: u64 = 7 * 24 * 3_600;
pub const DEFAULT_POLL_INTERVAL_SECS: u64 = 300;

const THRESHOLD_ENV: &str = "TTL_ARCHIVAL_WARNING_THRESHOLD_SECS";
const POLL_INTERVAL_ENV: &str = "TTL_WATCH_POLL_INTERVAL_SECS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TtlWatchConfig {
    /// Warn when the remaining storage TTL (seconds) is at or below this value.
    pub warning_threshold_secs: u64,
    pub poll_interval_secs: u64,
}

impl Default for TtlWatchConfig {
    fn default() -> Self {
        Self {
            warning_threshold_secs: DEFAULT_WARNING_THRESHOLD_SECS,
            poll_interval_secs: DEFAULT_POLL_INTERVAL_SECS,
        }
    }
}

impl TtlWatchConfig {
    pub fn from_env() -> Self {
        Self::from_values(
            std::env::var(THRESHOLD_ENV).ok(),
            std::env::var(POLL_INTERVAL_ENV).ok(),
        )
    }

    /// Build a config from raw values; missing, unparsable or zero values fall
    /// back to the defaults.
    pub fn from_values(threshold: Option<String>, poll_interval: Option<String>) -> Self {
        Self {
            warning_threshold_secs: parse_positive(
                THRESHOLD_ENV,
                threshold,
                DEFAULT_WARNING_THRESHOLD_SECS,
            ),
            poll_interval_secs: parse_positive(
                POLL_INTERVAL_ENV,
                poll_interval,
                DEFAULT_POLL_INTERVAL_SECS,
            ),
        }
    }
}

fn parse_positive(name: &str, raw: Option<String>, default: u64) -> u64 {
    let Some(raw) = raw else { return default };
    match raw.trim().parse::<u64>() {
        Ok(v) if v > 0 => v,
        _ => {
            tracing::warn!(name, value = %raw, default, "invalid TTL watch setting, using default");
            default
        }
    }
}

/// Vaults whose storage entries must be kept alive: finished vaults
/// (`Expired` / `Released`) no longer need archival warnings.
fn is_tracked(vault: &Vault) -> bool {
    matches!(vault.status, VaultStatus::Active | VaultStatus::Paused)
}

pub struct TtlArchivalWatcher {
    config: TtlWatchConfig,
    /// Vault IDs already warned during their current approach to archival.
    warned: Mutex<HashSet<String>>,
}

impl TtlArchivalWatcher {
    pub fn new(config: TtlWatchConfig) -> Self {
        Self {
            config,
            warned: Mutex::new(HashSet::new()),
        }
    }

    /// Poll the storage TTL of every tracked vault once and warn owners whose
    /// vault is near archival. Returns the number of vaults newly warned.
    #[tracing::instrument(skip_all)]
    pub fn poll_once(&self, db: &Db, notifications: &NotificationService) -> usize {
        // Snapshot so the vault store lock is not held while notifying.
        let tracked: Vec<(String, String, Option<u64>)> = db
            .vault_store
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .filter(|v| is_tracked(v))
            .map(|v| (v.id.clone(), v.owner.clone(), v.ttl_remaining))
            .collect();

        {
            // Drop state for vaults that are no longer tracked.
            let mut warned = self.warned.lock().unwrap_or_else(PoisonError::into_inner);
            warned.retain(|id| tracked.iter().any(|(tracked_id, ..)| tracked_id == id));
        }

        tracked
            .iter()
            .filter(|(id, owner, ttl)| self.check_vault(id, owner, *ttl, notifications))
            .count()
    }

    fn check_vault(
        &self,
        vault_id: &str,
        owner: &str,
        ttl_secs: Option<u64>,
        notifications: &NotificationService,
    ) -> bool {
        // Unknown TTL: nothing to compare against yet.
        let Some(ttl_secs) = ttl_secs else {
            return false;
        };

        let mut warned = self.warned.lock().unwrap_or_else(PoisonError::into_inner);

        if ttl_secs > self.config.warning_threshold_secs {
            // TTL was extended (or never low): re-arm for the next approach.
            warned.remove(vault_id);
            return false;
        }
        if warned.contains(vault_id) {
            return false;
        }

        if notifications.enqueue_ttl_archival_warning(vault_id, owner) {
            warned.insert(vault_id.to_string());
            tracing::info!(
                vault_id,
                owner,
                ttl_secs,
                threshold_secs = self.config.warning_threshold_secs,
                "storage TTL near archival, owner warned"
            );
            true
        } else {
            false
        }
    }
}

/// Spawn the background polling loop.
pub fn spawn(
    db: Arc<Db>,
    notifications: Arc<NotificationService>,
    config: TtlWatchConfig,
) -> tokio::task::JoinHandle<()> {
    tracing::info!(
        threshold_secs = config.warning_threshold_secs,
        poll_interval_secs = config.poll_interval_secs,
        "starting TTL archival watcher"
    );
    let watcher = TtlArchivalWatcher::new(config);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(config.poll_interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            watcher.poll_once(&db, &notifications);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DeliveryStatus, NotificationType, UpdatePreferencesRequest};
    use crate::notifications::{
        create_delivery_store, create_prefs_store, create_schedule_store, create_token_store,
        FcmClient,
    };
    use chrono::Utc;

    const THRESHOLD: u64 = 3_600;

    fn setup() -> (Db, NotificationService, TtlArchivalWatcher) {
        let db = Db::open(":memory:").unwrap();
        let fcm = Arc::new(FcmClient::new("test-key".into(), "test-project".into()));
        let svc = NotificationService::new(
            fcm,
            create_token_store(),
            create_prefs_store(),
            create_schedule_store(),
            create_delivery_store(),
        );
        let watcher = TtlArchivalWatcher::new(TtlWatchConfig {
            warning_threshold_secs: THRESHOLD,
            poll_interval_secs: 60,
        });
        (db, svc, watcher)
    }

    fn vault(id: &str, status: VaultStatus, ttl: Option<u64>) -> Vault {
        Vault {
            id: id.into(),
            owner: format!("owner-{id}"),
            beneficiary: "ben".into(),
            balance: 1_000,
            check_in_interval: 86_400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status,
            ttl_remaining: ttl,
        }
    }

    fn set_ttl(db: &Db, id: &str, ttl: u64) {
        db.vault_store
            .lock()
            .unwrap()
            .get_mut(id)
            .unwrap()
            .ttl_remaining = Some(ttl);
    }

    fn warnings_for(svc: &NotificationService, vault_id: &str) -> usize {
        svc.schedule
            .lock()
            .unwrap()
            .iter()
            .filter(|n| {
                n.vault_id == vault_id && n.notification_type == NotificationType::ExpiryWarning
            })
            .count()
    }

    /// Simulate the notification flusher delivering everything that is due.
    fn mark_all_sent(svc: &NotificationService) {
        for n in svc.schedule.lock().unwrap().iter_mut() {
            n.status = DeliveryStatus::Sent;
            n.sent_at = Some(Utc::now());
        }
    }

    // ── Config ───────────────────────────────────────────────────────────────

    #[test]
    fn config_defaults_when_unset() {
        assert_eq!(
            TtlWatchConfig::from_values(None, None),
            TtlWatchConfig::default()
        );
    }

    #[test]
    fn config_parses_custom_values() {
        let cfg = TtlWatchConfig::from_values(Some("86400".into()), Some(" 30 ".into()));
        assert_eq!(cfg.warning_threshold_secs, 86_400);
        assert_eq!(cfg.poll_interval_secs, 30);
    }

    #[test]
    fn config_falls_back_on_invalid_or_zero() {
        let cfg = TtlWatchConfig::from_values(Some("abc".into()), Some("0".into()));
        assert_eq!(cfg, TtlWatchConfig::default());
    }

    // ── Polling ──────────────────────────────────────────────────────────────

    #[test]
    fn warns_owner_when_ttl_at_or_below_threshold() {
        let (db, svc, watcher) = setup();
        db.insert_vault(vault("at", VaultStatus::Active, Some(THRESHOLD)));
        db.insert_vault(vault("below", VaultStatus::Active, Some(10)));

        assert_eq!(watcher.poll_once(&db, &svc), 2);

        let pending = svc.get_pending_notifications();
        assert_eq!(pending.len(), 2);
        let n = pending.iter().find(|n| n.vault_id == "at").unwrap();
        assert_eq!(n.owner, "owner-at");
        assert_eq!(n.notification_type, NotificationType::ExpiryWarning);
    }

    #[test]
    fn no_warning_above_threshold() {
        let (db, svc, watcher) = setup();
        db.insert_vault(vault("v", VaultStatus::Active, Some(THRESHOLD + 1)));
        assert_eq!(watcher.poll_once(&db, &svc), 0);
        assert_eq!(warnings_for(&svc, "v"), 0);
    }

    #[test]
    fn warns_only_once_per_approach() {
        let (db, svc, watcher) = setup();
        db.insert_vault(vault("v", VaultStatus::Active, Some(100)));

        assert_eq!(watcher.poll_once(&db, &svc), 1);
        mark_all_sent(&svc);
        set_ttl(&db, "v", 50);
        assert_eq!(watcher.poll_once(&db, &svc), 0);
        assert_eq!(warnings_for(&svc, "v"), 1);
    }

    #[test]
    fn rearms_after_ttl_is_extended() {
        let (db, svc, watcher) = setup();
        db.insert_vault(vault("v", VaultStatus::Active, Some(100)));
        assert_eq!(watcher.poll_once(&db, &svc), 1);
        mark_all_sent(&svc);

        set_ttl(&db, "v", THRESHOLD * 10); // owner extended TTL
        assert_eq!(watcher.poll_once(&db, &svc), 0);

        set_ttl(&db, "v", 100); // approaching archival again
        assert_eq!(watcher.poll_once(&db, &svc), 1);
        assert_eq!(warnings_for(&svc, "v"), 2);
    }

    #[test]
    fn ignores_untracked_statuses_and_unknown_ttl() {
        let (db, svc, watcher) = setup();
        db.insert_vault(vault("released", VaultStatus::Released, Some(10)));
        db.insert_vault(vault("expired", VaultStatus::Expired, Some(0)));
        db.insert_vault(vault("unknown", VaultStatus::Active, None));
        db.insert_vault(vault("paused", VaultStatus::Paused, Some(10)));

        assert_eq!(watcher.poll_once(&db, &svc), 1);
        assert_eq!(warnings_for(&svc, "paused"), 1);
    }

    #[test]
    fn respects_owner_opt_out() {
        let (db, svc, watcher) = setup();
        db.insert_vault(vault("v", VaultStatus::Active, Some(10)));
        svc.update_preferences(UpdatePreferencesRequest {
            owner: "owner-v".into(),
            expiry_warning_enabled: Some(false),
            check_in_reminder_enabled: None,
            vault_released_enabled: None,
            warning_hours_before: None,
            locale: None,
        });

        assert_eq!(watcher.poll_once(&db, &svc), 0);
        assert_eq!(warnings_for(&svc, "v"), 0);
    }

    #[test]
    fn skips_unsubscribed_owner() {
        let (db, svc, watcher) = setup();
        db.insert_vault(vault("v", VaultStatus::Active, Some(10)));
        let token = svc.generate_unsubscribe_token("owner-v");
        svc.process_unsubscribe(&token).unwrap();

        assert_eq!(watcher.poll_once(&db, &svc), 0);
    }

    #[test]
    fn does_not_stack_on_already_due_expiry_warning() {
        let (db, svc, watcher) = setup();
        db.insert_vault(vault("v", VaultStatus::Active, Some(10)));
        assert!(svc.enqueue_ttl_archival_warning("v", "owner-v"));

        // Counted as warned, but no second notification is queued...
        assert_eq!(watcher.poll_once(&db, &svc), 1);
        assert_eq!(warnings_for(&svc, "v"), 1);
        // ...and none after the due one is delivered.
        mark_all_sent(&svc);
        assert_eq!(watcher.poll_once(&db, &svc), 0);
        assert_eq!(warnings_for(&svc, "v"), 1);
    }
}
