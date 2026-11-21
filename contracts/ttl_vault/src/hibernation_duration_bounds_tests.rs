#![cfg(test)]

//! Issue #1280: `enter_hibernation` accepted `duration_seconds = 0` (instant
//! no-op) or arbitrarily large values (effectively a permanent freeze).
//! `MIN_HIBERNATION_SECONDS` / `MAX_HIBERNATION_SECONDS` now bound it.

extern crate alloc;

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let token_address = Address::generate(&env);

    let contract_id = env.register_contract(None, TtlVaultContract);
    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(TtlVaultContractClient::new(&env, &contract_id)) };
    client.initialize(&token_address, &admin);

    (env, owner, beneficiary, client)
}

#[test]
fn test_hibernation_duration_zero_returns_error() {
    let (_env, owner, beneficiary, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let result = client.try_enter_hibernation(&id, &owner, &0u64);
    assert!(result.is_err(), "zero duration should be rejected");
    match result.unwrap_err().unwrap() {
        ContractError::HibernationDurationTooShort => {}
        e => panic!("Expected HibernationDurationTooShort, got {:?}", e),
    }
}

#[test]
fn test_hibernation_duration_below_min_returns_error() {
    let (_env, owner, beneficiary, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let result = client.try_enter_hibernation(&id, &owner, &(MIN_HIBERNATION_SECONDS - 1));
    assert!(
        result.is_err(),
        "duration below MIN_HIBERNATION_SECONDS should be rejected"
    );
    match result.unwrap_err().unwrap() {
        ContractError::HibernationDurationTooShort => {}
        e => panic!("Expected HibernationDurationTooShort, got {:?}", e),
    }
}

#[test]
fn test_hibernation_duration_exact_min_succeeds() {
    let (_env, owner, beneficiary, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let result = client.try_enter_hibernation(&id, &owner, &MIN_HIBERNATION_SECONDS);
    assert!(
        result.is_ok(),
        "duration == MIN_HIBERNATION_SECONDS should succeed"
    );
}

#[test]
fn test_hibernation_duration_exact_max_succeeds() {
    let (_env, owner, beneficiary, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let result = client.try_enter_hibernation(&id, &owner, &MAX_HIBERNATION_SECONDS);
    assert!(
        result.is_ok(),
        "duration == MAX_HIBERNATION_SECONDS should succeed"
    );
}

#[test]
fn test_hibernation_duration_over_max_returns_error() {
    let (_env, owner, beneficiary, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let result = client.try_enter_hibernation(&id, &owner, &(MAX_HIBERNATION_SECONDS + 1));
    assert!(
        result.is_err(),
        "duration above MAX_HIBERNATION_SECONDS should be rejected"
    );
    match result.unwrap_err().unwrap() {
        ContractError::HibernationDurationTooLong => {}
        e => panic!("Expected HibernationDurationTooLong, got {:?}", e),
    }
    assert!(!client.is_hibernating(&id));
}
