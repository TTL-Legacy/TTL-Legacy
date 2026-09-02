//! Tests for #952: Withdrawal Destination Whitelist
//!
//! Verifies:
//!  - add_whitelist_destination stores the entry
//!  - remove_whitelist_destination removes the entry
//!  - withdraw succeeds when owner is on whitelist
//!  - withdraw is rejected when owner is NOT on whitelist
//!  - withdraw always succeeds when no whitelist is set (empty = allow all)
//!  - Only vault owner can add/remove destinations
//!  - get_whitelisted_destinations returns the list

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::StellarAssetClient,
    Address, Env,
};

// ---------------------------------------------------------------------------
// Test setup helper
// ---------------------------------------------------------------------------

fn setup_whitelist() -> (
    Env,
    Address, // owner
    Address, // beneficiary
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

    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.deposit(&vault_id, &owner, &2_000_000);

    (env, owner, beneficiary, vault_id, client)
}

// ---------------------------------------------------------------------------
// add_whitelist_destination
// ---------------------------------------------------------------------------

#[test]
fn test_add_whitelist_destination_stores_entry() {
    let (_env, owner, _beneficiary, vault_id, client) = setup_whitelist();

    // Add owner itself as approved destination (withdraw always sends to owner)
    client.add_whitelist_destination(&vault_id, &owner, &owner);

    let list = client.get_whitelisted_destinations(&vault_id).unwrap();
    assert_eq!(list.len(), 1u32);
    assert_eq!(list.get(0u32).unwrap().address, owner);
}

#[test]
fn test_add_whitelist_destination_requires_owner() {
    let (env, _owner, beneficiary, vault_id, client) = setup_whitelist();

    let stranger = Address::generate(&env);
    let result = client.try_add_whitelist_destination(&vault_id, &beneficiary, &stranger);
    assert!(result.is_err());
}

#[test]
fn test_add_multiple_whitelist_destinations() {
    let (env, owner, _beneficiary, vault_id, client) = setup_whitelist();

    let dest_a = Address::generate(&env);
    let dest_b = Address::generate(&env);

    client.add_whitelist_destination(&vault_id, &owner, &dest_a);
    client.add_whitelist_destination(&vault_id, &owner, &dest_b);

    let list = client.get_whitelisted_destinations(&vault_id).unwrap();
    assert_eq!(list.len(), 2u32);
}

// ---------------------------------------------------------------------------
// remove_whitelist_destination
// ---------------------------------------------------------------------------

#[test]
fn test_remove_whitelist_destination_removes_entry() {
    let (env, owner, _beneficiary, vault_id, client) = setup_whitelist();

    let dest = Address::generate(&env);
    client.add_whitelist_destination(&vault_id, &owner, &dest);
    assert_eq!(
        client.get_whitelisted_destinations(&vault_id).unwrap().len(),
        1u32
    );

    client.remove_whitelist_destination(&vault_id, &owner, &dest);
    let list = client.get_whitelisted_destinations(&vault_id).unwrap();
    assert_eq!(list.len(), 0u32);
}

#[test]
fn test_remove_whitelist_destination_requires_owner() {
    let (env, owner, beneficiary, vault_id, client) = setup_whitelist();

    let dest = Address::generate(&env);
    client.add_whitelist_destination(&vault_id, &owner, &dest);

    let result = client.try_remove_whitelist_destination(&vault_id, &beneficiary, &dest);
    assert!(result.is_err());
}

#[test]
fn test_remove_nonexistent_entry_is_idempotent() {
    let (env, owner, _beneficiary, vault_id, client) = setup_whitelist();

    // Removing an address that was never added should not error
    let ghost = Address::generate(&env);
    let result = client.try_remove_whitelist_destination(&vault_id, &owner, &ghost);
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// withdraw enforcement
// ---------------------------------------------------------------------------

#[test]
fn test_withdraw_succeeds_when_no_whitelist_set() {
    let (_env, owner, _beneficiary, vault_id, client) = setup_whitelist();

    // No whitelist configured → withdraw must succeed (allow-all behaviour)
    let result = client.try_withdraw(&vault_id, &owner, &500_000, &None, &None, &None);
    assert!(result.is_ok());
}

#[test]
fn test_withdraw_succeeds_when_owner_is_whitelisted() {
    let (_env, owner, _beneficiary, vault_id, client) = setup_whitelist();

    // Add owner as approved destination (withdraw sends to owner)
    client.add_whitelist_destination(&vault_id, &owner, &owner);

    let result = client.try_withdraw(&vault_id, &owner, &500_000, &None, &None, &None);
    assert!(result.is_ok());
}

#[test]
fn test_withdraw_fails_when_owner_not_on_whitelist() {
    let (env, owner, _beneficiary, vault_id, client) = setup_whitelist();

    // Add a *different* address — owner is not on the list
    let other = Address::generate(&env);
    client.add_whitelist_destination(&vault_id, &owner, &other);

    let result = client.try_withdraw(&vault_id, &owner, &500_000, &None, &None, &None);
    assert!(result.is_err());
}

#[test]
fn test_withdraw_fails_after_owner_removed_from_whitelist() {
    let (_env, owner, _beneficiary, vault_id, client) = setup_whitelist();

    // Add then remove the owner
    client.add_whitelist_destination(&vault_id, &owner, &owner);
    client.remove_whitelist_destination(&vault_id, &owner, &owner);

    let result = client.try_withdraw(&vault_id, &owner, &500_000, &None, &None, &None);
    assert!(result.is_err());
}

#[test]
fn test_withdraw_succeeds_after_owner_re_added_to_whitelist() {
    let (_env, owner, _beneficiary, vault_id, client) = setup_whitelist();

    // Add, remove, then re-add
    client.add_whitelist_destination(&vault_id, &owner, &owner);
    client.remove_whitelist_destination(&vault_id, &owner, &owner);
    client.add_whitelist_destination(&vault_id, &owner, &owner);

    let result = client.try_withdraw(&vault_id, &owner, &500_000, &None, &None, &None);
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// get_whitelisted_destinations
// ---------------------------------------------------------------------------

#[test]
fn test_get_whitelisted_destinations_returns_none_when_empty() {
    let (_env, _owner, _beneficiary, vault_id, client) = setup_whitelist();
    // No entries added yet
    assert!(client.get_whitelisted_destinations(&vault_id).is_none());
}

#[test]
fn test_whitelist_is_per_vault() {
    let (env, owner, beneficiary, vault_id_1, client) = setup_whitelist();

    // Create a second vault
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.deposit(&vault_id_2, &owner, &1_000_000);

    let dest = Address::generate(&env);
    // Add destination only to vault 1
    client.add_whitelist_destination(&vault_id_1, &owner, &dest);

    // Vault 2 should have no whitelist
    assert!(client.get_whitelisted_destinations(&vault_id_2).is_none());

    // Vault 1 should have the entry
    assert_eq!(
        client
            .get_whitelisted_destinations(&vault_id_1)
            .unwrap()
            .len(),
        1u32
    );
}
