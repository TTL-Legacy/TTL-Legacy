use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::{db::Db, models::Frequency};


/// Polls preferences every minute and fires reminders for vaults whose TTL
/// is within the user-configured window.
///
/// In production, replace `fetch_ttl_remaining` with a real Stellar RPC call
/// and `send_reminder` with actual email/SMS/push dispatch.
pub async fn run(db: Arc<Db>) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;

        // 1) Existing reminder preferences scheduler.
        if let Ok(all_prefs) = db.all() {
            for prefs in all_prefs {
                let ttl_hours = fetch_ttl_remaining(prefs.vault_id).await;
                let window = prefs.hours_before_expiry;

                let should_notify = match prefs.frequency {
                    Frequency::Once => ttl_hours <= window && ttl_hours > window.saturating_sub(1),
                    Frequency::Daily => ttl_hours <= window && ttl_hours % 24 == 0,
                    Frequency::Hourly => ttl_hours <= window,
                };

                if should_notify {
                    for channel in &prefs.channels {
                        send_reminder(prefs.vault_id, channel, ttl_hours).await;
                    }
                }
            }
        }

        // 2) TTL insurance scheduler.
        extend_ttl_for_inactive_owners(&db).await;
    }
}

async fn extend_ttl_for_inactive_owners(db: &Arc<Db>) {
    let Ok(policies) = db.all_enabled_insurance_policies() else { return };

    let now = Utc::now();

    for policy in policies {
        if !policy.enabled {
            continue;
        }
        // Owner inactivity
        let Ok(owner_last_active) = db.get_owner_last_active_at(policy.vault_id) else {
            continue;
        };
        let Some(last_active) = owner_last_active else {
            continue;
        };

        let inactive_for = now.signed_duration_since(last_active).num_seconds();
        if inactive_for < policy.inactivity_threshold_seconds as i64 {
            continue;
        }

        // Extend TTL (stubbed as we don't have a real on-chain state updater here).
        tracing::info!(
            vault_id = policy.vault_id,
            extension_seconds = policy.extension_seconds,
            "TTL extended by insurance due to inactivity"
        );

        let _ = db.upsert_insurance_policy(&crate::models::TtlInsurancePolicy {
            vault_id: policy.vault_id,
            extension_seconds: policy.extension_seconds,
            inactivity_threshold_seconds: policy.inactivity_threshold_seconds,
            enabled: true,
            purchased_at: policy.purchased_at,
            last_extended_at: Some(now),
        });
    }
}


/// Stub: returns hours remaining until vault TTL expiry.
/// Replace with a Stellar RPC call to `get_ttl_remaining`.
async fn fetch_ttl_remaining(_vault_id: u64) -> u32 {
    u32::MAX
}

/// Stub: dispatches a reminder via the given channel.
async fn send_reminder(vault_id: u64, channel: &crate::models::Channel, hours_left: u32) {
    tracing::info!(
        vault_id,
        ?channel,
        hours_left,
        "sending reminder"
    );
}
