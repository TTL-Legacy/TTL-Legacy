//! Tests for Issue #1338: vault export/import for disaster recovery.
//!
//! These tests verify:
//!   1. `export_vault_config` returns a config that faithfully reflects the
//!      current vault state.
//!   2. `import_vault` recreates a vault with the same configuration, assigning
//!      a new vault ID.
//!   3. The full roundtrip (export → import) reproduces the original vault.
//!   4. Access controls: only the owner can export; caller must match owner on
//!      import.
//!   5. Edge cases: multi-beneficiary splits, custom metadata, spending limits.

#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Bytes, Env, String,
};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn setup() -> (
    Env,
    Address, // owner
    Address, // beneficiary
    Address, // admin
    Address, // token_address
    TtlVaultContractClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    // Mint some XLM to the owner so deposit tests work
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_id = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_id);
    client.initialize(&token_address, &admin);

    // Safety transmute for lifetime — standard pattern in this test suite
    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    (env, owner, beneficiary, admin, token_address, client)
}

// ─── basic export ─────────────────────────────────────────────────────────────

/// `export_vault_config` returns a config whose fields match the stored vault.
#[test]
fn test_export_vault_config_fields() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let interval = 7_200u64; // 2 hours

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Advance time a little so exported_at ≠ 0
    env.ledger().with_mut(|l| l.timestamp = 500);

    let config = client.export_vault_config(&vault_id, &owner);

    assert_eq!(config.original_vault_id, vault_id);
    assert_eq!(config.owner, owner);
    assert_eq!(config.beneficiary, beneficiary);
    assert_eq!(config.check_in_interval, interval);
    assert_eq!(config.exported_at, 500);
    assert!(config.spending_limit.is_none());
    assert!(config.max_deposit_amount.is_none());
    assert_eq!(config.beneficiaries.len(), 0, "no multi-beneficiary split");
}

// ─── access control ───────────────────────────────────────────────────────────

/// A non-owner cannot export the vault configuration.
#[test]
#[should_panic]
fn test_export_requires_owner() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let attacker = Address::generate(&env);
    client.export_vault_config(&vault_id, &attacker);
}

// ─── basic import ─────────────────────────────────────────────────────────────

/// After export + import the new vault has the same config as the original.
#[test]
fn test_import_vault_recreates_config() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let interval = 3_600u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    env.ledger().with_mut(|l| l.timestamp = 100);

    let config = client.export_vault_config(&vault_id, &owner);

    // Simulate disaster: time passes, original vault would be archived/expired.
    env.ledger().with_mut(|l| l.timestamp = 200);

    let new_vault_id = client.import_vault(&config, &owner);

    // A new vault ID is assigned.
    assert_ne!(new_vault_id, vault_id, "import should create a new vault");

    let new_vault = client
        .get_vault(&new_vault_id)
        .expect("new vault should exist");
    assert_eq!(new_vault.owner, owner);
    assert_eq!(new_vault.beneficiary, beneficiary);
    assert_eq!(new_vault.check_in_interval, interval);
    assert_eq!(new_vault.balance, 0, "balance starts at zero");
    assert_eq!(new_vault.status, ReleaseStatus::Locked);
}

// ─── full roundtrip ───────────────────────────────────────────────────────────

/// Full roundtrip: create → deposit → export → import → deposit again →
/// verify balance on new vault.
#[test]
fn test_full_roundtrip_export_import() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let interval = 3_600u64;
    let deposit = 500_000i128;

    // 1. Create & fund original vault
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&vault_id, &owner, &deposit);
    assert_eq!(client.get_vault(&vault_id).unwrap().balance, deposit);

    // 2. Export config (owner only)
    env.ledger().with_mut(|l| l.timestamp = 100);
    let config = client.export_vault_config(&vault_id, &owner);
    assert_eq!(config.original_vault_id, vault_id);

    // 3. Import into a new vault
    env.ledger().with_mut(|l| l.timestamp = 200);
    let new_id = client.import_vault(&config, &owner);
    assert_ne!(new_id, vault_id);

    // 4. New vault starts empty; re-deposit the same amount
    client.deposit(&new_id, &owner, &deposit);
    let new_vault = client.get_vault(&new_id).unwrap();
    assert_eq!(new_vault.balance, deposit);
    assert_eq!(new_vault.check_in_interval, interval);
}

// ─── multi-beneficiary roundtrip ─────────────────────────────────────────────

/// Multi-beneficiary BPS split is preserved through export/import.
#[test]
fn test_export_import_preserves_multi_beneficiary_split() {
    let (env, owner, b1, _admin, _token, client) = setup();
    let b2 = Address::generate(&env);
    let interval = 3_600u64;

    let vault_id = client.create_vault(&owner, &b1, &interval, &None);
    client.set_beneficiaries(
        &vault_id,
        &owner,
        &vec![
            &env,
            BeneficiaryEntry {
                address: b1.clone(),
                bps: 6_000,
                minimum_threshold: 0,
            },
            BeneficiaryEntry {
                address: b2.clone(),
                bps: 4_000,
                minimum_threshold: 0,
            },
        ],
    );

    let config = client.export_vault_config(&vault_id, &owner);
    assert_eq!(config.beneficiaries.len(), 2);

    let new_id = client.import_vault(&config, &owner);
    let new_vault = client.get_vault(&new_id).unwrap();
    assert_eq!(new_vault.beneficiaries.len(), 2);
    assert_eq!(
        new_vault.beneficiaries.iter().map(|e| e.bps).sum::<u32>(),
        10_000
    );
}

// ─── custom metadata roundtrip ────────────────────────────────────────────────

/// Custom metadata bytes are faithfully preserved through export/import.
#[test]
fn test_export_import_preserves_custom_metadata() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Update custom metadata on the original vault
    let meta_bytes = Bytes::from_slice(&env, b"ipfs://QmDisasterRecovery");
    client.set_custom_vault_metadata(&vault_id, &owner, &meta_bytes);

    let config = client.export_vault_config(&vault_id, &owner);
    let new_id = client.import_vault(&config, &owner);

    let new_vault = client.get_vault(&new_id).unwrap();
    assert_eq!(new_vault.custom_metadata, meta_bytes);
}

// ─── spending limit roundtrip ─────────────────────────────────────────────────

/// Spending limit is preserved through export/import.
#[test]
fn test_export_import_preserves_spending_limit() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let limit = 100_000i128;
    client.set_spending_limit(&vault_id, &owner, &limit);

    let config = client.export_vault_config(&vault_id, &owner);
    assert_eq!(config.spending_limit, Some(limit));

    let new_id = client.import_vault(&config, &owner);
    let new_vault = client.get_vault(&new_id).unwrap();
    assert_eq!(new_vault.spending_limit, Some(limit));
}

// ─── import access control ────────────────────────────────────────────────────

/// `import_vault` panics if the caller is not the config owner.
#[test]
#[should_panic]
fn test_import_requires_owner() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    let config = client.export_vault_config(&vault_id, &owner);

    let attacker = Address::generate(&env);
    client.import_vault(&config, &attacker);
}

// ─── export emits event ───────────────────────────────────────────────────────

/// `export_vault_config` does not panic and returns a config — event emission
/// is verified indirectly (Soroban env doesn't expose event assertions easily,
/// but the call itself must succeed without error).
#[test]
fn test_export_does_not_panic() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    env.ledger().with_mut(|l| l.timestamp = 42);

    // If this panics the test fails automatically.
    let config = client.export_vault_config(&vault_id, &owner);
    assert_eq!(config.exported_at, 42);
}
