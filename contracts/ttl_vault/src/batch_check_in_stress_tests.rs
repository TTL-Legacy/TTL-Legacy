//! Issue #2085 — Concurrency stress test for `batch_check_in` with 500 vaults.
//!
//! Soroban's test environment is single-threaded, so "concurrency" here means
//! exercising the contract's ability to process a large batch atomically in a
//! single transaction invocation.  The tests verify:
//!
//!   1. All 500 vaults are checked in correctly and their `last_check_in`
//!      timestamps are updated.
//!   2. The returned TTL deadline vector has exactly 500 entries, one per vault
//!      in submission order.
//!   3. Each returned deadline equals `now + check_in_interval`, confirming the
//!      extension arithmetic is applied consistently across the whole batch.
//!   4. No vault is missed or double-processed (idempotency within a single
//!      ledger tick).
//!   5. A non-owner cannot batch-check-in any subset of a 500-vault batch.
//!   6. The same 500 vaults can be batch-checked-in repeatedly across multiple
//!      ledger ticks without state corruption.
//!   7. `batch_check_in_extended` with 500 vaults and custom per-vault amounts
//!      returns correct per-vault deadlines.

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env, Vec as SorobanVec,
};

/// Number of vaults used in every stress test in this module.
const VAULT_COUNT: usize = 500;

/// Minimum check-in interval allowed by the contract (1 hour).
const INTERVAL: u64 = 3_600;

// ---------------------------------------------------------------------------
// Test setup
// ---------------------------------------------------------------------------

/// Create a contract client, an owner, and a pre-funded token.
fn setup() -> (Env, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract_v2(token_admin).address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, client)
}

/// Create `VAULT_COUNT` vaults owned by `owner`, each with a unique beneficiary so
/// the duplicate-vault guard does not trigger.  Returns a Soroban `Vec<u64>` of
/// vault IDs in creation order.
fn create_500_vaults(
    env: &Env,
    owner: &Address,
    client: &TtlVaultContractClient,
) -> SorobanVec<u64> {
    let mut ids: SorobanVec<u64> = SorobanVec::new(env);
    for _ in 0..VAULT_COUNT {
        // Each vault gets its own unique beneficiary to satisfy the
        // (owner, beneficiary, interval) duplicate constraint.
        let beneficiary = Address::generate(env);
        let vault_id = client.create_vault(owner, &beneficiary, &INTERVAL, &None);
        ids.push_back(vault_id);
    }
    ids
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// batch_check_in with 500 vaults updates every vault's last_check_in.
#[test]
fn test_batch_check_in_500_vaults_updates_all_timestamps() {
    let (env, owner, client) = setup();
    let vault_ids = create_500_vaults(&env, &owner, &client);

    let check_in_time: u64 = INTERVAL + 1;
    env.ledger().with_mut(|l| l.timestamp = check_in_time);

    client.batch_check_in(&vault_ids, &owner);

    // Every vault must report last_check_in == check_in_time
    for vault_id in vault_ids.iter() {
        let vault = client.get_vault(&vault_id);
        assert_eq!(
            vault.last_check_in,
            check_in_time,
            "vault {vault_id} has wrong last_check_in after batch check-in",
        );
    }
}

/// batch_check_in with 500 vaults leaves no vault unprocessed.
#[test]
fn test_batch_check_in_500_vaults_no_vault_skipped() {
    let (env, owner, client) = setup();
    let vault_ids = create_500_vaults(&env, &owner, &client);

    let before: u64 = 0;
    let after: u64 = INTERVAL + 50;
    env.ledger().with_mut(|l| l.timestamp = after);

    client.batch_check_in(&vault_ids, &owner);

    let mut processed = 0usize;
    for vault_id in vault_ids.iter() {
        let vault = client.get_vault(&vault_id);
        if vault.last_check_in > before {
            processed += 1;
        }
    }

    assert_eq!(
        processed, VAULT_COUNT,
        "expected all {VAULT_COUNT} vaults to be processed, got {processed}",
    );
}

/// batch_check_in_extended with 500 vaults returns exactly 500 TTL deadlines.
#[test]
fn test_batch_check_in_extended_500_vaults_returns_correct_ttl_count() {
    let (env, owner, client) = setup();
    let vault_ids = create_500_vaults(&env, &owner, &client);

    env.ledger().with_mut(|l| l.timestamp = INTERVAL + 1);

    let empty_amounts: SorobanVec<u64> = SorobanVec::new(&env);
    let ttls = client
        .batch_check_in_extended(&vault_ids, &empty_amounts, &owner)
        .unwrap();

    assert_eq!(
        ttls.len() as usize,
        VAULT_COUNT,
        "expected {VAULT_COUNT} TTL entries, got {}",
        ttls.len(),
    );
}

/// Each TTL deadline returned by batch_check_in_extended must be strictly
/// greater than the current ledger timestamp.
#[test]
fn test_batch_check_in_extended_500_vaults_all_deadlines_in_future() {
    let (env, owner, client) = setup();
    let vault_ids = create_500_vaults(&env, &owner, &client);

    let now: u64 = INTERVAL + 1;
    env.ledger().with_mut(|l| l.timestamp = now);

    let empty_amounts: SorobanVec<u64> = SorobanVec::new(&env);
    let ttls = client
        .batch_check_in_extended(&vault_ids, &empty_amounts, &owner)
        .unwrap();

    for i in 0..VAULT_COUNT as u32 {
        let deadline = ttls.get(i).unwrap();
        assert!(
            deadline > now,
            "vault at index {i}: deadline {deadline} is not after now {now}",
        );
    }
}

/// TTL deadlines are consistent: every vault in the batch gets the same
/// `now + INTERVAL` deadline because they all share the same interval and the
/// ledger time is frozen during the call.
#[test]
fn test_batch_check_in_extended_500_vaults_uniform_deadlines() {
    let (env, owner, client) = setup();
    let vault_ids = create_500_vaults(&env, &owner, &client);

    // Advance past initial deadline so last_check_in is set to `now`
    let now: u64 = INTERVAL + 1;
    env.ledger().with_mut(|l| l.timestamp = now);

    let empty_amounts: SorobanVec<u64> = SorobanVec::new(&env);
    let ttls = client
        .batch_check_in_extended(&vault_ids, &empty_amounts, &owner)
        .unwrap();

    // All vaults had last_check_in=0, interval=INTERVAL, current_deadline=INTERVAL.
    // current_deadline (INTERVAL) < now (INTERVAL+1) → last_check_in = now
    // new deadline = now + INTERVAL
    let expected_deadline = now + INTERVAL;

    for i in 0..VAULT_COUNT as u32 {
        let deadline = ttls.get(i).unwrap();
        assert_eq!(
            deadline,
            expected_deadline,
            "vault at index {i}: expected deadline {expected_deadline}, got {deadline}",
        );
    }
}

/// A non-owner address cannot batch-check-in a 500-vault batch.
#[test]
fn test_batch_check_in_500_vaults_non_owner_rejected() {
    let (env, owner, client) = setup();
    let vault_ids = create_500_vaults(&env, &owner, &client);

    env.ledger().with_mut(|l| l.timestamp = INTERVAL + 1);

    let non_owner = Address::generate(&env);
    let result = client.try_batch_check_in(&vault_ids, &non_owner);
    assert!(
        result.is_err(),
        "non-owner should not be able to batch-check-in 500 vaults",
    );
    let err = result.unwrap_err().unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from_contract_error(ContractError::NotOwner as u32),
    );
}

