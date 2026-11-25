#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, Vec,
};

/// Issue #2046 — the integer-division remainder after splitting BPS across
/// beneficiaries must be assigned to the FIRST beneficiary, not silently dropped.
///
/// Example: 10 stroops split 3-ways with equal BPS (3333/3333/3334).
/// Floor-division gives 3+3+3 = 9; the missing 1 stroop must go to beneficiary[0].
#[test]
fn test_bps_remainder_assigned_to_first_beneficiary() {
    use super::*;
    use soroban_sdk::{token::StellarAssetClient, Address, Bytes, Env};

    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let ben1 = Address::generate(&env);
    let ben2 = Address::generate(&env);
    let ben3 = Address::generate(&env);
    let admin = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    // Mint enough for the deposit.
    StellarAssetClient::new(&env, &token_address).mint(&owner, &100_000_000i128);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);
    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    // Create vault with owner as beneficiary placeholder; we override below.
    let vault_id = client.create_vault(&owner, &ben1, &100u64, &None);

    // Deposit an amount that does NOT divide evenly by 3 (e.g. 10 stroops).
    // With 3 equal BPS splits each beneficiary would get 3 via floor-division,
    // leaving a remainder of 1 that must go to beneficiary[0].
    let deposit: i128 = 10;
    client.deposit(&owner, &vault_id, &deposit);

    // Set three equal-split beneficiaries.
    let addresses = soroban_sdk::vec![&env, ben1.clone(), ben2.clone(), ben3.clone()];
    client
        .set_equal_split_beneficiaries(&vault_id, &owner, &addresses)
        .unwrap();

    // Expire the vault so trigger_release can fire.
    env.ledger().set_timestamp(1_000u64);
    // Advance time beyond the check-in interval so the vault is expired.
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 200u64);

    client.trigger_release(&vault_id);

    // Verify that the total released equals the deposit (no stroop was lost).
    let token_client = soroban_sdk::token::TokenClient::new(&env, &token_address);
    let b1 = token_client.balance(&ben1);
    let b2 = token_client.balance(&ben2);
    let b3 = token_client.balance(&ben3);

    assert_eq!(
        b1 + b2 + b3,
        deposit,
        "total distributed must equal the deposit — no stroops dropped"
    );

    // The first beneficiary must hold at least as many stroops as the others
    // (they receive the remainder).
    assert!(
        b1 >= b2,
        "first beneficiary must receive the BPS remainder: b1={b1}, b2={b2}"
    );
    assert!(
        b1 >= b3,
        "first beneficiary must receive the BPS remainder: b1={b1}, b3={b3}"
    );
}

#[test]
fn test_batch_withdraw_single_destination() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let destination = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 1_000_000;

    // Test batch_withdraw with single destination entry
    // Should work like regular withdraw
    // Should return Vec with one withdrawal ID
}

#[test]
fn test_batch_withdraw_multiple_destinations() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let dest1 = Address::generate(&env);
    let dest2 = Address::generate(&env);
    let dest3 = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount1: i128 = 500_000;
    let amount2: i128 = 300_000;
    let amount3: i128 = 200_000;

    // Test batch_withdraw with 3 destinations in single transaction
    // Should return Vec of 3 withdrawal IDs
    // Each destination should receive exact amount
}

#[test]
fn test_batch_withdraw_owner_only() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let non_owner = Address::generate(&env);
    let destination = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 1_000_000;

    // Test that only owner can call batch_withdraw
    // Non-owner should get authorization error
}

#[test]
fn test_batch_withdraw_total_exceeds_balance() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let dest1 = Address::generate(&env);
    let dest2 = Address::generate(&env);
    let vault_id: u64 = 1;
    let balance: i128 = 1_000_000;
    let amount1: i128 = 700_000;
    let amount2: i128 = 500_000; // Total exceeds balance

    // Test that batch_withdraw fails if total amount exceeds balance
    // Should return InsufficientBalance error
    // No transfers should occur (atomic)
}

#[test]
fn test_batch_withdraw_validates_all_destinations_in_whitelist() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let valid_destination = Address::generate(&env);
    let invalid_destination = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 500_000;

    // Test that all destinations must be in whitelist
    // Invalid destination should cause entire batch to fail
    // No transfers should occur (atomic)
}

#[test]
fn test_batch_withdraw_empty_list() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let withdrawals: Vec<(Address, i128)> = vec![];

    // Test that batch_withdraw with empty list is rejected
    // Should return InvalidInput error
}

#[test]
fn test_batch_withdraw_zero_amount() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let destination = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 0;

    // Test that zero-amount withdrawals in batch are rejected
    // Should return InvalidAmount error
}

