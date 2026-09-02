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
    db::Db,
    models::{EscalationEvent, EscalationState, EscalationTier, TimelineEvent, TimelineEventKind},
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
        let ttl_hours = fetch_ttl_hours(pref.vault_id).await;
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

/// Actually dispatch the escalation: log the event, update state, and record
/// a timeline entry.
async fn dispatch_escalation(db: &Arc<Db>, vault_id: u64, tier: EscalationTier) {
    let channels: Vec<String> = channels_for_tier(tier)
        .into_iter()
        .map(String::from)
        .collect();
    let now = Utc::now();
    let event_id = Uuid::new_v4().to_string();

    tracing::info!(vault_id, ?tier, ?channels, "escalation: dispatching tier");

    // Stub: in production, call email/SMS/emergency-contact providers here.
    send_escalation_notifications(vault_id, tier, &channels).await;

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

/// Stub: dispatches notifications via the configured channels.
/// Replace with real email/SMS/emergency-contact integrations in production.
async fn send_escalation_notifications(vault_id: u64, tier: EscalationTier, channels: &[String]) {
    for channel in channels {
        tracing::info!(vault_id, ?tier, channel, "escalation: sending notification");
    }
}

/// Stub: returns hours remaining until TTL expiry for a vault.
/// Replace with a real Stellar RPC call in production.
async fn fetch_ttl_hours(_vault_id: u64) -> u32 {
    u32::MAX
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

    #[tokio::test]
    async fn test_evaluate_vault_no_escalation_needed() {
        let db = Arc::new(Db::open(":memory:").unwrap());
        db.migrate().unwrap();
        // TTL far from expiry — no escalation should be written.
        evaluate_vault(&db, 1, 300).await;
        let events = db.get_escalation_events(1).unwrap();
        assert!(events.is_empty(), "no escalation expected for TTL=300h");
    }

    #[tokio::test]
    async fn test_evaluate_vault_dispatches_t1() {
        let db = Arc::new(Db::open(":memory:").unwrap());
        db.migrate().unwrap();
        // TTL at T1 threshold.
        evaluate_vault(&db, 42, 100).await;
        let events = db.get_escalation_events(42).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].tier, EscalationTier::T1);
    }

    #[tokio::test]
    async fn test_evaluate_vault_deduplication() {
        let db = Arc::new(Db::open(":memory:").unwrap());
        db.migrate().unwrap();
        // First evaluation dispatches T1.
        evaluate_vault(&db, 99, 100).await;
        // Second evaluation within the dedup window should NOT dispatch again.
        evaluate_vault(&db, 99, 100).await;
        let events = db.get_escalation_events(99).unwrap();
        assert_eq!(events.len(), 1, "deduplication: only one event expected");
    }

    #[tokio::test]
    async fn test_evaluate_vault_promotes_to_higher_tier() {
        let db = Arc::new(Db::open(":memory:").unwrap());
        db.migrate().unwrap();
        // T1 is dispatched first.
        evaluate_vault(&db, 7, 100).await;
        // Force-expire the dedup window by manipulating the state timestamp.
        let mut state = db.get_escalation_state(7).unwrap().unwrap();
        state.escalated_at = Some(Utc::now() - chrono::Duration::hours(25));
        db.upsert_escalation_state(&state).unwrap();
        // Now TTL drops to T2 threshold — should promote.
        evaluate_vault(&db, 7, 48).await;
        let events = db.get_escalation_events(7).unwrap();
        assert_eq!(
            events.len(),
            2,
            "two escalation events expected (T1 then T2)"
        );
        // The most recent event should be T2.
        assert_eq!(events[0].tier, EscalationTier::T2);
    }
}
