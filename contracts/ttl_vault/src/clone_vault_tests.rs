#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

fn setup_clone_vault_env() -> (
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

// ========== Test: clone_vault_with_inherited_settings_copies_ttl_and_interval ==========

#[test]
fn test_clone_vault_with_inherited_settings_copies_ttl_and_interval() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_vault_env();
    let new_beneficiary = Address::generate(&env);

    // Create a source vault with specific check_in_interval
    let ttl_interval = 500u64;
    let source_with_interval = client.create_vault(&owner, &beneficiary, &ttl_interval, &None);

    // Clone with inherit_settings=true (via clone_vault_with_overrides with None overrides)
    let new_vault_id =
        client.clone_vault_with_overrides(&source_with_interval, &owner, &new_beneficiary, &None, &None, &None);

    // Load the new vault
    let new_vault = client.get_vault(&new_vault_id);

    // Assert that check_in_interval was copied
    assert_eq!(new_vault.check_in_interval, ttl_interval);
    // Assert that beneficiary is the new one
    assert_eq!(new_vault.beneficiary, new_beneficiary);
    // Assert that owner is the source owner
    assert_eq!(new_vault.owner, owner);
}

// ========== Test: clone_vault_with_inherited_settings_copies_metadata ==========

#[test]
fn test_clone_vault_with_inherited_settings_copies_metadata() {
    let (env, owner, beneficiary, _, _, client) = setup_clone_vault_env();
    let new_beneficiary = Address::generate(&env);
    let metadata = "test-metadata";

    // Create source vault with metadata
    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.update_metadata(&source_vault_id, &owner, &metadata.to_string());

    // Clone with inherit_settings=true (None overrides means inherit)
    let new_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    let new_vault = client.get_vault(&new_vault_id);

    // Assert metadata was copied
    assert_eq!(new_vault.metadata, metadata.to_string());
}

// ========== Test: clone_vault_without_inherited_settings_uses_defaults ==========

#[test]
fn test_clone_vault_without_inherited_settings_uses_defaults() {
    let (env, owner, beneficiary, _, _, client) = setup_clone_vault_env();
    let new_beneficiary = Address::generate(&env);

    // Create source vault with non-default settings
    let source_vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);
    let metadata = "source-metadata";
    client.update_metadata(&source_vault_id, &owner, &metadata.to_string());

    // Clone with override_interval=Some(default)
    // For this test, we use clone_vault_with_overrides to override with new defaults
    let new_interval = 100u64;
    let new_metadata = String::new();
    let new_vault_id = client.clone_vault_with_overrides(
        &source_vault_id,
        &owner,
        &new_beneficiary,
        &Some(new_interval),
        &None,
        &Some(new_metadata.clone()),
    );

    let new_vault = client.get_vault(&new_vault_id);

    // Assert that check_in_interval is the override (not the source)
    assert_eq!(new_vault.check_in_interval, new_interval);
    // Assert that metadata is the override (not the source)
    assert_eq!(new_vault.metadata, new_metadata);
}

// ========== Test: clone_vault_does_not_copy_balance ==========

#[test]
fn test_clone_vault_does_not_copy_balance() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_vault_env();
    let new_beneficiary = Address::generate(&env);

    // Deposit funds into source vault
    let deposit_amount = 1_000_000i128;
    client.deposit(&source_vault_id, &owner, &deposit_amount);

    // Verify source vault has the balance
    let source_vault = client.get_vault(&source_vault_id);
    assert_eq!(source_vault.balance, deposit_amount);

    // Clone the vault
    let new_vault_id = client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    // Verify new vault balance is 0
    let new_vault = client.get_vault(&new_vault_id);
    assert_eq!(new_vault.balance, 0);
}

// ========== Test: clone_vault_emits_vault_cloned_event ==========

#[test]
fn test_clone_vault_emits_vault_cloned_event() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_vault_env();
    let new_beneficiary = Address::generate(&env);

    let new_vault_id = client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    // Check for vault cloned event
    let events = env.events().all();
    let vault_cloned_event = events.iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        if topics.is_empty() {
            return false;
        }
        let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
        topic0
            .map(|s| s == types::VAULT_CLONED_TOPIC)
            .unwrap_or(false)
    });

    assert!(vault_cloned_event.is_some(), "vault_cloned event not emitted");
}

