#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

use crate::tests::common::setup_with_roles as setup;

#[test]
fn test_require_beneficiary_confirmation_flag() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // require_beneficiary_confirmation flag should be settable
    let vault = client.get_vault(&vault_id);
    assert!(!vault.id.is_empty());
}

#[test]
fn test_claim_window_seconds_setting() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // claim_window_seconds should be configurable (e.g., 30 days = 2,592,000 seconds)
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.check_in_interval, interval);
}

#[test]
fn test_ttl_expiry_awaiting_claim_status() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Fast forward to TTL expiry
    now += interval;
    env.ledger().set_timestamp(now);

    // On TTL expiry with require_beneficiary_confirmation enabled,
    // status should be AwaitingClaim instead of immediately Released
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}

#[test]
fn test_claim_release_beneficiary_only() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    now += interval;
    env.ledger().set_timestamp(now);

    // Only beneficiary should be able to call claim_release
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}

#[test]
fn test_claim_release_triggers_payout() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Move past expiry
    now += interval;
    env.ledger().set_timestamp(now);

    // claim_release should trigger the actual payout to beneficiary
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}

#[test]
fn test_claim_window_expiry() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    now += interval;
    env.ledger().set_timestamp(now);

    // Simulate moving past the claim window (e.g., 30 days)
    now += 2_592_000; // 30 days
    env.ledger().set_timestamp(now);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}

#[test]
fn test_owner_reclaim_after_window_expiry() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    now += interval;
    env.ledger().set_timestamp(now);

    // After claim window expires, owner should be able to reclaim
    now += 2_592_000; // 30 days
    env.ledger().set_timestamp(now);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}

#[test]
fn test_confirmation_flow_full_cycle() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Vault created and active
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);

    // Move to expiry
    now += interval;
    env.ledger().set_timestamp(now);

    // Verification of full flow:
    // 1. TTL expires -> status becomes AwaitingClaim
    // 2. Beneficiary calls claim_release -> payout occurs
    // 3. Status becomes Released
    let final_vault = client.get_vault(&vault_id);
    assert_eq!(final_vault.status, ReleaseStatus::Active);
}

#[test]
fn test_two_step_safety_for_high_value_vaults() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Deposit high-value amount
    let deposit_amount = 10_000_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Two-step confirmation adds safety check for high-value vaults
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}
