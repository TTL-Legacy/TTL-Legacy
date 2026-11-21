#![cfg(test)]

//! Regression tests for issue #1274: `check_in` must reject a check-in once
//! the vault has already expired. Before this fix, an owner could call
//! `check_in` after the TTL had elapsed to silently re-arm a vault that
//! should have released to the beneficiary instead.

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, BytesN, Env,
};

fn setup() -> (
    Env,
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
    let token_address = env.register_stellar_asset_contract_v2(token_admin).address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, beneficiary, client)
}

/// A check-in performed before the TTL elapses should succeed normally.
#[test]
fn test_check_in_succeeds_before_expiry() {
    let (env, owner, beneficiary, client) = setup();
    let interval = 3_600u64; // minimum allowed interval

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    env.ledger().with_mut(|l| l.timestamp += interval - 1);
    client.check_in(&vault_id, &owner, &BytesN::from_array(&env, &[1u8; 32]), &0u64, &None, &None);

    assert!(!client.is_expired(&vault_id));
}

/// Once `now >= last_check_in + check_in_interval`, the vault has expired and
/// `check_in` must be rejected with `ContractError::VaultExpired` rather than
/// silently resetting the timer.
#[test]
fn test_check_in_rejected_after_expiry() {
    let (env, owner, beneficiary, client) = setup();
    let interval = 3_600u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    assert!(!client.is_expired(&vault_id));

    // Advance past the deadline without checking in.
    env.ledger().with_mut(|l| l.timestamp += interval + 1);
    assert!(client.is_expired(&vault_id));

    let result = client.try_check_in(
        &vault_id,
        &owner,
        &BytesN::from_array(&env, &[1u8; 32]),
        &0u64,
    , &None, &None);
    assert!(result.is_err(), "check_in should reject an already-expired vault");
    match result.unwrap_err().unwrap() {
        ContractError::VaultExpired => {}
        e => panic!("Expected VaultExpired, got {:?}", e),
    }

    // The vault's last_check_in must be unchanged: the late check-in must
    // not have re-armed the TTL.
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.last_check_in, 0);
}

/// A check-in exactly at the deadline (now == last_check_in + interval) is
/// also considered expired per `is_expired`'s `now >= deadline` semantics.
#[test]
fn test_check_in_rejected_exactly_at_deadline() {
    let (env, owner, beneficiary, client) = setup();
    let interval = 3_600u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    env.ledger().with_mut(|l| l.timestamp += interval);
    assert!(client.is_expired(&vault_id));

    let result = client.try_check_in(
        &vault_id,
        &owner,
        &BytesN::from_array(&env, &[1u8; 32]),
        &0u64,
    , &None, &None);
    assert!(result.is_err());
    match result.unwrap_err().unwrap() {
        ContractError::VaultExpired => {}
        e => panic!("Expected VaultExpired, got {:?}", e),
    }
}
