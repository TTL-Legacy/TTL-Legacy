/// Tests for vault ownership transfer with two-step acceptance flow (Issue #1340)
///
/// Covers:
///   - `initiate_ownership_transfer`: current owner proposes a new owner
///   - `accept_ownership_transfer`: new owner accepts after the 24-hour time-lock
///   - Timeout / expiry: pending transfer expires after 7 days
///   - Rejection / cancellation: owner or proposed new owner can cancel
///
/// These tests exercise the complete flow described in docs/ownership-transfer.md.
#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env, IntoVal, TryIntoVal,
};

/// Returns `true` if any event in the environment has `topic_sym` as its first topic.
fn has_event(env: &Env, topic_sym: soroban_sdk::Symbol) -> bool {
    env.events().all().iter().any(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(env);
        topics
            .get(0)
            .and_then(|v| v.try_into_val(env).ok())
            .map(|s: soroban_sdk::Symbol| s == topic_sym)
            .unwrap_or(false)
    })
}

/// Shared test harness: registers the contract, mints tokens for `owner`, and
/// returns `(env, owner, beneficiary, admin, token_address, client)`.
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

// ---------------------------------------------------------------------------
// initiate_ownership_transfer
// ---------------------------------------------------------------------------

/// Happy path: initiating a transfer stores a pending request and leaves vault
/// ownership unchanged until the new owner accepts.
#[test]
fn test_1340_initiate_transfer_stores_pending_request() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    let unlocks_at = client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);

    let req = client
        .get_pending_ownership_transfer(&vault_id)
        .expect("pending request must exist after initiation");

    assert_eq!(req.new_owner, new_owner, "pending request should target new_owner");
    assert_eq!(req.unlocks_at, unlocks_at, "unlocks_at must match returned value");
    // Ownership not transferred yet
    assert_eq!(
        client.get_vault(&vault_id).owner,
        owner,
        "vault owner must remain unchanged until accepted"
    );
}

/// Initiating a transfer emits the OWNERSHIP_INITIATED_TOPIC event.
#[test]
fn test_1340_initiate_transfer_emits_initiated_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);

    assert!(
        has_event(&env, types::OWNERSHIP_INITIATED_TOPIC),
        "OWNERSHIP_INITIATED_TOPIC event must be emitted"
    );
}

/// Only the current vault owner can initiate a transfer; a non-owner is rejected.
#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_1340_initiate_transfer_non_owner_rejected() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let stranger = Address::generate(&env);
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    // Stranger must not be able to initiate a transfer
    client.initiate_ownership_transfer(&vault_id, &stranger, &new_owner);
}

/// Attempting to transfer to the current owner should fail with AlreadyOwner (#91).
#[test]
#[should_panic(expected = "Error(Contract, #91)")]
fn test_1340_initiate_transfer_to_current_owner_fails() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &owner);
}

/// Attempting to transfer to the vault's beneficiary is forbidden (#17).
#[test]
#[should_panic(expected = "Error(Contract, #17)")]
fn test_1340_initiate_transfer_to_beneficiary_fails() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &beneficiary);
}

/// A second call to `initiate_ownership_transfer` replaces the existing pending request.
#[test]
fn test_1340_initiate_transfer_replaces_existing_pending() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let first_new_owner = Address::generate(&env);
    let second_new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &first_new_owner);
    client.initiate_ownership_transfer(&vault_id, &owner, &second_new_owner);

    let req = client
        .get_pending_ownership_transfer(&vault_id)
        .expect("pending request must exist");
    assert_eq!(
        req.new_owner, second_new_owner,
        "second initiation must overwrite first"
    );
}

// ---------------------------------------------------------------------------
// accept_ownership_transfer
// ---------------------------------------------------------------------------

/// Full happy-path: initiate → wait past 24-hour time-lock → accept.
/// Vault owner must be updated and pending request cleared.
#[test]
fn test_1340_accept_transfer_after_timelock_succeeds() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);

    // Advance past the 24-hour (86_400 s) time-lock
    env.ledger().with_mut(|l| l.timestamp += 86_401);

    client.accept_ownership_transfer(&vault_id, &new_owner);

    assert_eq!(
        client.get_vault(&vault_id).owner,
        new_owner,
        "vault owner must be updated to new_owner after acceptance"
    );
    assert!(
        client.get_pending_ownership_transfer(&vault_id).is_none(),
        "pending request must be cleared after acceptance"
    );
}

/// Acceptance before the 24-hour time-lock elapses must be rejected (#36).
#[test]
#[should_panic(expected = "Error(Contract, #36)")]
fn test_1340_accept_transfer_before_timelock_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    // Do NOT advance time — time-lock not elapsed
    client.accept_ownership_transfer(&vault_id, &new_owner);
}

/// Acceptance after the 7-day expiry window must be rejected (#35).
#[test]
#[should_panic(expected = "Error(Contract, #35)")]
fn test_1340_accept_transfer_after_expiry_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    // Advance past 7-day expiry (604_800 s)
    env.ledger().with_mut(|l| l.timestamp += 604_801);

    client.accept_ownership_transfer(&vault_id, &new_owner);
}

/// Only the designated new owner can accept; an impostor must be rejected (#6).
#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_1340_accept_transfer_wrong_address_rejected() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let impostor = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    env.ledger().with_mut(|l| l.timestamp += 86_401);

    client.accept_ownership_transfer(&vault_id, &impostor);
}

