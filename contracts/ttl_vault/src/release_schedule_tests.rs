//! Tests for #951: Graduated Release Schedule
//!
//! Verifies:
//!  - set_release_schedule stores the schedule correctly
//!  - Total amount must match vault balance
//!  - Only vault owner can set a schedule
//!  - trigger_release activates the schedule without immediate transfer
//!  - claim_tranche transfers funds after unlock time
//!  - claim_tranche rejects early, duplicate, and wrong-caller attempts
//!  - Multiple sequential tranche claims work correctly

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

// ---------------------------------------------------------------------------
// Test setup helper
// ---------------------------------------------------------------------------

fn setup_schedule() -> (
    Env,
    Address, // owner
    Address, // beneficiary
    Address, // token address
    u64,     // vault_id
    TtlVaultContractClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let admin = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    // Mint enough to cover deposits
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.deposit(&vault_id, &owner, &4_000_000);

    (env, owner, beneficiary, token_address, vault_id, client)
}

// ---------------------------------------------------------------------------
// set_release_schedule
// ---------------------------------------------------------------------------

#[test]
fn test_set_release_schedule_stores_tranches() {
    let (env, owner, _beneficiary, _token, vault_id, client) = setup_schedule();

    let now = env.ledger().timestamp();
    let tranches = vec![
        &env,
        (1_000_000i128, now + 1000u64),
        (1_000_000i128, now + 2000u64),
        (1_000_000i128, now + 3000u64),
        (1_000_000i128, now + 4000u64),
    ];

    client.set_release_schedule(&vault_id, &owner, &tranches);

    let schedule = client.get_release_schedule(&vault_id).unwrap();
    assert_eq!(schedule.tranches.len(), 4u32);
    assert_eq!(schedule.total_amount, 4_000_000);
    assert_eq!(schedule.claimed_amount, 0);
    assert!(!schedule.active); // Not active until trigger_release fires
}

#[test]
fn test_set_release_schedule_requires_owner() {
    let (env, _owner, beneficiary, _token, vault_id, client) = setup_schedule();

    let now = env.ledger().timestamp();
    let tranches = vec![&env, (4_000_000i128, now + 1000u64)];

    let result = client.try_set_release_schedule(&vault_id, &beneficiary, &tranches);
    assert!(result.is_err());
}

#[test]
fn test_set_release_schedule_total_must_match_balance() {
    let (env, owner, _beneficiary, _token, vault_id, client) = setup_schedule();

    let now = env.ledger().timestamp();
    // Total is 3_000_000 but balance is 4_000_000 → must fail
    let tranches = vec![
        &env,
        (1_000_000i128, now + 1000u64),
        (1_000_000i128, now + 2000u64),
        (1_000_000i128, now + 3000u64),
    ];

    let result = client.try_set_release_schedule(&vault_id, &owner, &tranches);
    assert!(result.is_err());
}

#[test]
fn test_set_release_schedule_rejects_zero_amount_tranche() {
    let (env, owner, _beneficiary, _token, vault_id, client) = setup_schedule();

    let now = env.ledger().timestamp();
    // One tranche has 0 amount → must fail
    let tranches = vec![
        &env,
        (0i128, now + 1000u64),
        (4_000_000i128, now + 2000u64),
    ];

    let result = client.try_set_release_schedule(&vault_id, &owner, &tranches);
    assert!(result.is_err());
}

#[test]
fn test_set_release_schedule_rejects_empty_tranches() {
    let (_env, owner, _beneficiary, _token, vault_id, client) = setup_schedule();

    let tranches: soroban_sdk::Vec<(i128, u64)> = soroban_sdk::Vec::new(&_env);
    let result = client.try_set_release_schedule(&vault_id, &owner, &tranches);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// trigger_release with a schedule — no immediate transfer
// ---------------------------------------------------------------------------

#[test]
fn test_trigger_release_activates_schedule_no_immediate_transfer() {
    let (env, owner, _beneficiary, token_address, vault_id, client) = setup_schedule();

    let now = 1000u64;
    env.ledger().set_timestamp(now);

    let tranches = vec![
        &env,
        (2_000_000i128, now + 500u64),
        (2_000_000i128, now + 1500u64),
    ];
    client.set_release_schedule(&vault_id, &owner, &tranches);

    // Expire the vault
    env.ledger()
        .set_timestamp(now + 3600 + 1); // past check_in_interval
    client.trigger_release(&vault_id);

    // Schedule must be active now
    let schedule = client.get_release_schedule(&vault_id).unwrap();
    assert!(schedule.active);
    assert_eq!(schedule.claimed_amount, 0);

    // Vault balance is still 4_000_000 (funds held for tranche claims)
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, 4_000_000);
    assert_eq!(vault.status, ReleaseStatus::Released);

    // Beneficiary token balance should still be 0 — no immediate payout
    let token_client = soroban_sdk::token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&_beneficiary), 0);
}

