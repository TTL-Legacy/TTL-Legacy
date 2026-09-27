/// #1101: Reminder Escalation for Unresponsive Vault Owners
///
/// Escalation tiers:
///   T1 — 7 days (168 h) before expiry: email
///   T2 — 3 days  (72 h) before expiry: email + SMS
///   T3 — 24 h            before expiry: all channels + emergency contact
///
/// Rules:
///   - A tier is only dispatched once within a 24-hour window (deduplication).
///   - The scheduler promotes to the next tier when TTL crosses the threshold.
///   - Every dispatch is written to the escalation_events audit table.
use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::{
    audit::{AuditLog, AuditOutcome},
    db::Db,
    models::{EscalationEvent, EscalationState, EscalationTier, TimelineEvent, TimelineEventKind},
    notifications::{Notification, NotificationChannel, NotificationProvider},
};

/// How long (seconds) to wait before re-dispatching the same tier.
/// Prevents duplicate alerts within a 24-hour window.
const TIER_DEDUP_WINDOW_SECS: i64 = 86_400; // 24 h

/// Evaluate all vaults with reminder preferences and dispatch escalation
/// notifications as necessary. Called from the scheduler loop.
#[tracing::instrument(skip(db))]
pub async fn run_escalation_check(db: &Arc<Db>) {
    let prefs = match db.all() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "escalation: failed to fetch reminder preferences");
            return;
        }
    };

    for pref in prefs {
        let ttl_hours = fetch_ttl_hours(db, pref.vault_id).await;
        evaluate_vault(db, pref.vault_id, ttl_hours).await;
    }
}

/// Determine whether a new escalation tier should be dispatched for a single
/// vault, and if so, dispatch it.
pub async fn evaluate_vault(db: &Arc<Db>, vault_id: u64, ttl_hours: u32) {
    // Determine which tier the current TTL falls into.
    let required_tier = tier_for_ttl(ttl_hours);
    let Some(required_tier) = required_tier else {
        // TTL is still far enough away — no escalation needed.
        return;
    };

    // Load existing state.
    let state = match db.get_escalation_state(vault_id) {
        Ok(s) => s.unwrap_or(EscalationState {
            vault_id,
            last_escalation_tier: None,
            escalated_at: None,
        }),
        Err(e) => {
            tracing::error!(vault_id, error = %e, "escalation: failed to load state");
            return;
        }
    };

    // Check if this tier (or higher) was already dispatched.
    if let Some(last_tier) = state.last_escalation_tier {
        if last_tier >= required_tier {
            // Check deduplication window — don't re-send within 24 h.
            if let Some(escalated_at) = state.escalated_at {
                let age = Utc::now().signed_duration_since(escalated_at).num_seconds();
                if age < TIER_DEDUP_WINDOW_SECS {
                    tracing::debug!(
                        vault_id,
                        ?last_tier,
                        age_secs = age,
                        "escalation: skipping, within dedup window"
                    );
                    return;
                }
            } else {
                // Last tier was dispatched, no timestamp — skip to avoid double-send.
                return;
            }
        }
    }

    // Dispatch the escalation.
    dispatch_escalation(db, vault_id, required_tier).await;
}

/// Return the highest escalation tier triggered by the remaining TTL (hours).
/// Returns None if no escalation is warranted yet.
pub fn tier_for_ttl(ttl_hours: u32) -> Option<EscalationTier> {
    if ttl_hours <= EscalationTier::T3.hours_before_expiry() {
        Some(EscalationTier::T3)
    } else if ttl_hours <= EscalationTier::T2.hours_before_expiry() {
        Some(EscalationTier::T2)
    } else if ttl_hours <= EscalationTier::T1.hours_before_expiry() {
        Some(EscalationTier::T1)
    } else {
        None
    }
}

/// Channels used per tier.
fn channels_for_tier(tier: EscalationTier) -> Vec<&'static str> {
    match tier {
        EscalationTier::T1 => vec!["email"],
        EscalationTier::T2 => vec!["email", "sms"],
        EscalationTier::T3 => vec!["email", "sms", "emergency_contact"],
    }
}

/// Map a tier's channel names to the shared notification channels.
fn notification_channels_for_tier(tier: EscalationTier) -> Vec<NotificationChannel> {
    channels_for_tier(tier)
        .into_iter()
        .map(|c| match c {
            "sms" => NotificationChannel::Sms,
            "emergency_contact" => NotificationChannel::EmergencyContact,
            _ => NotificationChannel::Email,
        })
        .collect()
}

