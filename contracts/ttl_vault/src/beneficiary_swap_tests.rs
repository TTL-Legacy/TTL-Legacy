#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::StellarAssetClient,
    vec, Address, Env,
};

fn setup_swap_env() -> (
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

/// After a swap the BPS values are exchanged atomically.
#[test]
fn test_swap_allocations_success() {
    let (env, owner, _, client) = setup_swap_env();

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);

    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 7000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 3000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    client.swap_allocations(&vault_id, &owner, &b1, &b2);

    let vault = client.get_vault(&vault_id);
    let bps_b1 = vault.beneficiaries.iter().find(|e| e.address == b1).unwrap().bps;
    let bps_b2 = vault.beneficiaries.iter().find(|e| e.address == b2).unwrap().bps;
    assert_eq!(bps_b1, 3000, "b1 should now have b2's old BPS");
    assert_eq!(bps_b2, 7000, "b2 should now have b1's old BPS");
}

/// Swap is symmetric — swapping twice restores the original values.
#[test]
fn test_swap_allocations_idempotent_double_swap() {
    let (env, owner, _, client) = setup_swap_env();

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);

    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 6000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 4000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    client.swap_allocations(&vault_id, &owner, &b1, &b2);
    client.swap_allocations(&vault_id, &owner, &b1, &b2);

    let vault = client.get_vault(&vault_id);
    let bps_b1 = vault.beneficiaries.iter().find(|e| e.address == b1).unwrap().bps;
    let bps_b2 = vault.beneficiaries.iter().find(|e| e.address == b2).unwrap().bps;
    assert_eq!(bps_b1, 6000);
    assert_eq!(bps_b2, 4000);
}

/// Swap with an unregistered address returns an error.
#[test]
fn test_swap_allocations_rejects_unknown_address() {
    let (env, owner, _, client) = setup_swap_env();

    let b1 = Address::generate(&env);
    let stranger = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);

    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 10_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    let result = client.try_swap_allocations(&vault_id, &owner, &b1, &stranger);
    assert!(result.is_err());
}

/// Only the vault owner can swap allocations.
#[test]
fn test_swap_allocations_rejects_non_owner() {
    let (env, owner, _, client) = setup_swap_env();

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);
    let impostor = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);

    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 5000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 5000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    let result = client.try_swap_allocations(&vault_id, &impostor, &b1, &b2);
    assert!(result.is_err());
}