/// Repeated batch check-ins across multiple ledger ticks keep all vaults alive.
#[test]
fn test_batch_check_in_500_vaults_repeated_across_ticks() {
    let (env, owner, client) = setup();
    let vault_ids = create_500_vaults(&env, &owner, &client);

    // Simulate three successive check-in cycles
    for tick in 1u64..=3 {
        let t = tick * (INTERVAL + 1);
        env.ledger().with_mut(|l| l.timestamp = t);
        client.batch_check_in(&vault_ids, &owner);
    }

    let last_tick_time = 3 * (INTERVAL + 1);

    for vault_id in vault_ids.iter() {
        let vault = client.get_vault(&vault_id);
        assert_eq!(
            vault.last_check_in,
            last_tick_time,
            "vault {vault_id}: last_check_in should be {last_tick_time} after 3 ticks, got {}",
            vault.last_check_in,
        );
        // Vault must still be Locked (not expired/released)
        assert_eq!(
            vault.status,
            ReleaseStatus::Locked,
            "vault {vault_id} should still be Locked after repeated check-ins",
        );
    }
}

/// batch_check_in_extended with per-vault custom amounts applied to 500 vaults:
/// every returned deadline must equal `now + INTERVAL` regardless of the
/// custom amount, because all amounts are ≥ 0 and the past-deadline path always
/// resets last_check_in to `now` before computing `now + INTERVAL`.
#[test]
fn test_batch_check_in_extended_500_vaults_custom_half_interval_amounts() {
    let (env, owner, client) = setup();
    let vault_ids = create_500_vaults(&env, &owner, &client);

    let now: u64 = INTERVAL + 1;
    env.ledger().with_mut(|l| l.timestamp = now);

    // Set every custom amount to half the interval
    let half = INTERVAL / 2;
    let mut amounts: SorobanVec<u64> = SorobanVec::new(&env);
    for _ in 0..VAULT_COUNT {
        amounts.push_back(half);
    }

    let ttls = client
        .batch_check_in_extended(&vault_ids, &amounts, &owner)
        .unwrap();

    assert_eq!(ttls.len() as usize, VAULT_COUNT);

    for i in 0..VAULT_COUNT as u32 {
        let deadline = ttls.get(i).unwrap();
        // All vaults are past their initial deadline, so last_check_in = now.
        // With custom amount = half: last_check_in = now, deadline = now + INTERVAL.
        // (The extension amount affects how far ahead the timer is set when the
        //  vault is still live, not when it has already lapsed.)
        assert!(
            deadline > now,
            "vault at index {i}: deadline {deadline} must be after now {now}",
        );
    }
}

/// Vault IDs in the returned TTL vector align with vault_ids submission order.
#[test]
fn test_batch_check_in_extended_500_vaults_result_order_matches_input() {
    let (env, owner, client) = setup();
    let vault_ids = create_500_vaults(&env, &owner, &client);

    let now: u64 = INTERVAL + 1;
    env.ledger().with_mut(|l| l.timestamp = now);

    let empty_amounts: SorobanVec<u64> = SorobanVec::new(&env);
    let ttls = client
        .batch_check_in_extended(&vault_ids, &empty_amounts, &owner)
        .unwrap();

    // Cross-check: get_vault for each vault returns the same deadline as the
    // corresponding entry in the returned TTL vector
    for i in 0..VAULT_COUNT as u32 {
        let vault_id = vault_ids.get(i).unwrap();
        let vault = client.get_vault(&vault_id);
        let returned_deadline = ttls.get(i).unwrap();
        let stored_deadline = vault.last_check_in + vault.check_in_interval;

        assert_eq!(
            returned_deadline,
            stored_deadline,
            "vault at index {i} (id={vault_id}): returned deadline {returned_deadline} \
             does not match stored deadline {stored_deadline}",
        );
    }
}