/// After acceptance the old owner's vault index is updated and the new owner
/// gains the vault in their index.
#[test]
fn test_1340_accept_transfer_updates_owner_indexes() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    env.ledger().with_mut(|l| l.timestamp += 86_401);
    client.accept_ownership_transfer(&vault_id, &new_owner);

    let old_owner_vaults = client.get_vaults_by_owner(&owner, &None, &0u32, &10u32);
    let new_owner_vaults = client.get_vaults_by_owner(&new_owner, &None, &0u32, &10u32);

    assert!(
        !old_owner_vaults.iter().any(|id| id == vault_id),
        "old owner must no longer have the vault in their index"
    );
    assert!(
        new_owner_vaults.iter().any(|id| id == vault_id),
        "new owner must have the vault in their index"
    );
}

/// Acceptance emits the OWNERSHIP_ACCEPTED_TOPIC event.
#[test]
fn test_1340_accept_transfer_emits_accepted_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    env.ledger().with_mut(|l| l.timestamp += 86_401);
    client.accept_ownership_transfer(&vault_id, &new_owner);

    assert!(
        has_event(&env, types::OWNERSHIP_ACCEPTED_TOPIC),
        "OWNERSHIP_ACCEPTED_TOPIC event must be emitted on acceptance"
    );
}

/// Calling accept when no pending transfer exists must fail (#34).
#[test]
#[should_panic(expected = "Error(Contract, #34)")]
fn test_1340_accept_transfer_no_pending_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    env.ledger().with_mut(|l| l.timestamp += 86_401);
    // No initiation has been called
    client.accept_ownership_transfer(&vault_id, &new_owner);
}

// ---------------------------------------------------------------------------
// Timeout / expiry
// ---------------------------------------------------------------------------

/// `expire_ownership_transfer` cleans up a stale request and emits the expiry event.
#[test]
fn test_1340_expire_transfer_after_expiry_succeeds() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    // Advance past 7-day expiry
    env.ledger().with_mut(|l| l.timestamp += 604_801);

    client.expire_ownership_transfer(&vault_id);

    assert!(
        client.get_pending_ownership_transfer(&vault_id).is_none(),
        "pending request must be removed after expiry"
    );
    assert!(
        has_event(&env, types::OWNERSHIP_TRANSFER_EXPIRED_TOPIC),
        "OWNERSHIP_TRANSFER_EXPIRED_TOPIC must be emitted"
    );
    // Vault ownership must remain with original owner
    assert_eq!(client.get_vault(&vault_id).owner, owner);
}

/// `expire_ownership_transfer` must fail when the request has not yet expired (#16).
#[test]
#[should_panic(expected = "Error(Contract, #16)")]
fn test_1340_expire_transfer_before_expiry_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    // Not expired yet
    client.expire_ownership_transfer(&vault_id);
}

// ---------------------------------------------------------------------------
// Cancellation / rejection
// ---------------------------------------------------------------------------

/// The current vault owner can cancel a pending transfer.
#[test]
fn test_1340_cancel_transfer_by_owner_removes_pending_request() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    assert!(client.get_pending_ownership_transfer(&vault_id).is_some());

    client.cancel_ownership_transfer(&vault_id, &owner);

    assert!(
        client.get_pending_ownership_transfer(&vault_id).is_none(),
        "pending request must be cleared after cancellation"
    );
    // Vault owner must remain unchanged
    assert_eq!(client.get_vault(&vault_id).owner, owner);
}

/// The designated new owner can decline (cancel) the pending transfer.
#[test]
fn test_1340_cancel_transfer_by_new_owner_acts_as_rejection() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    client.cancel_ownership_transfer(&vault_id, &new_owner);

    assert!(
        client.get_pending_ownership_transfer(&vault_id).is_none(),
        "pending request must be cleared when new owner declines"
    );
    assert_eq!(
        client.get_vault(&vault_id).owner,
        owner,
        "vault owner must remain unchanged after rejection"
    );
}

/// Cancellation emits the OWNERSHIP_CANCELLED_TOPIC event.
#[test]
fn test_1340_cancel_transfer_emits_cancelled_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    client.cancel_ownership_transfer(&vault_id, &owner);

    assert!(
        has_event(&env, types::OWNERSHIP_CANCELLED_TOPIC),
        "OWNERSHIP_CANCELLED_TOPIC event must be emitted on cancellation"
    );
}

/// An unrelated address cannot cancel a pending transfer (#6).
#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_1340_cancel_transfer_by_stranger_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let stranger = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    client.cancel_ownership_transfer(&vault_id, &stranger);
}

/// Cancelling when no pending transfer exists must fail (#34).
#[test]
#[should_panic(expected = "Error(Contract, #34)")]
fn test_1340_cancel_transfer_no_pending_fails() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);
    // No initiation has been called
    client.cancel_ownership_transfer(&vault_id, &owner);
}

// ---------------------------------------------------------------------------
// Beneficiary index invariant
// ---------------------------------------------------------------------------

/// After a successful ownership transfer the beneficiary index must be preserved.
#[test]
fn test_1340_transfer_preserves_beneficiary_index() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    env.ledger().with_mut(|l| l.timestamp += 86_401);
    client.accept_ownership_transfer(&vault_id, &new_owner);

    // Beneficiary must still be the same address and still indexed
    assert_eq!(client.get_vault(&vault_id).beneficiary, beneficiary);
    let ben_vaults = client.get_vaults_by_beneficiary(&beneficiary, &None, &0u32, &10u32);
    assert!(
        ben_vaults.iter().any(|id| id == vault_id),
        "beneficiary index must remain intact after ownership transfer"
    );
}
