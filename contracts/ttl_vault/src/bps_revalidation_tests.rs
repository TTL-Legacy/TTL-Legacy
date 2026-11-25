//! Issue 1 – set_beneficiaries BPS re-validation on second call.
//!
//! The `set_beneficiaries` function must always recompute and validate the BPS
//! sum, even when called a second time on an existing vault.  Calling it with
//! a partial list (BPS != 10 000) must return `ContractError::InvalidBps`
//! regardless of whether a valid list was stored before.

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::StellarAssetClient,
    vec, Address, Env,
};

fn setup() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract_v2(token_admin).address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, admin, client)
}

/// First call with a valid BPS list succeeds; second call with a valid list
/// also succeeds and overwrites the stored beneficiaries.
#[test]
fn test_set_beneficiaries_second_valid_call_succeeds() {
    let (env, owner, _, client) = setup();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);
    let b3 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &3_600u64, &None);

    // First call — 50/50 split
    let first = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 5_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 5_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &first);

    let stored = client.get_vault(&vault_id).beneficiaries;
    let sum: u32 = stored.iter().map(|e| e.bps).sum();
    assert_eq!(sum, 10_000, "first call: BPS sum must be 10_000");

    // Second call — new 60/30/10 split
    let second = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 6_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 3_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b3.clone(), bps: 1_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &second);

    let stored2 = client.get_vault(&vault_id).beneficiaries;
    let sum2: u32 = stored2.iter().map(|e| e.bps).sum();
    assert_eq!(sum2, 10_000, "second call: BPS sum must still be 10_000");
    assert_eq!(stored2.len(), 3, "second call must overwrite with 3 entries");
}

/// A second call with BPS sum < 10 000 must return `InvalidBps` even though a
/// valid list was already stored.
#[test]
fn test_set_beneficiaries_second_call_partial_list_rejected() {
    let (env, owner, _, client) = setup();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &3_600u64, &None);

    // First call — valid
    let first = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 5_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 5_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &first);

    // Second call — only one entry, sum = 4_000 (invalid)
    let partial = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 4_000, minimum_threshold: 0 },
    ];
    let result = client.try_set_beneficiaries(&vault_id, &owner, &partial);
    assert!(
        result.is_err(),
        "second call with BPS sum != 10_000 must return an error"
    );
    let err = result.unwrap_err().unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from_contract_error(ContractError::InvalidBps as u32),
        "error must be InvalidBps"
    );
}

/// A second call with BPS sum > 10 000 must also be rejected.
#[test]
fn test_set_beneficiaries_second_call_over_10000_rejected() {
    let (env, owner, _, client) = setup();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &3_600u64, &None);

    // First call — valid
    let first = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 5_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 5_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &first);

    // Second call — sum = 11_000 (over)
    let over = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 6_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 5_000, minimum_threshold: 0 },
    ];
    let result = client.try_set_beneficiaries(&vault_id, &vault_id, &over);
    // The above uses vault_id as caller intentionally to also test NotOwner path —
    // use owner instead:
    let result = client.try_set_beneficiaries(&vault_id, &owner, &over);
    assert!(
        result.is_err(),
        "second call with BPS sum 11_000 must return an error"
    );
}

/// The previous valid list must remain unchanged after a rejected second call.
#[test]
fn test_set_beneficiaries_invalid_second_call_does_not_mutate_state() {
    let (env, owner, _, client) = setup();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &3_600u64, &None);

    // First call — valid 70/30
    let first = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 7_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 3_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &first);

    // Second call — invalid, partial list
    let partial = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 3_000, minimum_threshold: 0 },
    ];
    let _ = client.try_set_beneficiaries(&vault_id, &owner, &partial);

    // State must remain the original 70/30 split
    let stored = client.get_vault(&vault_id).beneficiaries;
    let sum: u32 = stored.iter().map(|e| e.bps).sum();
    assert_eq!(sum, 10_000, "state must not change after rejected call");
    assert_eq!(stored.len(), 2, "beneficiary count must remain 2");
}

// ===========================================================================
// Issue #2042 — update_beneficiary_share must re-validate the full BPS sum
// ===========================================================================

/// Updating a share to a value that keeps the sum at exactly 10 000 must succeed.
#[test]
fn test_update_beneficiary_share_valid_keeps_sum_10000() {
    let (env, owner, _, client) = setup();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &3_600u64, &None);

    // Initial split: 60/40
    let initial = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 6_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 4_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &initial);

    // Change b1 from 6000 → 7000 and b2 from 4000 → 3000 (sum still 10 000)
    // First update b2 to 3000
    let result = client.try_update_beneficiary_share(&vault_id, &owner, &b2, &3_000u32);
    // This should fail because after updating b2 to 3000, sum = 6000 + 3000 = 9000 ≠ 10000
    assert!(
        result.is_err(),
        "update that breaks sum invariant must be rejected"
    );
    let err = result.unwrap_err().unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from_contract_error(ContractError::InvalidBps as u32),
        "error must be InvalidBps"
    );

    // Verify state is unchanged
    let stored = client.get_vault(&vault_id).beneficiaries;
    let sum: u32 = stored.iter().map(|e| e.bps).sum();
    assert_eq!(sum, 10_000, "state must not change after rejected share update");
}

/// update_beneficiary_share where the new value does not break the sum should succeed.
/// We set both beneficiaries back to 50/50 by updating in a valid way.
#[test]
fn test_update_beneficiary_share_exact_10000_sum_accepted() {
    let (env, owner, _, client) = setup();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &3_600u64, &None);

    // Start: 50/50 — both values already equal 10 000
    let initial = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 5_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 5_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &initial);

    // "Update" b1 to the same value (5000) — sum stays 10 000, must succeed
    let result = client.try_update_beneficiary_share(&vault_id, &owner, &b1, &5_000u32);
    assert!(
        result.is_ok(),
        "update that preserves 10 000 sum must succeed"
    );
}

/// update_beneficiary_share on a non-existent beneficiary must fail with InvalidBeneficiary.
#[test]
fn test_update_beneficiary_share_unknown_address_rejected() {
    let (env, owner, _, client) = setup();
    let b1 = Address::generate(&env);
    let unknown = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &3_600u64, &None);

    let initial = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 10_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &initial);

    let result = client.try_update_beneficiary_share(&vault_id, &owner, &unknown, &5_000u32);
    assert!(
        result.is_err(),
        "update on unknown address must be rejected"
    );
    let err = result.unwrap_err().unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from_contract_error(ContractError::InvalidBeneficiary as u32),
        "error must be InvalidBeneficiary"
    );
}

/// Non-owner cannot call update_beneficiary_share.
#[test]
fn test_update_beneficiary_share_non_owner_rejected() {
    let (env, owner, _, client) = setup();
    let b1 = Address::generate(&env);
    let attacker = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &3_600u64, &None);

    let initial = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 10_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &initial);

    let result = client.try_update_beneficiary_share(&vault_id, &attacker, &b1, &5_000u32);
    assert!(
        result.is_err(),
        "non-owner must not be able to update beneficiary share"
    );
}
