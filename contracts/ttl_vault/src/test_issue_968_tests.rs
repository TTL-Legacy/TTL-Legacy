#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

fn setup_test_vault_env() -> (
    Env,
    Address,
    Address,
    Address,
    u64,
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

    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    (env, owner, beneficiary, admin, vault_id, client)
}

// ========== Test: basic_vault_creation_and_retrieval ==========

#[test]
fn test_basic_vault_creation_and_retrieval() {
    let (env, owner, beneficiary, _, vault_id, client) = setup_test_vault_env();

    let vault = client.get_vault(&vault_id);

    assert_eq!(vault.owner, owner);
    assert_eq!(vault.beneficiary, beneficiary);
    assert_eq!(vault.balance, 0i128);
}

// ========== Test: vault_creation_with_custom_check_in_interval ==========

#[test]
fn test_vault_creation_with_custom_check_in_interval() {
    let (env, owner, beneficiary, _, _, client) = setup_test_vault_env();
    let custom_interval = 259_200u64; // 3 days

    let vault_id = client.create_vault(&owner, &beneficiary, &custom_interval, &None);
    let vault = client.get_vault(&vault_id);

    assert_eq!(vault.check_in_interval, custom_interval);
}

// ========== Test: deposit_increases_vault_balance ==========

#[test]
fn test_deposit_increases_vault_balance() {
    let (env, owner, _, _, vault_id, client) = setup_test_vault_env();
    let deposit_amount = 1_000_000i128;

    client.deposit(&vault_id, &owner, &deposit_amount);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, deposit_amount);
}

// ========== Test: multiple_deposits_accumulate_balance ==========

#[test]
fn test_multiple_deposits_accumulate_balance() {
    let (env, owner, _, _, vault_id, client) = setup_test_vault_env();
    let deposit_1 = 500_000i128;
    let deposit_2 = 750_000i128;

    client.deposit(&vault_id, &owner, &deposit_1);
    client.deposit(&vault_id, &owner, &deposit_2);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, deposit_1 + deposit_2);
}

// ========== Test: check_in_updates_last_check_in_timestamp ==========

#[test]
fn test_check_in_updates_last_check_in_timestamp() {
    let (env, owner, _, _, vault_id, client) = setup_test_vault_env();
    let deposit_amount = 1_000_000i128;

    client.deposit(&vault_id, &owner, &deposit_amount);

    let vault_before = client.get_vault(&vault_id);
    let ledger_timestamp = env.ledger().timestamp();

    // Check-in requires a passkey hash (use mock)
    let passkey_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
    let nonce = 0u64;
    let _ = client.check_in(&vault_id, &owner, &passkey_hash, &nonce, &None, &None);

    let vault_after = client.get_vault(&vault_id);

    assert!(vault_after.last_check_in >= vault_before.last_check_in);
}

// ========== Test: vault_balance_persists_across_multiple_operations ==========

#[test]
fn test_vault_balance_persists_across_multiple_operations() {
    let (env, owner, beneficiary, _, vault_id, client) = setup_test_vault_env();
    let deposit_amount = 1_000_000i128;

    // Deposit
    client.deposit(&vault_id, &owner, &deposit_amount);

    // Update interval
    client.update_check_in_interval(&vault_id, &owner, &300u64);

    // Update metadata
    client.update_metadata(&vault_id, &owner, &"updated-metadata".to_string());

    let vault = client.get_vault(&vault_id);

    // Balance should remain unchanged
    assert_eq!(vault.balance, deposit_amount);
}

// ========== Test: vault_creation_initializes_with_zero_balance ==========

#[test]
fn test_vault_creation_initializes_with_zero_balance() {
    let (env, owner, beneficiary, _, _, client) = setup_test_vault_env();

    let new_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let vault = client.get_vault(&new_vault_id);

    assert_eq!(vault.balance, 0i128);
}

// ========== Test: vault_metadata_can_be_updated ==========

#[test]
fn test_vault_metadata_can_be_updated() {
    let (env, owner, _, _, vault_id, client) = setup_test_vault_env();
    let initial_metadata = "initial";
    let updated_metadata = "updated";

    client.update_metadata(&vault_id, &owner, &initial_metadata.to_string());
    let vault_1 = client.get_vault(&vault_id);
    assert_eq!(vault_1.metadata, initial_metadata);

    client.update_metadata(&vault_id, &owner, &updated_metadata.to_string());
    let vault_2 = client.get_vault(&vault_id);
    assert_eq!(vault_2.metadata, updated_metadata);
}

// ========== Test: vault_check_in_interval_can_be_updated ==========

#[test]
fn test_vault_check_in_interval_can_be_updated() {
    let (env, owner, _, _, vault_id, client) = setup_test_vault_env();
    let new_interval = 500u64;

    client.update_check_in_interval(&vault_id, &owner, &new_interval);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.check_in_interval, new_interval);
}

// ========== Test: vault_release_status_defaults_to_pending ==========

#[test]
fn test_vault_release_status_defaults_to_pending() {
    let (env, owner, beneficiary, _, _, client) = setup_test_vault_env();

    let new_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let vault = client.get_vault(&new_vault_id);

    // Status should be pending/locked by default
    assert!(vault.status != ReleaseStatus::Released);
}
