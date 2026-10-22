#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, token::StellarAssetClient, vec, Address, Env};

fn setup() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
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
    (env, owner, beneficiary, client)
}

#[test]
fn test_get_release_conditions_defaults_to_ttl_expiry() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    let conditions = client.get_release_conditions(&vault_id);
    assert_eq!(conditions, vec![&env, ReleaseCondition::TTLExpiry]);
}

#[test]
fn test_get_release_conditions_explicit_empty_is_preserved() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    client.set_release_conditions(&vault_id, &owner, &vec![&env]);

    assert_eq!(client.get_release_conditions(&vault_id), vec![&env]);
}

#[test]
fn test_get_release_conditions_single() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    let expected = vec![&env, ReleaseCondition::TTLExpiry];
    client.set_release_conditions(&vault_id, &owner, &expected);

    assert_eq!(client.get_release_conditions(&vault_id), expected);
}

#[test]
fn test_get_release_conditions_multiple() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let oracle = Address::generate(&env);

    let expected = vec![
        &env,
        ReleaseCondition::TTLExpiry,
        ReleaseCondition::OwnerInitiated,
        ReleaseCondition::Oracle(oracle),
    ];
    client.set_release_conditions(&vault_id, &owner, &expected);

    assert_eq!(client.get_release_conditions(&vault_id), expected);
}