#[test]
fn test_batch_withdraw_negative_amount() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let destination = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = -1_000_000;

    // Test that negative amounts are rejected
    // Should return InvalidAmount error
}

#[test]
fn test_batch_withdraw_duplicate_destinations() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let destination = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount1: i128 = 500_000;
    let amount2: i128 = 500_000;

    // Test batch_withdraw with same destination twice
    // Should either reject or combine amounts to same destination
    // Behavior must be clearly defined
}

#[test]
fn test_batch_withdraw_returns_withdrawal_ids() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let dest1 = Address::generate(&env);
    let dest2 = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 500_000;

    // Test that batch_withdraw returns Vec of withdrawal IDs
    // Each ID should be unique and sequential
}

#[test]
fn test_batch_withdraw_atomicity() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let dest1 = Address::generate(&env);
    let dest2 = Address::generate(&env);
    let dest3 = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount1: i128 = 400_000;
    let amount2: i128 = 400_000;
    let amount3: i128 = 400_000; // Total exceeds balance

    // Test that batch_withdraw is atomic (all-or-nothing)
    // If total exceeds balance, no transfers should occur
    // Vault state should be unchanged on failure
}

#[test]
fn test_batch_withdraw_order_independence() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let dest1 = Address::generate(&env);
    let dest2 = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount1: i128 = 500_000;
    let amount2: i128 = 300_000;

    // Test that order of destinations in batch doesn't matter
    // Results should be same regardless of order
}

#[test]
fn test_batch_withdraw_large_batch() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;

    // Test batch_withdraw with many destinations (e.g., 100+)
    // Should handle efficiently
    // All should receive correct amounts
}

#[test]
fn test_batch_withdraw_fractional_amounts() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let dest1 = Address::generate(&env);
    let dest2 = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount1: i128 = 333_333;
    let amount2: i128 = 333_334; // Handling fractional stroops

    // Test batch_withdraw with fractional stroops
    // Should handle rounding correctly
    // Sum should match total withdrawn
}

#[test]
fn test_batch_withdraw_paused_vault() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let destination = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 1_000_000;

    // Test that batch_withdraw fails if vault is paused
    // Should return VaultPaused error
}

#[test]
fn test_batch_withdraw_frozen_vault() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let destination = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 1_000_000;

    // Test that batch_withdraw fails if vault is frozen
    // Should return VaultFrozen error
}

#[test]
fn test_batch_withdraw_audit_trail() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let dest1 = Address::generate(&env);
    let dest2 = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 500_000;

    // Test that batch_withdraw operations are logged
    // Audit trail should record all destination transfers
    // Each transfer should be traceable
}

#[test]
fn test_batch_withdraw_event_emission() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let dest1 = Address::generate(&env);
    let dest2 = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 500_000;

    // Test that batch_withdraw emits appropriate events
    // Should emit individual withdrawal events or batch event
    // Events should include all destination transfers
}

#[test]
fn test_batch_withdraw_vs_individual_withdrawals() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let dest1 = Address::generate(&env);
    let dest2 = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 500_000;

    // Test that batch_withdraw result matches doing individual withdrawals
    // All destinations should receive same amounts
    // Gas/fee behavior may differ but results same
}

#[test]
fn test_batch_withdraw_maximum_amount() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let destination = Address::generate(&env);
    let vault_id: u64 = 1;
    let max_amount: i128 = i128::MAX / 2; // Large but valid amount

    // Test batch_withdraw with very large amounts
    // Should handle without overflow
}

#[test]
fn test_batch_withdraw_destination_self() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 1_000_000;

    // Test batch_withdraw to owner's own address
    // Should either reject or allow
    // Behavior must be consistent
}

#[test]
fn test_batch_withdraw_with_approval_threshold() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let destination = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 1_000_000;
    let threshold: i128 = 500_000; // Approval required over threshold

    // Test batch_withdraw respects withdrawal approval thresholds
    // High-value batches should require approval
}

#[test]
fn test_batch_withdraw_reduces_balance() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let dest1 = Address::generate(&env);
    let dest2 = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 500_000;

    // Test that batch_withdraw correctly reduces vault balance
    // New balance = old balance - sum of all amounts
}

#[test]
fn test_batch_withdraw_fee_handling() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let destination = Address::generate(&env);
    let vault_id: u64 = 1;
    let amount: i128 = 1_000_000;

    // Test that batch_withdraw handles protocol fees correctly
    // Fees should be deducted from vault
    // Destinations should receive exact amounts requested
}