// Work around: capture beneficiary from outer scope via a helper
#[test]
fn test_trigger_release_activates_schedule_beneficiary_balance_unchanged() {
    let (env, owner, beneficiary, token_address, vault_id, client) = setup_schedule();

    let now = 1000u64;
    env.ledger().set_timestamp(now);

    let tranches = vec![&env, (4_000_000i128, now + 100u64)];
    client.set_release_schedule(&vault_id, &owner, &tranches);

    env.ledger().set_timestamp(now + 3600 + 1);
    client.trigger_release(&vault_id);

    let token_client = soroban_sdk::token::Client::new(&env, &token_address);
    // No transfer yet — schedule not claimed
    assert_eq!(token_client.balance(&beneficiary), 0);
}

// ---------------------------------------------------------------------------
// claim_tranche
// ---------------------------------------------------------------------------

#[test]
fn test_claim_tranche_transfers_correct_amount() {
    let (env, owner, beneficiary, token_address, vault_id, client) = setup_schedule();

    let start = 1000u64;
    env.ledger().set_timestamp(start);

    let tranches = vec![
        &env,
        (1_000_000i128, start + 500u64),  // unlocks at t=1500
        (3_000_000i128, start + 2000u64), // unlocks at t=3000
    ];
    client.set_release_schedule(&vault_id, &owner, &tranches);

    // Expire and release
    env.ledger().set_timestamp(start + 3600 + 1);
    client.trigger_release(&vault_id);

    // Move to after first tranche unlock
    env.ledger().set_timestamp(start + 3600 + 600);
    let claimed = client.claim_tranche(&vault_id, &beneficiary, &0u32);
    assert_eq!(claimed, 1_000_000);

    let token_client = soroban_sdk::token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&beneficiary), 1_000_000);

    // Schedule updated
    let schedule = client.get_release_schedule(&vault_id).unwrap();
    assert_eq!(schedule.claimed_amount, 1_000_000);
    assert!(schedule.tranches.get(0u32).unwrap().claimed);
    assert!(!schedule.tranches.get(1u32).unwrap().claimed);
}

#[test]
fn test_claim_tranche_rejects_before_unlock_time() {
    let (env, owner, beneficiary, _token, vault_id, client) = setup_schedule();

    let start = 1000u64;
    env.ledger().set_timestamp(start);

    let tranches = vec![&env, (4_000_000i128, start + 10_000u64)]; // far future
    client.set_release_schedule(&vault_id, &owner, &tranches);

    env.ledger().set_timestamp(start + 3600 + 1);
    client.trigger_release(&vault_id);

    // Try to claim immediately — should fail
    env.ledger().set_timestamp(start + 3601);
    let result = client.try_claim_tranche(&vault_id, &beneficiary, &0u32);
    assert!(result.is_err());
}

#[test]
fn test_claim_tranche_rejects_duplicate_claim() {
    let (env, owner, beneficiary, _token, vault_id, client) = setup_schedule();

    let start = 1000u64;
    env.ledger().set_timestamp(start);

    let tranches = vec![&env, (4_000_000i128, start + 100u64)];
    client.set_release_schedule(&vault_id, &owner, &tranches);

    env.ledger().set_timestamp(start + 3600 + 1);
    client.trigger_release(&vault_id);

    env.ledger().set_timestamp(start + 3700);
    // First claim succeeds
    client.claim_tranche(&vault_id, &beneficiary, &0u32);

    // Second claim on same tranche must fail
    let result = client.try_claim_tranche(&vault_id, &beneficiary, &0u32);
    assert!(result.is_err());
}

