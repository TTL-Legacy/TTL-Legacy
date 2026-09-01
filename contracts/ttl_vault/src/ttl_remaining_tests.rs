//! Tests for TTL recalculation across ledger advances (Issue #1269)

#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, BytesN, Env,
};

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
fn test_ttl_decrements_across_ledger_advances() {
    let (env, owner, beneficiary, client) = setup();
    let interval = MIN_CHECK_IN_INTERVAL;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let start = client.get_ttl_remaining(&vault_id).unwrap();
    assert_eq!(start, interval);

    let step = interval / 4;
    let mut expected = start;
    for _ in 0..3 {
        env.ledger().set_timestamp(env.ledger().timestamp() + step);
        expected -= step;
        assert_eq!(client.get_ttl_remaining(&vault_id), Some(expected));
    }
}

#[test]
fn test_ttl_none_after_deadline() {
    let (env, owner, beneficiary, client) = setup();
    let interval = MIN_CHECK_IN_INTERVAL;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    env.ledger().set_timestamp(env.ledger().timestamp() + interval);
    assert_eq!(client.get_ttl_remaining(&vault_id), None);
}

#[test]
fn test_ttl_resets_after_check_in() {
    let (env, owner, beneficiary, client) = setup();
    let interval = MIN_CHECK_IN_INTERVAL;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    env.ledger().set_timestamp(env.ledger().timestamp() + interval / 2);
    assert_eq!(client.get_ttl_remaining(&vault_id), Some(interval / 2));

    let passkey_hash = BytesN::<32>::from_array(&env, &[7u8; 32]);
    client.add_passkey(&vault_id, &owner, &passkey_hash);
    client.check_in(&vault_id, &owner, &passkey_hash, &0u64, &None, &None);

    assert_eq!(client.get_ttl_remaining(&vault_id), Some(interval));
}