/// Actually dispatch the escalation: send via the shared notification provider,
/// log the event, update state, and record a timeline entry.
async fn dispatch_escalation(db: &Arc<Db>, vault_id: u64, tier: EscalationTier) {
    let channels: Vec<String> = channels_for_tier(tier)
        .into_iter()
        .map(String::from)
        .collect();
    let now = Utc::now();
    let event_id = Uuid::new_v4().to_string();

    tracing::info!(vault_id, ?tier, ?channels, "escalation: dispatching tier");

    // Deliver through the shared notification provider.
    let delivered = send_escalation_notifications(db, vault_id, tier, &channels).await;

    // Record the attempt in the audit log.
    let outcome = if delivered {
        AuditOutcome::Success
    } else {
        AuditOutcome::Failure
    };
    let audit = AuditLog::new(
        "escalation.dispatch",
        format!("vault:{vault_id}"),
        outcome,
    )
    .with_metadata(serde_json::json!({
        "tier": format!("{:?}", tier).to_lowercase(),
        "channels": channels,
        "delivered": delivered,
    }));
    if let Err(e) = db.insert_audit_log(&audit) {
        tracing::error!(vault_id, error = %e, "escalation: failed to record audit log");
    }

    // Persist the escalation event for the audit trail.
    let event = EscalationEvent {
        id: event_id.clone(),
        vault_id,
        tier,
        dispatched_at: now,
        channels: channels.clone(),
    };
    if let Err(e) = db.insert_escalation_event(&event) {
        tracing::error!(vault_id, error = %e, "escalation: failed to insert event");
    }

    // Update escalation state.
    let new_state = EscalationState {
        vault_id,
        last_escalation_tier: Some(tier),
        escalated_at: Some(now),
    };
    if let Err(e) = db.upsert_escalation_state(&new_state) {
        tracing::error!(vault_id, error = %e, "escalation: failed to upsert state");
    }

    // Record in vault timeline.
    let timeline_event = TimelineEvent {
        id: Uuid::new_v4().to_string(),
        vault_id: vault_id.to_string(),
        kind: TimelineEventKind::EscalationSent,
        timestamp: now,
        description: format!("Escalation {:?} sent via: {}", tier, channels.join(", ")),
        amount: None,
        metadata: serde_json::json!({
            "tier": format!("{:?}", tier).to_lowercase(),
            "channels": channels,
        }),
    };
    if let Err(e) = db.insert_timeline_event(&timeline_event) {
        tracing::error!(vault_id, error = %e, "escalation: failed to insert timeline event");
    }
}

/// Dispatch notifications for the given tier through the shared notification
/// provider. Returns true when every channel was delivered successfully.
async fn send_escalation_notifications(
    db: &Arc<Db>,
    vault_id: u64,
    tier: EscalationTier,
    channels: &[String],
) -> bool {
    let provider = NotificationProvider::from_db(db);
    let mut all_delivered = true;

    for channel in notification_channels_for_tier(tier) {
        let notification = Notification::new(
            format!("Vault {vault_id} escalation {tier:?}"),
            format!(
                "Vault {vault_id} requires attention: escalation tier {tier:?} triggered."
            ),
            channel,
        );
        match provider.send(&notification).await {
            Ok(_) => {
                tracing::info!(vault_id, ?tier, ?channel, "escalation: notification delivered");
            }
            Err(e) => {
                all_delivered = false;
                tracing::error!(
                    vault_id,
                    ?tier,
                    ?channel,
                    error = %e,
                    "escalation: notification delivery failed"
                );
            }
        }
    }

    let _ = channels;
    all_delivered
}