#[test]
fn test_claim_tranche_rejects_non_beneficiary() {
    let (env, owner, _beneficiary, _token, vault_id, client) = setup_schedule();

    let start = 1000u64;
    env.ledger().set_timestamp(start);

    let tranches = vec![&env, (4_000_000i128, start + 100u64)];
    client.set_release_schedule(&vault_id, &owner, &tranches);

    env.ledger().set_timestamp(start + 3600 + 1);
    client.trigger_release(&vault_id);

    env.ledger().set_timestamp(start + 3700);

    let stranger = Address::generate(&env);
    let result = client.try_claim_tranche(&vault_id, &stranger, &0u32);
    assert!(result.is_err());
}

#[test]
fn test_claim_tranche_rejects_invalid_index() {
    let (env, owner, beneficiary, _token, vault_id, client) = setup_schedule();

    let start = 1000u64;
    env.ledger().set_timestamp(start);

    let tranches = vec![&env, (4_000_000i128, start + 100u64)];
    client.set_release_schedule(&vault_id, &owner, &tranches);

    env.ledger().set_timestamp(start + 3600 + 1);
    client.trigger_release(&vault_id);

    env.ledger().set_timestamp(start + 3700);

    // Index 1 doesn't exist (only index 0)
    let result = client.try_claim_tranche(&vault_id, &beneficiary, &1u32);
    assert!(result.is_err());
}

#[test]
fn test_claim_tranche_rejects_before_trigger_release() {
    let (env, owner, beneficiary, _token, vault_id, client) = setup_schedule();

    let start = 1000u64;
    env.ledger().set_timestamp(start);

    let tranches = vec![&env, (4_000_000i128, start + 100u64)];
    client.set_release_schedule(&vault_id, &owner, &tranches);

    // Do NOT call trigger_release — schedule is not active
    env.ledger().set_timestamp(start + 500);
    let result = client.try_claim_tranche(&vault_id, &beneficiary, &0u32);
    assert!(result.is_err());
}

#[test]
fn test_all_tranches_can_be_claimed_sequentially() {
    let (env, owner, beneficiary, token_address, vault_id, client) = setup_schedule();

    let start = 1000u64;
    env.ledger().set_timestamp(start);

    let tranches = vec![
        &env,
        (1_000_000i128, start + 100u64),
        (1_000_000i128, start + 200u64),
        (1_000_000i128, start + 300u64),
        (1_000_000i128, start + 400u64),
    ];
    client.set_release_schedule(&vault_id, &owner, &tranches);

    env.ledger().set_timestamp(start + 3600 + 1);
    client.trigger_release(&vault_id);

    // Claim all 4 tranches sequentially, advancing time
    for i in 0u32..4 {
        env.ledger().set_timestamp(start + 3601 + (i as u64 + 1) * 100);
        let claimed = client.claim_tranche(&vault_id, &beneficiary, &i);
        assert_eq!(claimed, 1_000_000);
    }

    let schedule = client.get_release_schedule(&vault_id).unwrap();
    assert_eq!(schedule.claimed_amount, 4_000_000);

    let token_client = soroban_sdk::token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&beneficiary), 4_000_000);
}

#[test]
fn test_set_release_schedule_cannot_overwrite_active_schedule() {
    let (env, owner, _beneficiary, _token, vault_id, client) = setup_schedule();

    let start = 1000u64;
    env.ledger().set_timestamp(start);

    let tranches = vec![&env, (4_000_000i128, start + 100u64)];
    client.set_release_schedule(&vault_id, &owner, &tranches);

    env.ledger().set_timestamp(start + 3600 + 1);
    client.trigger_release(&vault_id);

    // Try to overwrite the now-active schedule
    let new_tranches = vec![&env, (4_000_000i128, start + 200u64)];
    let result = client.try_set_release_schedule(&vault_id, &owner, &new_tranches);
    // Should fail because schedule is already active AND vault is Released
    assert!(result.is_err());
}

#[test]
fn test_get_release_schedule_returns_none_when_not_set() {
    let (_env, _owner, _beneficiary, _token, vault_id, client) = setup_schedule();
    assert!(client.get_release_schedule(&vault_id).is_none());
}