// ========== Test: clone_vault_requires_owner_auth ==========

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_clone_vault_requires_owner_auth() {
    let env = Env::default();
    // Do NOT mock auth for this test — only set auth for the imposter
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let admin = Address::generate(&env);
    let imposter = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    // Mock auth only for the owner during vault creation
    env.mock_all_auths();
    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    // Clear mock auths and only set auth for the imposter
    env.set_auth(soroban_sdk::testutils::MockAuth::new(vec![
        &env,
        soroban_sdk::testutils::MockAuthorizationEntry {
            contract: &contract_address,
            fn_name: soroban_sdk::symbol_short!("clone_vault_with_overrides"),
            args: (&source_vault_id, &imposter, &Address::generate(&env), &None::<u64>, &None::<soroban_sdk::Vec<BeneficiaryEntry>>, &None::<String>).into_val(&env),
            invoke_contract: true,
        },
    ]));

    let new_beneficiary = Address::generate(&env);
    // This should fail because imposter != owner
    client.clone_vault_with_overrides(&source_vault_id, &imposter, &new_beneficiary, &None, &None, &None);
}

// ========== Test: clone_vault_fails_when_source_not_found ==========

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_clone_vault_fails_when_source_not_found() {
    let (env, owner, _beneficiary, _, _, client) = setup_clone_vault_env();
    let new_beneficiary = Address::generate(&env);

    // Try to clone a vault that doesn't exist
    client.clone_vault_with_overrides(&9999u64, &owner, &new_beneficiary, &None, &None, &None);
}

// ========== Test: clone_vault_returns_new_unique_id ==========

#[test]
fn test_clone_vault_returns_new_unique_id() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_vault_env();
    let new_beneficiary1 = Address::generate(&env);
    let new_beneficiary2 = Address::generate(&env);

    // Clone the same source vault twice
    let cloned_id_1 =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary1, &None, &None, &None);
    let cloned_id_2 =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary2, &None, &None, &None);

    // Assert both returned IDs are different
    assert_ne!(cloned_id_1, cloned_id_2);

    // Assert both new vaults exist independently
    assert!(client.vault_exists(&cloned_id_1));
    assert!(client.vault_exists(&cloned_id_2));

    // Assert they have different beneficiaries
    assert_eq!(client.get_vault(&cloned_id_1).beneficiary, new_beneficiary1);
    assert_eq!(client.get_vault(&cloned_id_2).beneficiary, new_beneficiary2);
}

// ========== Test: clone_vault_with_override_interval ==========

#[test]
fn test_clone_vault_with_override_interval() {
    let (env, owner, beneficiary, _, _, client) = setup_clone_vault_env();
    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let new_beneficiary = Address::generate(&env);
    let override_interval = 500u64;

    let new_vault_id = client.clone_vault_with_overrides(
        &source_vault_id,
        &owner,
        &new_beneficiary,
        &Some(override_interval),
        &None,
        &None,
    );

    let new_vault = client.get_vault(&new_vault_id);
    assert_eq!(new_vault.check_in_interval, override_interval);
}

// ========== Test: clone_vault_rejects_zero_interval ==========

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_clone_vault_rejects_zero_interval() {
    let (env, owner, beneficiary, _, _, client) = setup_clone_vault_env();
    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let new_beneficiary = Address::generate(&env);

    // Try to override with zero interval
    client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &Some(0u64), &None, &None);
}

// ========== Test: clone_vault_with_override_beneficiaries_replaces_split ==========

#[test]
fn test_clone_vault_with_override_beneficiaries_replaces_split() {
    let (env, owner, beneficiary, _, _, client) = setup_clone_vault_env();
    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let new_beneficiary = Address::generate(&env);

    // Create override beneficiaries with 50-50 split
    let ben1 = Address::generate(&env);
    let ben2 = Address::generate(&env);
    let override_beneficiaries = vec![
        &env,
        BeneficiaryEntry {
            address: ben1.clone(),
            bps: 5000,
        },
        BeneficiaryEntry {
            address: ben2.clone(),
            bps: 5000,
        },
    ];

    let new_vault_id = client.clone_vault_with_overrides(
        &source_vault_id,
        &owner,
        &new_beneficiary,
        &None,
        &Some(override_beneficiaries.clone()),
        &None,
    );

    let new_vault = client.get_vault(&new_vault_id);

    // Assert beneficiaries were replaced
    assert_eq!(new_vault.beneficiaries.len(), 2);
    assert_eq!(new_vault.beneficiaries.get(0).unwrap().bps, 5000);
    assert_eq!(new_vault.beneficiaries.get(1).unwrap().bps, 5000);
}

