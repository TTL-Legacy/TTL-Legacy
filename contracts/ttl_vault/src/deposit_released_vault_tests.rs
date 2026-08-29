//! Tests for Issue #1282: deposit must reject vaults that are no longer in
//! active (Locked) state.
//!
//! Covered states:
//!   * `ReleaseStatus::Released`        → `ContractError::VaultReleased`
//!   * `ReleaseStatus::Cancelled`       → `ContractError::VaultReleased`
//!   * `ReleaseStatus::EmergencyFrozen` → `ContractError::VaultFrozen`
//!
//! Also includes a positive control confirming the happy path is unaffected.

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env,
};

// ---------------------------------------------------------------------------
// Shared setup
// ---------------------------------------------------------------------------

fn setup() -> (
    Env,
    Address, // owner
    Address, // beneficiary
    Address, // admin
    Address, // token_address
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
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    (env, owner, beneficiary, admin, token_address, client)
}

// ---------------------------------------------------------------------------
// Released vault
// ---------------------------------------------------------------------------

/// Issue #1282: deposit into a Released vault must return VaultReleased.
///
/// Steps:
///   1. Create a vault with a short check-in interval.
///   2. Deposit initial funds so the vault has a balance.
///   3. Advance the ledger past the check-in deadline (vault expires).
///   4. Call `trigger_release` — status becomes `Released`.
///   5. Attempt another deposit and assert it panics with `VaultReleased`.
#[test]
#[should_panic(expected = "VaultReleased")]
fn test_deposit_on_released_vault_panics_with_vault_released() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 1_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Fund vault before releasing it
    client.deposit(&vault_id, &owner, &1_000_000i128);

    // Expire the vault
    env.ledger()
        .with_mut(|l| l.timestamp = l.timestamp + interval + 1);

    client.trigger_release(&vault_id);
    assert_eq!(
        client.get_vault(&vault_id).status,
        ReleaseStatus::Released,
        "Vault must be Released after trigger_release"
    );

    // Must panic with VaultReleased
    client.deposit(&vault_id, &owner, &500_000i128);
}

/// Issue #1282: try_deposit on a Released vault returns Err with error code
/// matching ContractError::VaultReleased (97).
#[test]
fn test_try_deposit_on_released_vault_returns_vault_released_error() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 1_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&vault_id, &owner, &1_000_000i128);

    env.ledger()
        .with_mut(|l| l.timestamp = l.timestamp + interval + 1);
    client.trigger_release(&vault_id);

    let result = client.try_deposit(&vault_id, &owner, &500_000i128);
    assert!(result.is_err(), "Deposit into Released vault must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ContractError::VaultReleased as u32),
        "Error code must be VaultReleased"
    );
}

// ---------------------------------------------------------------------------
// Cancelled vault
// ---------------------------------------------------------------------------

/// Issue #1282: deposit into a Cancelled vault must return VaultReleased.
///
/// Steps:
///   1. Create a vault.
///   2. Call `cancel_vault` as owner — status becomes `Cancelled`.
///   3. Attempt a deposit and assert it panics with `VaultReleased`.
#[test]
#[should_panic(expected = "VaultReleased")]
fn test_deposit_on_cancelled_vault_panics_with_vault_released() {
    let (_env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 1_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    client.cancel_vault(&vault_id, &owner);
    assert_eq!(
        client.get_vault(&vault_id).status,
        ReleaseStatus::Cancelled,
        "Vault must be Cancelled after cancel_vault"
    );

    // Must panic with VaultReleased
    client.deposit(&vault_id, &owner, &500_000i128);
}

/// Issue #1282: try_deposit on a Cancelled vault returns Err(VaultReleased).
#[test]
fn test_try_deposit_on_cancelled_vault_returns_vault_released_error() {
    let (_env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 1_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.cancel_vault(&vault_id, &owner);

    let result = client.try_deposit(&vault_id, &owner, &500_000i128);
    assert!(result.is_err(), "Deposit into Cancelled vault must fail");
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ContractError::VaultReleased as u32),
        "Error code must be VaultReleased"
    );
}

// ---------------------------------------------------------------------------
// EmergencyFrozen vault
// ---------------------------------------------------------------------------

/// Issue #1282: deposit into an EmergencyFrozen vault must return VaultFrozen,
/// not VaultReleased.
///
/// EmergencyFrozen status is set via direct storage write — the same technique
/// used by trigger_release_bench_tests.rs and vault_archiving_tests.rs, since
/// no public entry-point transitions a vault into this state.
#[test]
#[should_panic(expected = "VaultFrozen")]
fn test_deposit_on_emergency_frozen_vault_panics_with_vault_frozen() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 1_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Directly set status to EmergencyFrozen in persistent storage
    let mut vault = client.get_vault(&vault_id);
    vault.status = ReleaseStatus::EmergencyFrozen;
    env.as_contract(&client.address, || {
        let key = StorageKey::Vault(vault_id);
        env.storage().persistent().set(&key, &vault);
    });

    assert_eq!(
        client.get_vault(&vault_id).status,
        ReleaseStatus::EmergencyFrozen,
        "Vault must be EmergencyFrozen after storage write"
    );

    // Must panic with VaultFrozen, not VaultReleased
    client.deposit(&vault_id, &owner, &500_000i128);
}

/// Issue #1282: try_deposit on an EmergencyFrozen vault returns Err(VaultFrozen).
#[test]
fn test_try_deposit_on_emergency_frozen_vault_returns_vault_frozen_error() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 1_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let mut vault = client.get_vault(&vault_id);
    vault.status = ReleaseStatus::EmergencyFrozen;
    env.as_contract(&client.address, || {
        let key = StorageKey::Vault(vault_id);
        env.storage().persistent().set(&key, &vault);
    });

    let result = client.try_deposit(&vault_id, &owner, &500_000i128);
    assert!(
        result.is_err(),
        "Deposit into EmergencyFrozen vault must fail"
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        soroban_sdk::Error::from_contract_error(ContractError::VaultFrozen as u32),
        "Error code must be VaultFrozen, not VaultReleased"
    );
}

// ---------------------------------------------------------------------------
// Positive control — active vault
// ---------------------------------------------------------------------------

/// Sanity check: a Locked (active) vault must still accept deposits normally.
/// Guards against over-eager state checks breaking the happy path.
#[test]
fn test_deposit_on_active_locked_vault_succeeds() {
    let (_env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 1_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    assert_eq!(
        client.get_vault(&vault_id).status,
        ReleaseStatus::Locked,
        "New vault must start in Locked state"
    );

    client.deposit(&vault_id, &owner, &1_000_000i128);

    assert_eq!(
        client.get_vault(&vault_id).balance,
        1_000_000i128,
        "Balance must reflect the deposit"
    );
}
