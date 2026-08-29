#![cfg(test)]

//! Regression tests for issue #1277: `get_vault` must return
//! `ContractError::VaultNotFound` for an unknown vault ID instead of
//! panicking with no structured error context.

use super::*;
use soroban_sdk::{testutils::Address as _, token::StellarAssetClient, Address, Env};

fn setup() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let admin = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract_v2(token_admin).address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, beneficiary, client)
}

/// Looking up a vault ID that was never created must return
/// `ContractError::VaultNotFound` rather than panicking.
#[test]
fn test_get_vault_nonexistent_returns_error() {
    let (_env, _owner, _beneficiary, client) = setup();

    let result = client.try_get_vault(&999u64);
    assert!(result.is_err(), "get_vault should not panic on an invalid vault ID");
    match result.unwrap_err().unwrap() {
        ContractError::VaultNotFound => {}
        e => panic!("Expected VaultNotFound, got {:?}", e),
    }
}

/// A vault ID that does exist is returned normally.
#[test]
fn test_get_vault_existing_succeeds() {
    let (_env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);

    let result = client.try_get_vault(&vault_id);
    assert!(result.is_ok());
    let vault = result.unwrap().unwrap();
    assert_eq!(vault.owner, owner);
    assert_eq!(vault.beneficiary, beneficiary);
}

/// Vault IDs beyond the current count also resolve to `VaultNotFound`.
#[test]
fn test_get_vault_id_past_count_returns_error() {
    let (_env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);

    let result = client.try_get_vault(&(vault_id + 1));
    assert!(result.is_err());
    match result.unwrap_err().unwrap() {
        ContractError::VaultNotFound => {}
        e => panic!("Expected VaultNotFound, got {:?}", e),
    }
}
