#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{storage::{Instance as _, Persistent as _}, Address as _, Events, Ledger},
    token::{self, StellarAssetClient},
    vec, Address, Env, IntoVal, TryIntoVal,
};

fn setup() -> (
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

fn withdrawal(env: &Env, vault_id: u64, destination: &Address, amount: i128) -> BatchWithdrawal {
    BatchWithdrawal {
        vault_id,
        destination: destination.clone(),
        amount,
    }
}

#[test]
fn test_batch_withdraw_single_destination() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let dest = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    client.batch_withdraw(
        &vec![&env, withdrawal(&env, vault_id, &dest, 400i128)],
        &owner,
    );

    assert_eq!(client.get_vault(&vault_id).balance, 600i128);
}

#[test]
fn test_batch_withdraw_multiple_destinations() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let dest1 = Address::generate(&env);
    let dest2 = Address::generate(&env);
    let dest3 = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    client.batch_withdraw(
        &vec![
            &env,
            withdrawal(&env, vault_id, &dest1, 100i128),
            withdrawal(&env, vault_id, &dest2, 200i128),
            withdrawal(&env, vault_id, &dest3, 300i128),
        ],
        &owner,
    );

    assert_eq!(client.get_vault(&vault_id).balance, 400i128);
}

#[test]
fn test_batch_withdraw_validates_total_against_vault_balance() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let dest = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &500i128);

    // Two instructions whose summed amount (600) exceeds the vault balance (500).
    let err = client
        .try_batch_withdraw(
            &vec![
                &env,
                withdrawal(&env, vault_id, &dest, 400i128),
                withdrawal(&env, vault_id, &dest, 200i128),
            ],
            &owner,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::InsufficientBalance);

    // Vault balance must be unchanged when the batch is rejected.
    assert_eq!(client.get_vault(&vault_id).balance, 500i128);
}

#[test]
fn test_batch_withdraw_owner_only() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let non_owner = Address::generate(&env);
    let dest = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    let err = client
        .try_batch_withdraw(
            &vec![&env, withdrawal(&env, vault_id, &dest, 100i128)],
            &non_owner,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::NotOwner);
}

#[test]
fn test_batch_withdraw_empty_list_rejected() {
    let (env, owner, _, _, _, client) = setup();

    let err = client
        .try_batch_withdraw(&vec![&env], &owner)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::InvalidAmount);
}

#[test]
fn test_batch_withdraw_zero_amount_rejected() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let dest = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    let err = client
        .try_batch_withdraw(
            &vec![&env, withdrawal(&env, vault_id, &dest, 0i128)],
            &owner,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::InvalidAmount);
}

#[test]
fn test_batch_withdraw_is_atomic() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let dest = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &500i128);

    // First instruction is valid; second exceeds the balance. The whole batch must fail.
    assert!(client
        .try_batch_withdraw(
            &vec![
                &env,
                withdrawal(&env, vault_id, &dest, 100i128),
                withdrawal(&env, vault_id, &dest, 1_000i128),
            ],
            &owner,
        )
        .is_err());

    assert_eq!(client.get_vault(&vault_id).balance, 500i128);
}

#[test]
fn test_batch_withdraw_reduces_balance() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);
    let dest = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);
    let before = token_client.balance(&dest);

    client.batch_withdraw(
        &vec![
            &env,
            withdrawal(&env, vault_id, &dest, 250i128),
            withdrawal(&env, vault_id, &dest, 250i128),
        ],
        &owner,
    );

    assert_eq!(client.get_vault(&vault_id).balance, 500i128);
    assert_eq!(token_client.balance(&dest), before + 500i128);
}
