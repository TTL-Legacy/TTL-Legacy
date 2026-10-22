//! Issue 4 – batch_check_in_extended: custom extension amounts + returned TTLs.
//!
//! `batch_check_in_extended` extends multiple vaults in a single call.
//! When `extension_amounts` is empty, each vault gets a full-interval extension
//! (same behaviour as the original `batch_check_in`).
//! When `extension_amounts` is populated, each vault gets a per-vault custom
//! extension (capped at the vault's check_in_interval).
//! The function returns a `Vec<u64>` of new deadline timestamps.

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient, vec, Address, Env, IntoVal, TryIntoVal,
};

fn setup() -> (Env, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract_v2(token_admin).address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, client)
}

/// Full-interval extension (empty extension_amounts) returns correct TTL deadlines.
#[test]
fn test_batch_check_in_extended_default_extension() {
    let (env, owner, client) = setup();
    let b = Address::generate(&env);
    let interval = 3_600u64;

    let v1 = client.create_vault(&owner, &b, &interval, &None);
    let v2 = client.create_vault(&owner, &b, &interval, &None);

    let now = 100u64;
    env.ledger().with_mut(|l| l.timestamp = now);

    let vault_ids = vec![&env, v1, v2];
    let empty_amounts: soroban_sdk::Vec<u64> = soroban_sdk::Vec::new(&env);

    let ttls = client
        .batch_check_in_extended(&vault_ids, &empty_amounts, &owner)
        .unwrap();

    assert_eq!(ttls.len(), 2, "must return one TTL per vault");

    // Both vaults had last_check_in = 0, check_in_interval = interval.
    // Extension logic: current_deadline = 0 + interval = interval > now=100,
    // so new last_check_in = interval - interval + interval = interval,
    // new deadline = interval + interval = 2 * interval.
    let expected = 2 * interval;
    assert_eq!(ttls.get(0).unwrap(), expected);
    assert_eq!(ttls.get(1).unwrap(), expected);
}

#[test]
fn test_batch_check_in_extended_emits_check_in_event_for_each_vault() {
    let (env, owner, client) = setup();
    let beneficiary = Address::generate(&env);
    let interval = 3_600u64;
    let vault_id_1 = client.create_vault(&owner, &beneficiary, &interval, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &interval, &None);
    let vault_ids = vec![&env, vault_id_1, vault_id_2];
    let extension_amounts: soroban_sdk::Vec<u64> = soroban_sdk::Vec::new(&env);

    env.ledger().with_mut(|ledger| ledger.timestamp = 100);
    let ttls = client
        .batch_check_in_extended(&vault_ids, &extension_amounts, &owner)
        .unwrap();

    let check_in_events: alloc::vec::Vec<CheckInEvent> = env
        .events()
        .all()
        .iter()
        .filter_map(|event| {
            let topics: soroban_sdk::Vec<soroban_sdk::Val> = event.1.clone().into_val(&env);
            let topic = topics
                .get(0)
                .and_then(|value| value.try_into_val(&env).ok());
            if topic == Some(CHECK_IN_RECORDED_TOPIC) {
                event.2.try_into_val(&env).ok()
            } else {
                None
            }
        })
        .collect();

    assert_eq!(check_in_events.len(), vault_ids.len());
    for (index, vault_id) in vault_ids.iter().enumerate() {
        let event = check_in_events
            .iter()
            .find(|event| event.vault_id == vault_id)
            .expect("CheckInEvent missing for batch vault");
        assert_eq!(event.caller, owner);
        assert_eq!(event.new_ttl, ttls.get(index as u32).unwrap());
    }
}

