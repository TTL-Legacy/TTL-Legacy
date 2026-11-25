#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

fn setup_clawback_env() -> (
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

/// Clawback within the grace period zeroes the beneficiary's BPS and emits the event.
#[test]
fn test_clawback_within_grace_period() {
    let (env, owner, _, client) = setup_clawback_env();
    let b1 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);
    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 10_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    // Mark released at ledger timestamp 1000
    env.ledger().with_mut(|li| li.timestamp = 1_000);
    client.mark_beneficiary_released(&vault_id, &owner, &b1);

    // Clawback well inside the 7-day window
    env.ledger().with_mut(|li| li.timestamp = 1_000 + 86_400); // +1 day
    let reclaimed = client.clawback_post_release(&vault_id, &owner, &b1);
    assert_eq!(reclaimed, 10_000u32, "all BPS should be reclaimed");

    let vault = client.get_vault(&vault_id);
    let bps = vault.beneficiaries.iter().find(|e| e.address == b1).unwrap().bps;
    assert_eq!(bps, 0u32, "BPS should be zeroed after clawback");
}

/// Clawback after the grace period expires is rejected.
#[test]
fn test_clawback_after_grace_period_rejected() {
    let (env, owner, _, client) = setup_clawback_env();
    let b1 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);
    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 10_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    env.ledger().with_mut(|li| li.timestamp = 1_000);
    client.mark_beneficiary_released(&vault_id, &owner, &b1);

    // Move past the 7-day grace period
    env.ledger().with_mut(|li| li.timestamp = 1_000 + GRACE_PERIOD_SECONDS + 1);
    let result = client.try_clawback_post_release(&vault_id, &owner, &b1);
    assert!(result.is_err(), "clawback after grace period should fail");
}

/// Clawback without a prior mark_beneficiary_released returns NotReleased.
#[test]
fn test_clawback_without_release_mark_rejected() {
    let (env, owner, _, client) = setup_clawback_env();
    let b1 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);
    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 10_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    let result = client.try_clawback_post_release(&vault_id, &owner, &b1);
    assert!(result.is_err(), "clawback without prior release mark should fail");
}

/// Only the vault owner can call clawback.
#[test]
fn test_clawback_rejects_non_owner() {
    let (env, owner, _, client) = setup_clawback_env();
    let b1 = Address::generate(&env);
    let impostor = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);
    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 10_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    env.ledger().with_mut(|li| li.timestamp = 1_000);
    client.mark_beneficiary_released(&vault_id, &owner, &b1);

    let result = client.try_clawback_post_release(&vault_id, &impostor, &b1);
    assert!(result.is_err());
}
