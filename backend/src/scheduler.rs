use std::sync::Arc;
use std::time::Duration;

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