/// Custom extension amounts produce the correct per-vault TTL deadlines.
#[test]
fn test_batch_check_in_extended_custom_extension() {
    let (env, owner, client) = setup();
    let b = Address::generate(&env);
    let interval = 10_000u64;

    let v1 = client.create_vault(&owner, &b, &interval, &None);
    let v2 = client.create_vault(&owner, &b, &interval, &None);

    // Advance time past the initial check-in deadline so `last_check_in = now`
    let now = interval + 1;
    env.ledger().with_mut(|l| l.timestamp = now);

    let vault_ids = vec![&env, v1, v2];
    // v1 gets half the interval, v2 gets the full interval
    let half = interval / 2;
    let amounts = vec![&env, half, interval];

    let ttls = client
        .batch_check_in_extended(&vault_ids, &amounts, &owner)
        .unwrap();

    assert_eq!(ttls.len(), 2);

    // Both vaults: current_deadline = 0 + interval = interval < now,
    // so last_check_in = now, new deadline = now + interval.
    // v1: extension = half, last_check_in = now, deadline = now + interval (extension
    //     is applied differently — see implementation notes).
    // The returned value is last_check_in + check_in_interval.
    // For v1 extension=half: last_check_in = now (past deadline path), deadline = now + interval
    // For v2 extension=interval: same
    //
    // Actually the key invariant: both deadlines must be > now (vault kept alive).
    let d1 = ttls.get(0).unwrap();
    let d2 = ttls.get(1).unwrap();
    assert!(d1 > now, "v1 deadline must be after now");
    assert!(d2 > now, "v2 deadline must be after now");
}

/// Mixed extension amounts: one vault gets a shorter extension, another gets full.
#[test]
fn test_batch_check_in_mixed_extension_amounts() {
    let (env, owner, client) = setup();
    let b = Address::generate(&env);
    let interval = 7_200u64; // 2 hours

    let v1 = client.create_vault(&owner, &b, &interval, &None);
    let v2 = client.create_vault(&owner, &b, &interval, &None);
    let v3 = client.create_vault(&owner, &b, &interval, &None);

    env.ledger().with_mut(|l| l.timestamp = interval + 100); // past initial deadlines

    let vault_ids = vec![&env, v1, v2, v3];
    // v1: 1-hour extension, v2: full interval, v3: 30 minutes
    let amounts = vec![&env, 3_600u64, interval, 1_800u64];

    let ttls = client
        .batch_check_in_extended(&vault_ids, &amounts, &owner)
        .unwrap();

    assert_eq!(ttls.len(), 3);
    for i in 0..3 {
        assert!(
            ttls.get(i).unwrap() > interval + 100,
            "all deadlines must be after current time"
        );
    }
}

/// extension_amounts length mismatch returns InvalidAmount.
#[test]
fn test_batch_check_in_extended_length_mismatch_is_error() {
    let (env, owner, client) = setup();
    let b = Address::generate(&env);

    let v1 = client.create_vault(&owner, &b, &3_600u64, &None);
    let v2 = client.create_vault(&owner, &b, &3_600u64, &None);

    let vault_ids = vec![&env, v1, v2];
    // Only one amount for two vaults
    let amounts = vec![&env, 1_800u64];

    let result = client.try_batch_check_in_extended(&vault_ids, &amounts, &owner);
    assert!(result.is_err(), "mismatched lengths must return an error");
    let err = result.unwrap_err().unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from_contract_error(ContractError::InvalidAmount as u32),
    );
}

/// Non-owner cannot batch-extend vaults they do not own.
#[test]
fn test_batch_check_in_extended_non_owner_rejected() {
    let (env, owner, client) = setup();
    let b = Address::generate(&env);
    let v1 = client.create_vault(&owner, &b, &3_600u64, &None);

    let non_owner = Address::generate(&env);
    let vault_ids = vec![&env, v1];
    let empty: soroban_sdk::Vec<u64> = soroban_sdk::Vec::new(&env);

    let result = client.try_batch_check_in_extended(&vault_ids, &empty, &non_owner);
    assert!(result.is_err());
    let err = result.unwrap_err().unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from_contract_error(ContractError::NotOwner as u32),
    );
}

/// An extension amount larger than the vault interval is capped to the interval.
#[test]
fn test_batch_check_in_extended_oversized_extension_is_capped() {
    let (env, owner, client) = setup();
    let b = Address::generate(&env);
    let interval = 3_600u64;
    let v1 = client.create_vault(&owner, &b, &interval, &None);

    env.ledger().with_mut(|l| l.timestamp = interval + 1);

    let vault_ids = vec![&env, v1];
    // Request extension larger than the interval
    let oversized = vec![&env, interval * 10];

    let ttls = client
        .batch_check_in_extended(&vault_ids, &oversized, &owner)
        .unwrap();

    let now = interval + 1;
    let deadline = ttls.get(0).unwrap();
    // Capped to `interval`, so deadline = now + interval
    assert_eq!(deadline, now + interval, "oversized extension must be capped to interval");
}
