#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

fn setup_clone_comprehensive_env() -> (
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

    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    (env, owner, beneficiary, admin, source_vault_id, client)
}

// ========== Test: clone_vault_creates_independent_vault_instances ==========

#[test]
fn test_clone_vault_creates_independent_vault_instances() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_comprehensive_env();
    let new_beneficiary = Address::generate(&env);

    let cloned_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    let source_vault = client.get_vault(&source_vault_id);
    let cloned_vault = client.get_vault(&cloned_vault_id);

    // Both vaults should exist independently
    assert_eq!(source_vault.owner, owner);
    assert_eq!(cloned_vault.owner, owner);
    assert_ne!(source_vault_id, cloned_vault_id);
}

// ========== Test: clone_vault_copies_all_settings_except_balance ==========

#[test]
fn test_clone_vault_copies_all_settings_except_balance() {
    let (env, owner, beneficiary, _, _, client) = setup_clone_comprehensive_env();

    // Create source vault with specific settings
    let ttl_interval = 432_000u64; // 5 days
    let source_vault_id = client.create_vault(&owner, &beneficiary, &ttl_interval, &None);
    let metadata = "comprehensive-test-vault";
    client.update_metadata(&source_vault_id, &owner, &metadata.to_string());

    // Deposit funds into source vault
    let deposit_amount = 5_000_000i128;
    client.deposit(&source_vault_id, &owner, &deposit_amount);

    let new_beneficiary = Address::generate(&env);

    // Clone without overrides (inherit all settings)
    let cloned_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    let source_vault = client.get_vault(&source_vault_id);
    let cloned_vault = client.get_vault(&cloned_vault_id);

    // Settings should be copied
    assert_eq!(cloned_vault.check_in_interval, source_vault.check_in_interval);
    assert_eq!(cloned_vault.metadata, source_vault.metadata);

    // Balance should NOT be copied
    assert_eq!(source_vault.balance, deposit_amount);
    assert_eq!(cloned_vault.balance, 0);

    // Beneficiary should be the new one
    assert_eq!(cloned_vault.beneficiary, new_beneficiary);
}

// ========== Test: clone_vault_allows_custom_beneficiary_override ==========

#[test]
fn test_clone_vault_allows_custom_beneficiary_override() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_comprehensive_env();
    let custom_beneficiary = Address::generate(&env);

    let cloned_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &custom_beneficiary, &None, &None, &None);

    let cloned_vault = client.get_vault(&cloned_vault_id);

    assert_eq!(cloned_vault.beneficiary, custom_beneficiary);
}

// ========== Test: clone_vault_with_interval_override ==========

#[test]
fn test_clone_vault_with_interval_override() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_comprehensive_env();
    let new_beneficiary = Address::generate(&env);
    let override_interval = 600u64;

    // Source vault has interval 100
    let source_vault = client.get_vault(&source_vault_id);
    assert_eq!(source_vault.check_in_interval, 100u64);

    // Clone with override interval
    let cloned_vault_id = client.clone_vault_with_overrides(
        &source_vault_id,
        &owner,
        &new_beneficiary,
        &Some(override_interval),
        &None,
        &None,
    );

    let cloned_vault = client.get_vault(&cloned_vault_id);

    // Cloned vault should have the override interval, not the source's
    assert_eq!(cloned_vault.check_in_interval, override_interval);
}

// ========== Test: clone_vault_with_metadata_override ==========

#[test]
fn test_clone_vault_with_metadata_override() {
    let (env, owner, beneficiary, _, _, client) = setup_clone_comprehensive_env();

    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.update_metadata(&source_vault_id, &owner, &"source-metadata".to_string());

    let new_beneficiary = Address::generate(&env);
    let override_metadata = "cloned-vault-metadata";

    let cloned_vault_id = client.clone_vault_with_overrides(
        &source_vault_id,
        &owner,
        &new_beneficiary,
        &None,
        &None,
        &Some(override_metadata.to_string()),
    );

    let cloned_vault = client.get_vault(&cloned_vault_id);

    // Cloned vault should have the override metadata
    assert_eq!(cloned_vault.metadata, override_metadata);
}

// ========== Test: clone_vault_preserves_owner ==========

