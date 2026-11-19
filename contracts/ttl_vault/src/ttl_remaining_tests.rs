//! Tests for TTL recalculation across ledger advances (Issue #1269)

#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
};

use crate::tests::common::setup_funded as setup;

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

#[test]
fn test_ttl_is_frozen_during_hibernation_then_resumes() {
    let (env, owner, beneficiary, client) = setup();
    let interval = MIN_CHECK_IN_INTERVAL;
    let hibernation_duration = 10_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    env.ledger().set_timestamp(env.ledger().timestamp() + interval / 4);
    let ttl_at_entry = client.get_ttl_remaining(&vault_id).unwrap();
    client.enter_hibernation(&vault_id, &owner, &hibernation_duration);

    env.ledger()
        .set_timestamp(env.ledger().timestamp() + hibernation_duration / 2);
    assert_eq!(client.get_ttl_remaining(&vault_id), Some(ttl_at_entry));

    env.ledger()
        .set_timestamp(env.ledger().timestamp() + hibernation_duration / 2);
    assert_eq!(client.get_ttl_remaining(&vault_id), Some(ttl_at_entry));

    env.ledger().set_timestamp(env.ledger().timestamp() + 1);
    assert_eq!(client.get_ttl_remaining(&vault_id), Some(ttl_at_entry - 1));
}