// ========== Test: clone_vault_rejects_invalid_bps_sum ==========

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_clone_vault_rejects_invalid_bps_sum() {
    let (env, owner, beneficiary, _, _, client) = setup_clone_vault_env();
    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let new_beneficiary = Address::generate(&env);

    // Create override beneficiaries with invalid BPS sum (not 10,000)
    let ben1 = Address::generate(&env);
    let ben2 = Address::generate(&env);
    let override_beneficiaries = vec![
        &env,
        BeneficiaryEntry {
            address: ben1.clone(),
            bps: 5000,
        },
        BeneficiaryEntry {
            address: ben2.clone(),
            bps: 4000, // Sum = 9000, not 10000
        },
    ];

    client.clone_vault_with_overrides(
        &source_vault_id,
        &owner,
        &new_beneficiary,
        &None,
        &Some(override_beneficiaries),
        &None,
    );
}

// ========== Test: clone_vault_rejects_owner_in_beneficiaries ==========

#[test]
#[should_panic(expected = "Error(Contract, #17)")]
fn test_clone_vault_rejects_owner_in_beneficiaries() {
    let (env, owner, beneficiary, _, _, client) = setup_clone_vault_env();
    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let new_beneficiary = Address::generate(&env);

    // Create override beneficiaries where one is the owner
    let override_beneficiaries = vec![
        &env,
        BeneficiaryEntry {
            address: owner.clone(),
            bps: 5000,
        },
        BeneficiaryEntry {
            address: new_beneficiary.clone(),
            bps: 5000,
        },
    ];

    client.clone_vault_with_overrides(
        &source_vault_id,
        &owner,
        &new_beneficiary,
        &None,
        &Some(override_beneficiaries),
        &None,
    );
}

// ========== Test: clone_vault_with_override_metadata ==========

#[test]
fn test_clone_vault_with_override_metadata() {
    let (env, owner, beneficiary, _, _, client) = setup_clone_vault_env();
    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let new_beneficiary = Address::generate(&env);
    let source_metadata = "source-metadata";
    let override_metadata = "override-metadata";

    client.update_metadata(&source_vault_id, &owner, &source_metadata.to_string());

    let new_vault_id = client.clone_vault_with_overrides(
        &source_vault_id,
        &owner,
        &new_beneficiary,
        &None,
        &None,
        &Some(override_metadata.to_string()),
    );

    let new_vault = client.get_vault(&new_vault_id);

    // Assert metadata is the override, not the source
    assert_eq!(new_vault.metadata, override_metadata.to_string());
}

// ========== Test: clone_vault_inherits_release_condition ==========

#[test]
fn test_clone_vault_inherits_release_condition() {
    let (env, owner, beneficiary, _, _, client) = setup_clone_vault_env();
    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let new_beneficiary = Address::generate(&env);

    // Verify the cloned vault has the same release_condition as source
    let source_vault = client.get_vault(&source_vault_id);
    let cloned_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);
    let cloned_vault = client.get_vault(&cloned_vault_id);

    assert_eq!(cloned_vault.release_condition, source_vault.release_condition);
}

// ========== Test: clone_vault_parent_vault_id_set_correctly ==========

#[test]
fn test_clone_vault_parent_vault_id_set_correctly() {
    let (env, owner, beneficiary, _, _, client) = setup_clone_vault_env();
    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let new_beneficiary = Address::generate(&env);

    let cloned_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    let cloned_vault = client.get_vault(&cloned_vault_id);

    // Assert parent_vault_id is set to source_vault_id
    assert_eq!(cloned_vault.parent_vault_id, Some(source_vault_id));
}

// ========== Test: clone_vault_new_vault_has_locked_status ==========

#[test]
fn test_clone_vault_new_vault_has_locked_status() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_vault_env();
    let new_beneficiary = Address::generate(&env);

    let cloned_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    let cloned_vault = client.get_vault(&cloned_vault_id);

    // Assert new vault status is Locked
    assert_eq!(cloned_vault.status, ReleaseStatus::Locked);
}

// ========== Test: clone_vault_new_vault_last_check_in_reset ==========