#[test]
fn test_clone_vault_preserves_owner() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_comprehensive_env();
    let new_beneficiary = Address::generate(&env);

    let cloned_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    let source_vault = client.get_vault(&source_vault_id);
    let cloned_vault = client.get_vault(&cloned_vault_id);

    // Both should have the same owner
    assert_eq!(cloned_vault.owner, source_vault.owner);
    assert_eq!(cloned_vault.owner, owner);
}

// ========== Test: clone_vault_new_vault_is_independent_for_operations ==========

#[test]
fn test_clone_vault_new_vault_is_independent_for_operations() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_comprehensive_env();
    let new_beneficiary = Address::generate(&env);

    let cloned_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    // Deposit into source vault
    let deposit_amount_source = 1_000_000i128;
    client.deposit(&source_vault_id, &owner, &deposit_amount_source);

    // Deposit into cloned vault
    let deposit_amount_cloned = 2_000_000i128;
    client.deposit(&cloned_vault_id, &owner, &deposit_amount_cloned);

    let source_vault = client.get_vault(&source_vault_id);
    let cloned_vault = client.get_vault(&cloned_vault_id);

    // Deposits should be independent
    assert_eq!(source_vault.balance, deposit_amount_source);
    assert_eq!(cloned_vault.balance, deposit_amount_cloned);
}

// ========== Test: clone_vault_with_all_overrides ==========

#[test]
fn test_clone_vault_with_all_overrides() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_comprehensive_env();

    let override_beneficiary = Address::generate(&env);
    let override_interval = 259_200u64;
    let override_metadata = "fully-overridden";

    let cloned_vault_id = client.clone_vault_with_overrides(
        &source_vault_id,
        &owner,
        &override_beneficiary,
        &Some(override_interval),
        &None,
        &Some(override_metadata.to_string()),
    );

    let cloned_vault = client.get_vault(&cloned_vault_id);

    // All overrides should be applied
    assert_eq!(cloned_vault.beneficiary, override_beneficiary);
    assert_eq!(cloned_vault.check_in_interval, override_interval);
    assert_eq!(cloned_vault.metadata, override_metadata);
}

// ========== Test: clone_vault_release_status_independent ==========

#[test]
fn test_clone_vault_release_status_independent() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_comprehensive_env();
    let new_beneficiary = Address::generate(&env);

    let source_vault = client.get_vault(&source_vault_id);
    let initial_status = source_vault.status.clone();

    let cloned_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    let cloned_vault = client.get_vault(&cloned_vault_id);

    // Release status should be copied from source
    assert_eq!(cloned_vault.status, initial_status);
}

// ========== Test: multiple_clones_from_single_source ==========

#[test]
fn test_multiple_clones_from_single_source() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_comprehensive_env();

    let beneficiary_1 = Address::generate(&env);
    let beneficiary_2 = Address::generate(&env);
    let beneficiary_3 = Address::generate(&env);

    let clone_1 = client.clone_vault_with_overrides(&source_vault_id, &owner, &beneficiary_1, &None, &None, &None);
    let clone_2 = client.clone_vault_with_overrides(&source_vault_id, &owner, &beneficiary_2, &None, &None, &None);
    let clone_3 = client.clone_vault_with_overrides(&source_vault_id, &owner, &beneficiary_3, &None, &None, &None);

    let vault_1 = client.get_vault(&clone_1);
    let vault_2 = client.get_vault(&clone_2);
    let vault_3 = client.get_vault(&clone_3);

    // All clones should be independent with different beneficiaries
    assert_eq!(vault_1.beneficiary, beneficiary_1);
    assert_eq!(vault_2.beneficiary, beneficiary_2);
    assert_eq!(vault_3.beneficiary, beneficiary_3);
    assert_ne!(clone_1, clone_2);
    assert_ne!(clone_2, clone_3);
}

// ========== Test: clone_vault_metadata_empty_string_override ==========

#[test]
fn test_clone_vault_metadata_empty_string_override() {
    let (env, owner, beneficiary, _, _, client) = setup_clone_comprehensive_env();

    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.update_metadata(&source_vault_id, &owner, &"has-metadata".to_string());

    let new_beneficiary = Address::generate(&env);
    let empty_metadata = String::new();

    let cloned_vault_id = client.clone_vault_with_overrides(
        &source_vault_id,
        &owner,
        &new_beneficiary,
        &None,
        &None,
        &Some(empty_metadata),
    );

    let cloned_vault = client.get_vault(&cloned_vault_id);

    // Metadata should be empty string (override applied)
    assert_eq!(cloned_vault.metadata, String::new());
}
