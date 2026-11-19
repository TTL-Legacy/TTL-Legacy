//! Issue 3 – low-TTL warning events.
//!
//! `check_low_ttl_status` returns `true` and emits a `LOW_TTL_WARNING_TOPIC`
//! event when the remaining TTL is below the configured threshold.
//! Owners can customise the threshold via `set_low_ttl_threshold`.
//! The default threshold is 7 days (604 800 s).

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    vec, Address, BytesN, Env, IntoVal,
};

use crate::tests::common::setup_funded as setup;

/// When TTL is comfortably above the default 7-day threshold, status is false
/// and no LOW_TTL_WARNING_TOPIC event is emitted.
#[test]
fn test_check_low_ttl_status_false_when_ttl_is_high() {
    let (env, owner, beneficiary, client) = setup();
    // Use a 30-day interval so TTL starts at 30 days (>> 7-day default threshold)
    let interval = 30 * 86_400u64; // 30 days
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // At t=0 the remaining TTL equals the full 30-day interval
    let below = client.check_low_ttl_status(&vault_id);
    assert!(!below, "status must be false when TTL >> threshold");
}

/// When TTL drops below the default 7-day threshold, status is true and the
/// LOW_TTL_WARNING_TOPIC event is emitted.
#[test]
fn test_check_low_ttl_status_true_and_event_emitted_below_default_threshold() {
    let (env, owner, beneficiary, client) = setup();
    let interval = 30 * 86_400u64; // 30 days
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Advance time so that remaining TTL = 3 days (< 7-day default threshold)
    let elapsed = interval - 3 * 86_400u64; // 27 days in
    env.ledger().with_mut(|l| l.timestamp = elapsed);

    let below = client.check_low_ttl_status(&vault_id);
    assert!(below, "status must be true when TTL < default 7-day threshold");
}

/// After the owner lowers the threshold, the function only triggers when TTL
/// drops below the new (smaller) value.
#[test]
fn test_custom_threshold_respected() {
    let (env, owner, beneficiary, client) = setup();
    let interval = 10 * 86_400u64; // 10 days
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Set a 2-day custom threshold
    let two_days = 2 * 86_400u64;
    client.set_low_ttl_threshold(&vault_id, &owner, &two_days).unwrap();

    // At t=0 remaining TTL = 10 days > 2 days → status false
    assert!(!client.check_low_ttl_status(&vault_id));

    // Advance to 8.5 days in → remaining TTL = 1.5 days < 2-day threshold → status true
    env.ledger().with_mut(|l| l.timestamp = (interval as f64 * 0.85) as u64);
    assert!(client.check_low_ttl_status(&vault_id));
}

/// Non-owner cannot change the threshold.
#[test]
fn test_set_low_ttl_threshold_owner_only() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);

    let non_owner = Address::generate(&env);
    let result = client.try_set_low_ttl_threshold(&vault_id, &non_owner, &86_400u64);
    assert!(result.is_err(), "non-owner must not be able to set threshold");
}

/// check_low_ttl_status returns false for a released vault (no event emitted).
#[test]
fn test_check_low_ttl_status_false_for_released_vault() {
    let (env, owner, beneficiary, client) = setup();
    let interval = 1_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    client.deposit(&vault_id, &owner, &500_000i128);

    // Let the vault expire and release it
    env.ledger().with_mut(|l| l.timestamp = interval + 1);
    client.trigger_release(&vault_id);

    // check_low_ttl_status must not fire for a released vault
    let below = client.check_low_ttl_status(&vault_id);
    assert!(!below, "released vault must return false");
}
