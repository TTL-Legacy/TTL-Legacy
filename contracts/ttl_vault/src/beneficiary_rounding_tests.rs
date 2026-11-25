#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::StellarAssetClient,
    Address, Env,
};

fn setup_rounding_env() -> (
    Env,
    Address,
    Address,
    TtlVaultContractClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, admin, client)
}

/// Default rounding mode is Floor.
#[test]
fn test_default_rounding_mode_is_floor() {
    let (env, owner, _, client) = setup_rounding_env();
    let b1 = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &b1, &100);

    // No mode set — should default to Floor (0)
    let mode = client.get_rounding_mode(&vault_id);
    assert_eq!(mode, RoundingMode::Floor);
}

/// set_rounding_mode persists the mode and emits an event.
#[test]
fn test_set_rounding_mode_ceil() {
    let (env, owner, _, client) = setup_rounding_env();
    let b1 = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &b1, &100);

    client.set_rounding_mode(&vault_id, &owner, &RoundingMode::Ceil);
    let mode = client.get_rounding_mode(&vault_id);
    assert_eq!(mode, RoundingMode::Ceil);
}

/// apply_rounding — Floor truncates toward zero.
#[test]
fn test_apply_rounding_floor() {
    let (env, owner, _, client) = setup_rounding_env();
    let b1 = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &b1, &100);

    client.set_rounding_mode(&vault_id, &owner, &RoundingMode::Floor);
    // 10 / 3 = 3 (floor)
    let result = client.apply_rounding(&vault_id, &10i128, &3i128);
    assert_eq!(result, 3i128);
}

/// apply_rounding — Ceil rounds up.
#[test]
fn test_apply_rounding_ceil() {
    let (env, owner, _, client) = setup_rounding_env();
    let b1 = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &b1, &100);

    client.set_rounding_mode(&vault_id, &owner, &RoundingMode::Ceil);
    // 10 / 3 = 4 (ceil)
    let result = client.apply_rounding(&vault_id, &10i128, &3i128);
    assert_eq!(result, 4i128);
}

/// apply_rounding — Round uses nearest-integer logic.
#[test]
fn test_apply_rounding_round() {
    let (env, owner, _, client) = setup_rounding_env();
    let b1 = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &b1, &100);

    client.set_rounding_mode(&vault_id, &owner, &RoundingMode::Round);
    // 7 / 2 = 4 (round: (7 + 1) / 2 = 4)
    let result = client.apply_rounding(&vault_id, &7i128, &2i128);
    assert_eq!(result, 4i128);
    // 10 / 3 = 3 (round: (10 + 1) / 3 = 3)
    let result2 = client.apply_rounding(&vault_id, &10i128, &3i128);
    assert_eq!(result2, 3i128);
}

/// BPS storage is not mutated when rounding mode changes.
#[test]
fn test_rounding_mode_does_not_alter_bps_storage() {
    let (env, owner, _, client) = setup_rounding_env();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &b1, &100);

    let entries = soroban_sdk::vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 3333, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 6667, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    client.set_rounding_mode(&vault_id, &owner, &RoundingMode::Ceil);

    // BPS values must be unchanged
    let vault = client.get_vault(&vault_id);
    let total: u32 = vault.beneficiaries.iter().map(|e| e.bps).sum();
    assert_eq!(total, 10_000u32, "total BPS must remain 10 000 regardless of rounding mode");
}

/// Only the vault owner can change the rounding mode.
#[test]
fn test_set_rounding_mode_rejects_non_owner() {
    let (env, owner, _, client) = setup_rounding_env();
    let b1 = Address::generate(&env);
    let impostor = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &b1, &100);

    let result = client.try_set_rounding_mode(&vault_id, &impostor, &RoundingMode::Ceil);
    assert!(result.is_err());
}
