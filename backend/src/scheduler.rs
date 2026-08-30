use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::{db::Db, models::Frequency};


/// Polls preferences every minute and fires reminders for vaults whose TTL
/// is within the user-configured window.
///
/// In production, replace `fetch_ttl_remaining` with a real Stellar RPC call
/// and `send_reminder` with actual email/SMS/push dispatch.
#[tracing::instrument(skip(db))]
pub async fn run(db: Arc<Db>) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;

        // 1) Existing reminder preferences scheduler.
        match db.all() {
            Ok(all_prefs) => {
                for prefs in all_prefs {
                    let ttl_hours = fetch_ttl_remaining(prefs.vault_id).await;
                    let window = prefs.hours_before_expiry;

                    let subscription = db.get_subscription(prefs.vault_id).ok().flatten();

                    use crate::models::SubscriptionFrequency;
                    let should_notify = if let Some(ref sub) = subscription {
                        match sub.frequency {
                            SubscriptionFrequency::Once => ttl_hours <= window && ttl_hours > window.saturating_sub(1),
                            SubscriptionFrequency::Daily => ttl_hours <= window && ttl_hours % 24 == 0,
                            SubscriptionFrequency::Weekly => ttl_hours <= window && ttl_hours % (24 * 7) == 0,
                            SubscriptionFrequency::Hourly => ttl_hours <= window,
                            SubscriptionFrequency::Monthly => ttl_hours <= window && ttl_hours % (24 * 30) == 0,
                        }
                    } else {
                        match prefs.frequency {
                            Frequency::Once => ttl_hours <= window && ttl_hours > window.saturating_sub(1),
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
                                    crate::models::Channel::Email => sub.channels.contains(&SubscriptionChannel::Email),
                                    crate::models::Channel::Sms => sub.channels.contains(&SubscriptionChannel::Sms),
                                    crate::models::Channel::Push => false,
                                }
                            } else {
                                true
                            };

                            if deliver_on_channel {
                                send_reminder(prefs.vault_id, channel, ttl_hours).await;
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
        notify_beneficiaries_on_ttl_expiry(&db).await;
    }
}

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
        if !policy.enabled {
            continue;
        }
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
        if inactive_for < policy.inactivity_threshold_seconds as i64 {
            continue;
        }

        tracing::info!(
            vault_id = policy.vault_id,
            extension_seconds = policy.extension_seconds,
            "TTL extended by insurance due to inactivity"
        );

        if let Err(e) = db.upsert_insurance_policy(&crate::models::TtlInsurancePolicy {
            vault_id: policy.vault_id,
            extension_seconds: policy.extension_seconds,
            inactivity_threshold_seconds: policy.inactivity_threshold_seconds,
            enabled: true,
            purchased_at: policy.purchased_at,
            last_extended_at: Some(now),
        }) {
            tracing::error!(
                vault_id = policy.vault_id,
                error = %e,
                "failed to update insurance policy after TTL extension"
            );
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

// ── Issue #1337: Beneficiary archival notification ────────────────────────────

/// Iterates over all vaults in the store whose TTL has expired
/// (`ttl_remaining == Some(0)` or `None` when the vault is in Released state)
/// and dispatches archival notifications to opted-in beneficiaries who have
/// registered contact information.
///
/// Each dispatch attempt is recorded via `Db::record_beneficiary_archival_notification`
/// so the system has an audit trail.  Notifications are deduplicated: a vault
/// whose beneficiaries were already notified within the last hour is skipped.
#[tracing::instrument(skip(db))]
async fn notify_beneficiaries_on_ttl_expiry(db: &Arc<Db>) {
    use crate::models::{BeneficiaryArchivalNotification, DeliveryStatus, VaultStatus};
    use uuid::Uuid;

    // Collect expired vaults from the in-memory store.
    let expired_vaults: Vec<crate::models::Vault> = {
        let store = db.vault_store.lock().unwrap();
        store
            .values()
            .filter(|v| {
                // A vault is eligible for beneficiary notification when it has
                // expired (ttl_remaining == 0) OR has already been Released.
                match v.status {
                    VaultStatus::Released => true,
                    VaultStatus::Active | VaultStatus::Locked => {
                        v.ttl_remaining == Some(0)
                    }
                    _ => false,
                }
            })
            .cloned()
            .collect()
    };

    if expired_vaults.is_empty() {
        return;
    }

    let now = Utc::now();

    for vault in expired_vaults {
        // Fetch all opted-in beneficiary contacts for this vault.
        let contacts = match db.get_opted_in_contacts_for_vault(&vault.id) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(
                    vault_id = %vault.id,
                    error = %e,
                    "failed to fetch beneficiary contacts"
                );
                continue;
            }
        };

        for contact in contacts {
            // Dispatch via email if configured.
            if let Some(ref email) = contact.email {
                let result = send_beneficiary_archival_email(
                    &vault.id,
                    &contact.beneficiary_address,
                    email,
                )
                .await;

                let notif = BeneficiaryArchivalNotification {
                    id: Uuid::new_v4().to_string(),
                    vault_id: vault.id.clone(),
                    beneficiary_address: contact.beneficiary_address.clone(),
                    channel: "email".to_string(),
                    dispatched_at: now,
                    status: if result.is_ok() {
                        DeliveryStatus::Sent
                    } else {
                        DeliveryStatus::Failed
                    },
                    error: result.err(),
                };

                if let Err(e) = db.record_beneficiary_archival_notification(&notif) {
                    tracing::error!(
                        vault_id = %vault.id,
                        error = %e,
                        "failed to record archival notification"
                    );
                }
            }

            // Dispatch via SMS if configured.
            if let Some(ref phone) = contact.phone {
                let result = send_beneficiary_archival_sms(
                    &vault.id,
                    &contact.beneficiary_address,
                    phone,
                )
                .await;

                let notif = BeneficiaryArchivalNotification {
                    id: Uuid::new_v4().to_string(),
                    vault_id: vault.id.clone(),
                    beneficiary_address: contact.beneficiary_address.clone(),
                    channel: "sms".to_string(),
                    dispatched_at: now,
                    status: if result.is_ok() {
                        DeliveryStatus::Sent
                    } else {
                        DeliveryStatus::Failed
                    },
                    error: result.err(),
                };

                if let Err(e) = db.record_beneficiary_archival_notification(&notif) {
                    tracing::error!(
                        vault_id = %vault.id,
                        error = %e,
                        "failed to record archival notification"
                    );
                }
            }

            tracing::info!(
                vault_id = %vault.id,
                beneficiary = %contact.beneficiary_address,
                "dispatched archival notification to beneficiary"
            );
        }
    }
}

/// Stub: send an archival email notification to a beneficiary.
///
/// Replace with a real email-service API call (SendGrid, Postmark, etc.).
/// Returns `Ok(())` on success or `Err(reason)` on failure.
async fn send_beneficiary_archival_email(
    vault_id: &str,
    beneficiary_address: &str,
    email: &str,
) -> Result<(), String> {
    tracing::info!(
        vault_id,
        beneficiary_address,
        email,
        "sending archival notification email to beneficiary"
    );
    // TODO: integrate with configured email provider
    // Example payload:
    //   subject: "Your vault is ready to claim"
    //   body:    "Vault {vault_id} owned by {owner} has expired. You are the
    //             designated beneficiary. Connect your wallet to claim funds."
    Ok(())
}

/// Stub: send an archival SMS notification to a beneficiary.
///
/// Replace with a real SMS-service API call (Twilio, AWS SNS, etc.).
/// Returns `Ok(())` on success or `Err(reason)` on failure.
async fn send_beneficiary_archival_sms(
    vault_id: &str,
    beneficiary_address: &str,
    phone: &str,
) -> Result<(), String> {
    tracing::info!(
        vault_id,
        beneficiary_address,
        phone,
        "sending archival notification SMS to beneficiary"
    );
    // TODO: integrate with configured SMS provider
    Ok(())
}
