//! Issue 2 – owner-initiated vault lock/unlock.
//!
//! Owners can call `owner_lock_vault` to freeze deposit, withdraw, and
//! check_in operations when they suspect a passkey compromise.
//! `owner_unlock_vault` (requires fresh auth) restores normal operation.

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, BytesN, Env,
};

fn setup() -> (Env, Address, Address, Address, TtlVaultContractClient<'static>) {
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
    (env, owner, beneficiary, token_address, client)
}

/// Owner can lock a vault; `is_owner_vault_locked` reflects the new state.
#[test]
fn test_owner_lock_vault_sets_flag() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);

    assert!(!client.is_owner_vault_locked(&vault_id), "vault must start unlocked");

    client.owner_lock_vault(&vault_id, &owner).unwrap();

    assert!(client.is_owner_vault_locked(&vault_id), "vault must be locked after owner_lock_vault");
}

/// deposit is rejected with VaultOwnerLocked when the vault is owner-locked.
#[test]
fn test_deposit_rejected_when_vault_is_owner_locked() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);

    client.owner_lock_vault(&vault_id, &owner).unwrap();

    let result = client.try_deposit(&vault_id, &owner, &100_000i128);
    assert!(result.is_err(), "deposit must fail on owner-locked vault");
    let err = result.unwrap_err().unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from_contract_error(ContractError::VaultOwnerLocked as u32),
    );
}

/// withdraw is rejected with VaultOwnerLocked when the vault is owner-locked.
#[test]
fn test_withdraw_rejected_when_vault_is_owner_locked() {
    let (env, owner, beneficiary, token_address, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);

    // Deposit first so there is something to withdraw
    client.deposit(&vault_id, &owner, &500_000i128);

    client.owner_lock_vault(&vault_id, &owner).unwrap();

    let result = client.try_withdraw(&vault_id, &owner, &100_000i128, &None, &None, &None);
    assert!(result.is_err(), "withdraw must fail on owner-locked vault");
    let err = result.unwrap_err().unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from_contract_error(ContractError::VaultOwnerLocked as u32),
    );
}

/// check_in is rejected with VaultOwnerLocked when the vault is owner-locked.
#[test]
fn test_check_in_rejected_when_vault_is_owner_locked() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);

    client.owner_lock_vault(&vault_id, &owner).unwrap();

    let pk = BytesN::from_array(&env, &[1u8; 32]);
    let result = client.try_check_in(&vault_id, &owner, &pk, &0u64);
    assert!(result.is_err(), "check_in must fail on owner-locked vault");
    let err = result.unwrap_err().unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from_contract_error(ContractError::VaultOwnerLocked as u32),
    );
}

/// After unlock, operations resume normally.
#[test]
fn test_operations_resume_after_owner_unlock() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);

    client.owner_lock_vault(&vault_id, &owner).unwrap();
    assert!(client.is_owner_vault_locked(&vault_id));

    client.owner_unlock_vault(&vault_id, &owner).unwrap();
    assert!(!client.is_owner_vault_locked(&vault_id), "vault must be unlocked");

    // deposit should succeed now
    client.deposit(&vault_id, &owner, &100_000i128);
    assert_eq!(client.get_vault(&vault_id).balance, 100_000);
}

/// Non-owner cannot lock the vault.
#[test]
fn test_non_owner_cannot_lock_vault() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);

    let non_owner = Address::generate(&env);
    let result = client.try_owner_lock_vault(&vault_id, &non_owner);
    assert!(result.is_err(), "non-owner must not be able to lock vault");
}

/// Non-owner cannot unlock the vault.
#[test]
fn test_non_owner_cannot_unlock_vault() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);

    client.owner_lock_vault(&vault_id, &owner).unwrap();

    let non_owner = Address::generate(&env);
    let result = client.try_owner_unlock_vault(&vault_id, &non_owner);
    assert!(result.is_err(), "non-owner must not be able to unlock vault");
    // Vault must still be locked
    assert!(client.is_owner_vault_locked(&vault_id));
}

/// Unlocking an already-unlocked vault returns VaultOwnerLocked error.
#[test]
fn test_unlock_already_unlocked_vault_is_error() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);

    // Vault is not locked — try to unlock it
    let result = client.try_owner_unlock_vault(&vault_id, &owner);
    assert!(
        result.is_err(),
        "unlocking an already-unlocked vault must return an error"
    );
}
