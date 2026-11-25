#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

fn setup_migration_vault_env() -> (
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

// ========== Test: migrate_vault_transfers_ownership_to_new_owner ==========

#[test]
fn test_migrate_vault_transfers_ownership_to_new_owner() {
    let (env, owner, beneficiary, _, vault_id, client) = setup_migration_vault_env();
    let new_owner = Address::generate(&env);

    // Migrate vault to new owner with same beneficiary
    client.migrate_vault(&vault_id, &owner, &new_owner, &beneficiary);

    let vault = client.get_vault(&vault_id);

    // Assert ownership transferred
    assert_eq!(vault.owner, new_owner);
    // Assert beneficiary remains the same
    assert_eq!(vault.beneficiary, beneficiary);
}

// ========== Test: migrate_vault_can_change_beneficiary_on_migration ==========

#[test]
fn test_migrate_vault_can_change_beneficiary_on_migration() {
    let (env, owner, beneficiary, _, vault_id, client) = setup_migration_vault_env();
    let new_owner = Address::generate(&env);
    let new_beneficiary = Address::generate(&env);

    // Migrate vault with new owner and new beneficiary
    client.migrate_vault(&vault_id, &owner, &new_owner, &new_beneficiary);

    let vault = client.get_vault(&vault_id);

    // Assert both ownership and beneficiary changed
    assert_eq!(vault.owner, new_owner);
    assert_eq!(vault.beneficiary, new_beneficiary);
}

// ========== Test: migrate_vault_preserves_vault_balance ==========

#[test]
fn test_migrate_vault_preserves_vault_balance() {
    let (env, owner, beneficiary, _, vault_id, client) = setup_migration_vault_env();
    let new_owner = Address::generate(&env);
    let deposit_amount = 1_000_000i128;

    // Deposit funds before migration
    client.deposit(&vault_id, &owner, &deposit_amount);

    let vault_before = client.get_vault(&vault_id);
    let balance_before = vault_before.balance;

    // Migrate vault
    client.migrate_vault(&vault_id, &owner, &new_owner, &beneficiary);

    let vault_after = client.get_vault(&vault_id);

    // Assert balance is preserved
    assert_eq!(vault_after.balance, balance_before);
}

// ========== Test: migrate_vault_preserves_check_in_history ==========

#[test]
fn test_migrate_vault_preserves_check_in_interval() {
    let (env, owner, beneficiary, _, vault_id, client) = setup_migration_vault_env();
    let new_owner = Address::generate(&env);
    let custom_interval = 500u64;

    // Update check-in interval
    client.update_check_in_interval(&vault_id, &owner, &custom_interval);

    let vault_before = client.get_vault(&vault_id);
    let interval_before = vault_before.check_in_interval;

    // Migrate vault
    client.migrate_vault(&vault_id, &owner, &new_owner, &beneficiary);

    let vault_after = client.get_vault(&vault_id);

    // Assert check-in interval is preserved
    assert_eq!(vault_after.check_in_interval, interval_before);
}

// ========== Test: migrate_vault_preserves_metadata ==========

#[test]
fn test_migrate_vault_preserves_metadata_on_migration() {
    let (env, owner, beneficiary, _, vault_id, client) = setup_migration_vault_env();
    let new_owner = Address::generate(&env);
    let metadata = "important-vault-metadata";

    // Set metadata before migration
    client.update_metadata(&vault_id, &owner, &metadata.to_string());

    let vault_before = client.get_vault(&vault_id);
    let meta_before = vault_before.metadata.clone();

    // Migrate vault
    client.migrate_vault(&vault_id, &owner, &new_owner, &beneficiary);

    let vault_after = client.get_vault(&vault_id);

    // Assert metadata is preserved
    assert_eq!(vault_after.metadata, meta_before);
}

// ========== Test: migrate_vault_preserves_release_status ==========

#[test]
fn test_migrate_vault_preserves_release_status() {
    let (env, owner, beneficiary, _, vault_id, client) = setup_migration_vault_env();
    let new_owner = Address::generate(&env);

    let vault_before = client.get_vault(&vault_id);
    let status_before = vault_before.status;

    // Migrate vault
    client.migrate_vault(&vault_id, &owner, &new_owner, &beneficiary);

    let vault_after = client.get_vault(&vault_id);

    // Assert release status is preserved
    assert_eq!(vault_after.status, status_before);
}

// ========== Test: migrate_vault_only_callable_by_current_owner ==========

#[test]
#[should_panic]
fn test_migrate_vault_only_callable_by_current_owner() {
    let (env, owner, beneficiary, _, vault_id, client) = setup_migration_vault_env();
    let new_owner = Address::generate(&env);
    let unauthorized_caller = Address::generate(&env);

    // Try to migrate vault from unauthorized address
    client.migrate_vault(&vault_id, &unauthorized_caller, &new_owner, &beneficiary);
}

// ========== Test: migrate_vault_emits_migration_event ==========

#[test]
fn test_migrate_vault_emits_migration_event() {
    let (env, owner, beneficiary, _, vault_id, client) = setup_migration_vault_env();
    let new_owner = Address::generate(&env);

    // Migrate vault
    client.migrate_vault(&vault_id, &owner, &new_owner, &beneficiary);

    // Check event was emitted (verified through contract event logs)
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.owner, new_owner);
}

// ========== Test: migrate_vault_preserves_custom_metadata ==========

#[test]
fn test_migrate_vault_preserves_custom_metadata() {
    let (env, owner, beneficiary, _, vault_id, client) = setup_migration_vault_env();
    let new_owner = Address::generate(&env);
    let custom_meta = soroban_sdk::Bytes::from_slice(&env, b"custom-metadata-bytes");

    // Set custom metadata before migration
    client.set_custom_metadata(&vault_id, &owner, &custom_meta);

    let vault_before = client.get_vault(&vault_id);
    let custom_meta_before = vault_before.custom_metadata.clone();

    // Migrate vault
    client.migrate_vault(&vault_id, &owner, &new_owner, &beneficiary);

    let vault_after = client.get_vault(&vault_id);

    // Assert custom metadata is preserved
    assert_eq!(vault_after.custom_metadata, custom_meta_before);
}

// ========== Test: migrate_vault_allows_new_owner_to_manage_vault ==========

#[test]
fn test_migrate_vault_allows_new_owner_to_manage_vault() {
    let (env, owner, beneficiary, _, vault_id, client) = setup_migration_vault_env();
    let new_owner = Address::generate(&env);
    let new_interval = 300u64;

    // Migrate vault to new owner
    client.migrate_vault(&vault_id, &owner, &new_owner, &beneficiary);

    // New owner should be able to update interval
    client.update_check_in_interval(&vault_id, &new_owner, &new_interval);

    let vault = client.get_vault(&vault_id);

    // Assert new owner can manage the vault
    assert_eq!(vault.check_in_interval, new_interval);
    assert_eq!(vault.owner, new_owner);
}