#[test]
fn test_clone_vault_new_vault_last_check_in_reset() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_vault_env();
    let new_beneficiary = Address::generate(&env);

    // Advance time and check in source vault
    env.ledger().with_mut(|l| l.timestamp += 50);
    client.check_in(&source_vault_id, &owner);

    let source_vault = client.get_vault(&source_vault_id);
    let source_last_check_in = source_vault.last_check_in;

    // Advance more and clone
    env.ledger().with_mut(|l| l.timestamp += 50);
    let current_timestamp = env.ledger().timestamp();

    let cloned_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    let cloned_vault = client.get_vault(&cloned_vault_id);

    // Assert new vault's last_check_in is set to current time (not copied from source)
    assert_eq!(cloned_vault.last_check_in, current_timestamp as u64);
    assert_ne!(cloned_vault.last_check_in, source_last_check_in);
}

// ========== Test: clone_vault_new_vault_not_paused ==========

#[test]
fn test_clone_vault_new_vault_not_paused() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_vault_env();
    let new_beneficiary = Address::generate(&env);

    let cloned_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    let cloned_vault = client.get_vault(&cloned_vault_id);

    // Assert new vault is not paused
    assert!(!cloned_vault.is_paused);
}

// ========== Test: clone_vault_adds_to_owner_index ==========

#[test]
fn test_clone_vault_adds_to_owner_index() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_vault_env();
    let new_beneficiary = Address::generate(&env);

    // Get owner's vaults before cloning
    let owner_vaults_before = client.get_vaults_by_owner(&owner, &None, &0u32, &100u32);

    let cloned_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    // Get owner's vaults after cloning
    let owner_vaults_after = client.get_vaults_by_owner(&owner, &None, &0u32, &100u32);

    // Assert the cloned vault is in the owner's list
    assert_eq!(owner_vaults_after.len(), owner_vaults_before.len() + 1);
    assert!(owner_vaults_after.contains(&cloned_vault_id));
}

// ========== Test: clone_vault_adds_to_beneficiary_index ==========

#[test]
fn test_clone_vault_adds_to_beneficiary_index() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_vault_env();
    let new_beneficiary = Address::generate(&env);

    // Get new_beneficiary's vaults before cloning (should be empty)
    let ben_vaults_before = client.get_vaults_by_beneficiary(&new_beneficiary, &None, &0u32, &100u32);
    assert_eq!(ben_vaults_before.len(), 0);

    let cloned_vault_id =
        client.clone_vault_with_overrides(&source_vault_id, &owner, &new_beneficiary, &None, &None, &None);

    // Get new_beneficiary's vaults after cloning
    let ben_vaults_after = client.get_vaults_by_beneficiary(&new_beneficiary, &None, &0u32, &100u32);

    // Assert the cloned vault is in the new_beneficiary's list
    assert_eq!(ben_vaults_after.len(), 1);
    assert_eq!(ben_vaults_after.get(0).unwrap(), cloned_vault_id);
}

// ========== Test: clone_vault_basic_function (non-override variant) ==========

#[test]
fn test_clone_vault_basic_function() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_vault_env();
    let new_beneficiary = Address::generate(&env);

    // Call clone_vault (the basic variant without overrides)
    let new_vault_id = client.clone_vault(&source_vault_id, &owner, &new_beneficiary);

    // Verify vault exists
    assert!(client.vault_exists(&new_vault_id));
    let new_vault = client.get_vault(&new_vault_id);

    // Verify key fields
    assert_eq!(new_vault.owner, owner);
    assert_eq!(new_vault.beneficiary, new_beneficiary);
    assert_eq!(new_vault.balance, 0);
    assert_eq!(new_vault.status, ReleaseStatus::Locked);
}

// ========== Test: clone_vault_basic_function_rejects_non_owner ==========

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_clone_vault_basic_function_rejects_non_owner() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let admin = Address::generate(&env);
    let non_owner = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    env.mock_all_auths();
    let source_vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    // Try to clone as non-owner (should fail with NotOwner)
    client.clone_vault(&source_vault_id, &non_owner, &Address::generate(&env));
}

// ========== Test: clone_vault_basic_function_rejects_released_vault ==========

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_clone_vault_basic_function_rejects_released_vault() {
    let (env, owner, beneficiary, _, source_vault_id, client) = setup_clone_vault_env();
    let new_beneficiary = Address::generate(&env);

    // Advance time past expiry to trigger release
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&source_vault_id);

    // Try to clone released vault (should fail with AlreadyReleased)
    client.clone_vault(&source_vault_id, &owner, &new_beneficiary);
}
