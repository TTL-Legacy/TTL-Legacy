#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

use crate::tests::common::setup_with_roles as setup;

#[test]
fn test_grace_period_active_blocks_release() {
    let (env, owner, beneficiary, admin, token_address, client) = setup();

    // Set grace period to 100 seconds
    let mut config = client.get_protocol_config();
    config.release_grace_period_seconds = 100;
    client.propose_protocol_config(&config);

    // fast forward 24 hours (86400 seconds) to apply protocol config
    env.ledger().set_timestamp(env.ledger().timestamp() + 86400);
    client.apply_protocol_config();

    // Create vault with check-in interval of 500 seconds
    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    // Initial check in to reset the timestamp
    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Fast forward to expiry (now + 500)
    now += 500;
    env.ledger().set_timestamp(now);

    // It is expired now, but grace period is 100 seconds
    // So attempting to trigger release should fail with GracePeriodActive
    let err = client.try_trigger_release(&vault_id).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(83)); // GracePeriodActive

    // Fast forward past the grace period
    now += 100;
    env.ledger().set_timestamp(now);

    // Release should now succeed
    client.trigger_release(&vault_id);
    assert_eq!(client.get_vault(&vault_id).status, ReleaseStatus::Released);
}

#[test]
fn test_grace_period_default_allows_immediate_release() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    // Create vault with check-in interval of 500 seconds
    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    // Initial check in to reset the timestamp
    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Fast forward to expiry (now + 500)
    now += 500;
    env.ledger().set_timestamp(now);

    // Default grace period is 0, so release should succeed immediately
    client.trigger_release(&vault_id);
    assert_eq!(client.get_vault(&vault_id).status, ReleaseStatus::Released);
}

// ─── Issue #2029: extend_grace_period accumulation guard ─────────────────────

/// A single call to `extend_grace_period` with a delta that fits within
/// `MAX_GRACE_PERIOD_SECONDS` must succeed and persist the new deadline.
#[test]
fn test_extend_grace_period_single_valid_extension() {
    let (env, _owner, _beneficiary, _admin, _token_address, client) = setup();

    // Start from zero; add 100 seconds — well within the ceiling.
    env.ledger().set_timestamp(1_000);
    client.extend_grace_period(&100u64);

    let config = client.get_protocol_config();
    assert_eq!(
        config.release_grace_period_seconds, 100,
        "grace period must be 100 after a single valid extension"
    );
}

/// A series of small extensions that individually look harmless must be
/// rejected once their sum would exceed `MAX_GRACE_PERIOD_SECONDS`.
///
/// This is the core regression test for Issue #2029: previously the check
/// validated only the incremental delta (always small), so accumulation
/// could bypass the ceiling.  The fix compares the resulting absolute
/// deadline, not just the delta.
#[test]
fn test_extend_grace_period_accumulation_blocked() {
    let (env, _owner, _beneficiary, _admin, _token_address, client) = setup();

    env.ledger().set_timestamp(1_000);

    // Set the grace period close to the maximum via propose/apply so we start
    // from a known large baseline.
    let near_max = MAX_GRACE_PERIOD_SECONDS - 1; // 1 second below ceiling
    let mut config = client.get_protocol_config();
    config.release_grace_period_seconds = near_max;
    client.propose_protocol_config(&config);

    // Apply after the 24-hour timelock.
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 86_401);
    client.apply_protocol_config();

    let stored = client.get_protocol_config();
    assert_eq!(stored.release_grace_period_seconds, near_max);

    // Any positive delta would push the total over MAX_GRACE_PERIOD_SECONDS.
    // Even a delta of 1 must be rejected.
    let result = client.try_extend_grace_period(&1u64);
    assert!(
        result.is_err(),
        "extending past MAX_GRACE_PERIOD_SECONDS must be rejected"
    );

    // The stored value must remain unchanged.
    let after = client.get_protocol_config();
    assert_eq!(
        after.release_grace_period_seconds, near_max,
        "grace period must not change after a rejected extension"
    );
}

/// Calling `extend_grace_period` with a delta of 0 must be rejected.
#[test]
fn test_extend_grace_period_zero_delta_rejected() {
    let (_env, _owner, _beneficiary, _admin, _token_address, client) = setup();
    let result = client.try_extend_grace_period(&0u64);
    assert!(result.is_err(), "zero delta must be rejected");
}
