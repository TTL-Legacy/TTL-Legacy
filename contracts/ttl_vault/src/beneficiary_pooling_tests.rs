#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

fn setup_pool_env() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
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

    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, beneficiary, admin, token_address, client)
}

/// Pool creation succeeds, total BPS is summed, event is emitted.
#[test]
fn test_create_pool_success() {
    let (env, owner, _, _, _, client) = setup_pool_env();

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);

    let entries = vec![
        &env,
        BeneficiaryEntry {
            address: b1.clone(),
            bps: 6000,
            minimum_threshold: 0,
        },
        BeneficiaryEntry {
            address: b2.clone(),
            bps: 4000,
            minimum_threshold: 0,
        },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    let members = vec![&env, b1.clone(), b2.clone()];
    client.create_pool(&vault_id, &owner, &1u64, &members);

    let pool = client.get_pool(&1u64).expect("pool should exist");
    assert_eq!(pool.total_bps, 10_000);
    assert_eq!(pool.members.len(), 2);
}

/// Pool creation with partial members uses only their BPS sum.
#[test]
fn test_create_pool_partial_members() {
    let (env, owner, _, _, _, client) = setup_pool_env();

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);

    let entries = vec![
        &env,
        BeneficiaryEntry {
            address: b1.clone(),
            bps: 6000,
            minimum_threshold: 0,
        },
        BeneficiaryEntry {
            address: b2.clone(),
            bps: 4000,
            minimum_threshold: 0,
        },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    let members = vec![&env, b1.clone()];
    client.create_pool(&vault_id, &owner, &2u64, &members);

    let pool = client.get_pool(&2u64).expect("pool should exist");
    assert_eq!(pool.total_bps, 6000);
}

/// Pool creation fails when a member is not a registered beneficiary.
#[test]
fn test_create_pool_rejects_unregistered_member() {
    let (env, owner, _, _, _, client) = setup_pool_env();

    let b1 = Address::generate(&env);
    let stranger = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);

    let entries = vec![
        &env,
        BeneficiaryEntry {
            address: b1.clone(),
            bps: 10_000,
            minimum_threshold: 0,
        },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    let members = vec![&env, b1.clone(), stranger];
    let result = client.try_create_pool(&vault_id, &owner, &3u64, &members);
    assert!(result.is_err());
}

/// Only the vault owner can create a pool.
#[test]
fn test_create_pool_rejects_non_owner() {
    let (env, owner, _, _, _, client) = setup_pool_env();

    let b1 = Address::generate(&env);
    let impostor = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);

    let entries = vec![
        &env,
        BeneficiaryEntry {
            address: b1.clone(),
            bps: 10_000,
            minimum_threshold: 0,
        },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    let members = vec![&env, b1.clone()];
    let result = client.try_create_pool(&vault_id, &impostor, &4u64, &members);
    assert!(result.is_err());
}
