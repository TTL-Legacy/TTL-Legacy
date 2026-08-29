#![cfg(test)]

//! Regression tests for issue #1275: `withdraw` must only be callable by the
//! vault owner. These guard the `caller != vault.owner` check in `withdraw`
//! against regressing back to allowing any address to drain vault funds.

use super::*;
use soroban_sdk::{testutils::Address as _, token::StellarAssetClient, Address, Env};

fn setup() -> (Env, Address, Address, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let attacker = Address::generate(&env);
    let admin = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract_v2(token_admin).address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, beneficiary, attacker, client)
}

/// A non-owner address must not be able to withdraw funds from a vault, even
/// though `mock_all_auths` makes `require_auth` succeed for any caller — the
/// contract-level owner check is what must reject the attempt.
#[test]
fn test_withdraw_rejected_for_non_owner() {
    let (_env, owner, beneficiary, attacker, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);
    client.deposit(&vault_id, &owner, &500_000i128);

    let result = client.try_withdraw(&vault_id, &attacker, &100_000i128);
    assert!(result.is_err(), "withdraw should reject a non-owner caller");
    match result.unwrap_err().unwrap() {
        ContractError::NotOwner => {}
        e => panic!("Expected NotOwner, got {:?}", e),
    }

    // Balance must be untouched by the rejected attempt.
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, 500_000);
}

/// The beneficiary is also not the owner and must not be able to withdraw
/// via `withdraw` (that path is reserved for `trigger_release`/`partial_release`).
#[test]
fn test_withdraw_rejected_for_beneficiary() {
    let (_env, owner, beneficiary, _attacker, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);
    client.deposit(&vault_id, &owner, &500_000i128);

    let result = client.try_withdraw(&vault_id, &beneficiary, &100_000i128);
    assert!(result.is_err());
    match result.unwrap_err().unwrap() {
        ContractError::NotOwner => {}
        e => panic!("Expected NotOwner, got {:?}", e),
    }
}

/// The actual owner can withdraw successfully.
#[test]
fn test_withdraw_succeeds_for_owner() {
    let (_env, owner, beneficiary, _attacker, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);
    client.deposit(&vault_id, &owner, &500_000i128);

    let result = client.try_withdraw(&vault_id, &owner, &100_000i128);
    assert!(result.is_ok());

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, 400_000);
}