/// Return hours remaining until TTL expiry for a vault, derived from the real
/// vault TTL data stored in the database.
async fn fetch_ttl_hours(db: &Arc<Db>, vault_id: u64) -> u32 {
    match db.get_vault_ttl(vault_id) {
        Ok(Some(ttl)) => {
            let remaining = ttl.expires_at.signed_duration_since(Utc::now()).num_seconds();
            if remaining <= 0 {
                0
            } else {
                (remaining / 3_600) as u32
            }
        }
        Ok(None) => {
            tracing::debug!(vault_id, "escalation: no TTL data for vault");
            u32::MAX
        }
        Err(e) => {
            tracing::error!(vault_id, error = %e, "escalation: failed to fetch vault TTL");
            u32::MAX
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_for_ttl_no_escalation() {
        // 200 h remaining — beyond T1 threshold (168 h)
        assert_eq!(tier_for_ttl(200), None);
    }

    #[test]
    fn test_tier_for_ttl_t1() {
        // Between T1 (168 h) and T2 (72 h) thresholds
        assert_eq!(tier_for_ttl(168), Some(EscalationTier::T1));
        assert_eq!(tier_for_ttl(100), Some(EscalationTier::T1));
        assert_eq!(tier_for_ttl(73), Some(EscalationTier::T1));
    }

    #[test]
    fn test_tier_for_ttl_t2() {
        // Between T2 (72 h) and T3 (24 h) thresholds
        assert_eq!(tier_for_ttl(72), Some(EscalationTier::T2));
        assert_eq!(tier_for_ttl(48), Some(EscalationTier::T2));
        assert_eq!(tier_for_ttl(25), Some(EscalationTier::T2));
    }

    #[test]
    fn test_tier_for_ttl_t3() {
        // Within T3 threshold (24 h)
        assert_eq!(tier_for_ttl(24), Some(EscalationTier::T3));
        assert_eq!(tier_for_ttl(1), Some(EscalationTier::T3));
        assert_eq!(tier_for_ttl(0), Some(EscalationTier::T3));
    }

    #[test]
    fn test_channels_for_tier() {
        assert_eq!(channels_for_tier(EscalationTier::T1), vec!["email"]);
        assert_eq!(channels_for_tier(EscalationTier::T2), vec!["email", "sms"]);
        assert_eq!(
            channels_for_tier(EscalationTier::T3),
            vec!["email", "sms", "emergency_contact"]
        );
    }

    #[test]
    fn test_notification_channels_for_tier() {
        assert_eq!(
            notification_channels_for_tier(EscalationTier::T1),
            vec![NotificationChannel::Email]
        );
        assert_eq!(
            notification_channels_for_tier(EscalationTier::T2),
            vec![NotificationChannel::Email, NotificationChannel::Sms]
        );
        assert_eq!(
            notification_channels_for_tier(EscalationTier::T3),
            vec![
                NotificationChannel::Email,
                NotificationChannel::Sms,
                NotificationChannel::EmergencyContact
            ]
        );
    }

    #[test]
    fn test_ttl_hours_from_expiry() {
        // 48 h in the future should round down to 48 hours remaining.
        let expires_at = Utc::now() + chrono::Duration::hours(48);
        assert_eq!(ttl_hours_from_expiry(expires_at), 48);
    }

    #[test]
    fn test_ttl_hours_from_expiry_past() {
        // Already expired TTLs clamp to zero.
        let expires_at = Utc::now() - chrono::Duration::hours(5);
        assert_eq!(ttl_hours_from_expiry(expires_at), 0);
    }

    #[tokio::test]
    async fn test_escalation_stops_after_check_in() {
        let db = Arc::new(Db::open(":memory:").unwrap());
        db.migrate().unwrap();
        // T1 is dispatched first.
        evaluate_vault(&db, 123, 100).await;
        let initial_events = db.get_escalation_events(123).unwrap();
        assert_eq!(initial_events.len(), 1);
        assert_eq!(initial_events[0].tier, EscalationTier::T1);

        // Simulate a check-in by clearing escalation state.
        db.clear_escalation_state(123).unwrap();
        let cleared_state = db.get_escalation_state(123).unwrap();
        assert!(cleared_state.is_none(), "escalation state should be cleared after check-in");

        // TTL is still within T1 range, but since state is cleared,
        // a new escalation should be dispatched on re-evaluation.
        evaluate_vault(&db, 123, 100).await;
        let events_after_checkin = db.get_escalation_events(123).unwrap();
        assert_eq!(
            events_after_checkin.len(),
            2,
            "new escalation should be dispatched after check-in clears state"
        );
    }

    #[tokio::test]
    async fn test_t1_threshold_exact_boundary() {
        let db = Arc::new(Db::open(":memory:").unwrap());
        db.migrate().unwrap();
        // Test exactly at T1 threshold (168 hours)
        evaluate_vault(&db, 200, 168).await;
        let events = db.get_escalation_events(200).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].tier, EscalationTier::T1);
    }

    #[tokio::test]
    async fn test_t2_threshold_exact_boundary() {
        let db = Arc::new(Db::open(":memory:").unwrap());
        db.migrate().unwrap();
        // Test exactly at T2 threshold (72 hours)
        evaluate_vault(&db, 201, 72).await;
        let events = db.get_escalation_events(201).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].tier, EscalationTier::T2);
    }

    #[tokio::test]
    async fn test_t3_threshold_exact_boundary() {
        let db = Arc::new(Db::open(":memory:").unwrap());
        db.migrate().unwrap();
        // Test exactly at T3 threshold (24 hours)
        evaluate_vault(&db, 202, 24).await;
        let events = db.get_escalation_events(202).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].tier, EscalationTier::T3);
    }

    #[tokio::test]
    async fn test_channels_dispatched_per_tier() {
        let db = Arc::new(Db::open(":memory:").unwrap());
        db.migrate().unwrap();
        // Test T1 channels (email only)
        evaluate_vault(&db, 300, 100).await;
        let t1_events = db.get_escalation_events(300).unwrap();
        assert_eq!(t1_events[0].channels, vec!["email"]);

        // Clear and test T2 channels (email + sms)
        db.clear_escalation_state(300).unwrap();
        let mut state = db.get_escalation_state(300).unwrap();
        state.escalated_at = Some(Utc::now() - chrono::Duration::hours(25));
        db.upsert_escalation_state(&state).unwrap();
        evaluate_vault(&db, 300, 48).await;
        let t2_events = db.get_escalation_events(300).unwrap();
        assert_eq!(t2_events[0].channels, vec!["email", "sms"]);

        // Clear and test T3 channels (email + sms + emergency_contact)
        db.clear_escalation_state(300).unwrap();
        let mut state = db.get_escalation_state(300).unwrap();
        state.escalated_at = Some(Utc::now() - chrono::Duration::hours(25));
        db.upsert_escalation_state(&state).unwrap();
        evaluate_vault(&db, 300, 12).await;
        let t3_events = db.get_escalation_events(300).unwrap();
        assert_eq!(t3_events[0].channels, vec!["email", "sms", "emergency_contact"]);
    }
}
