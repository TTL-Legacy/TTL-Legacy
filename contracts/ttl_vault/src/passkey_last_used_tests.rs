#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
};

use crate::tests::common::setup_funded as setup;

#[test]
fn test_last_used_none_before_first_use() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let passkey_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);

    client.add_passkey(&vault_id, &owner, &passkey_hash);

    assert_eq!(client.get_passkey_last_used(&vault_id, &passkey_hash), None);
}

#[test]
fn test_last_used_set_on_first_use() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let passkey_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
    client.add_passkey(&vault_id, &owner, &passkey_hash);

    let checkin_time = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(checkin_time);
    client.check_in(&vault_id, &owner, &passkey_hash, &0u64, &None, &None);

    assert_eq!(
        client.get_passkey_last_used(&vault_id, &passkey_hash),
        Some(checkin_time)
    );
}

#[test]
fn test_last_used_updates_on_subsequent_use() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let passkey_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
    client.add_passkey(&vault_id, &owner, &passkey_hash);

    let first_checkin = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(first_checkin);
    client.check_in(&vault_id, &owner, &passkey_hash, &0u64, &None, &None);

    let second_checkin = first_checkin + MIN_CHECK_IN_INTERVAL;
    env.ledger().set_timestamp(second_checkin);
    client.check_in(&vault_id, &owner, &passkey_hash, &0u64, &None, &None);

    assert_eq!(
        client.get_passkey_last_used(&vault_id, &passkey_hash),
        Some(second_checkin)
    );
}

#[test]
fn test_last_used_none_for_unregistered_passkey() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let unregistered = BytesN::<32>::from_array(&env, &[9u8; 32]);

    assert_eq!(client.get_passkey_last_used(&vault_id, &unregistered), None);
}
