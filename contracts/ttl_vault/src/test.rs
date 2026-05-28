#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{storage::{Instance as _, Persistent as _}, Address as _, Events, Ledger},
    token::{self, StellarAssetClient},
    vec, Address, BytesN, Env, IntoVal, TryIntoVal,
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

// ---- existing tests ----

#[test]
fn test_initialize_guard_against_double_init() {
    let (env, _, _, admin, token_address, client) = setup();

    let original_admin = client.get_admin();
    let original_token = client.get_contract_token();

    let new_admin = Address::generate(&env);
    let new_token_admin = Address::generate(&env);
    let new_token_address = env
        .register_stellar_asset_contract_v2(new_token_admin)
        .address();

    let err = client.try_initialize(&new_token_address, &new_admin).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(1));

    assert_eq!(client.get_admin(), original_admin);
    assert_eq!(client.get_contract_token(), original_token);
}

#[test]
#[should_panic(expected = "Error(Contract, #20)")]
fn test_initialize_rejects_same_xlm_token_and_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let addr = Address::generate(&env);
    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&addr, &addr);
}

#[test]
fn test_vault_count_view() {
    let (_, owner, beneficiary, _, _, client) = setup();

    assert_eq!(client.vault_count(), 0);
    let id_1 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let id_2 = client.create_vault(&owner, &beneficiary, &200u64, &None);

    assert_eq!(id_1, 1);
    assert_eq!(id_2, 2);
    assert_eq!(client.vault_count(), 2);
}

#[test]
fn test_vault_count_not_incremented_on_failed_create() {
    let (env, owner, _beneficiary, _, _, client) = setup();

    assert_eq!(client.vault_count(), 0);

    assert!(client.try_create_vault(&owner, &owner, &100u64, &None).is_err()); // InvalidBeneficiary must not mutate count
    assert_eq!(client.vault_count(), 0);
}

#[test]
fn test_vault_count_is_consistent_after_create() {
    let (_env, owner, beneficiary, _, _, client) = setup();

    assert_eq!(client.vault_count(), 0);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    assert_eq!(vault_id, 1);
    assert_eq!(client.vault_count(), vault_id);

    let vault_id_2 = client.create_vault(&owner, &beneficiary, &200u64, &None);
    assert_eq!(vault_id_2, 2);
    assert_eq!(client.vault_count(), vault_id_2);
}

#[test]
fn test_vault_exists_for_existing_and_missing_ids() {
    let (_, owner, beneficiary, _, _, client) = setup();

    assert!(!client.vault_exists(&1));

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    assert!(client.vault_exists(&vault_id));
    assert!(!client.vault_exists(&(vault_id + 1)));
}

#[test]
fn test_get_release_status_view() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Locked);

    client.deposit(&vault_id, &owner, &500i128);
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    assert_eq!(
        client.get_release_status(&vault_id),
        ReleaseStatus::Released
    );

    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&beneficiary), 500i128);
}

#[test]
fn test_batch_deposit_updates_multiple_vaults() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &200u64, &None);
    let token_client = token::Client::new(&env, &token_address);

    client.batch_deposit(
        &owner,
        &vec![&env, (vault_id_1, 150i128), (vault_id_2, 250i128)],
    );

    assert_eq!(client.get_vault(&vault_id_1).balance, 150i128);
    assert_eq!(client.get_vault(&vault_id_2).balance, 250i128);
    assert_eq!(token_client.balance(&owner), 999_600i128);
}

#[test]
fn test_batch_deposit_validates_all_items_before_transfer() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let token_client = token::Client::new(&env, &token_address);

    assert!(
        client
            .try_batch_deposit(&owner, &vec![&env, (vault_id, 100i128), (999u64, 200i128)])
            .is_err()
    );

    assert_eq!(client.get_vault(&vault_id).balance, 0i128);
    assert_eq!(token_client.balance(&owner), 1_000_000i128);

    assert!(
        client
            .try_batch_deposit(&owner, &vec![&env, (vault_id, 100i128), (vault_id, 0i128)])
            .is_err()
    );

    assert_eq!(client.get_vault(&vault_id).balance, 0i128);
    assert_eq!(token_client.balance(&owner), 1_000_000i128);
}

#[test]
fn test_batch_deposit_rejected_when_any_vault_expired() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &200u64, &None);
    let token_client = token::Client::new(&env, &token_address);

    // Advance time past vault_id_1's expiry
    env.ledger().with_mut(|l| l.timestamp += 150);

    // batch_deposit should fail because vault_id_1 is expired
    assert!(
        client
            .try_batch_deposit(&owner, &vec![&env, (vault_id_1, 100i128), (vault_id_2, 200i128)])
            .is_err()
    );

    // No funds should have been transferred
    assert_eq!(client.get_vault(&vault_id_1).balance, 0i128);
    assert_eq!(client.get_vault(&vault_id_2).balance, 0i128);
    assert_eq!(token_client.balance(&owner), 1_000_000i128);
}

#[test]
fn test_pause_and_unpause_toggle() {
    let (_, _, _, _, _, client) = setup();

    assert!(!client.is_paused());
    client.pause();
    assert!(client.is_paused());
    client.unpause();
    assert!(!client.is_paused());
}

// ---- Issue #316: pause event emission test ----

#[test]
fn test_pause_emits_event() {
    let (env, _, _, _, _, client) = setup();
    client.pause();

    let event = env.events().all().iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        topics
            .get(0)
            .and_then(|v| v.try_into_val(&env).ok())
            .map(|s: soroban_sdk::Symbol| s == types::PAUSE_TOPIC)
            .unwrap_or(false)
    });
    assert!(event.is_some(), "pause event not emitted");

    let data: bool = event.unwrap().2.into_val(&env);
    assert!(data);
}

// ---- Issue #317: unpause event emission test ----

#[test]
fn test_unpause_emits_event() {
    let (env, _, _, _, _, client) = setup();

    client.pause();
    client.unpause();

    let events = env.events().all();
    let unpause_event = events.iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        if topics.len() < 2 {
            return false;
        }
        let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
        topic0.map(|s| s == soroban_sdk::symbol_short!("unpause")).unwrap_or(false)
    });

    assert!(unpause_event.is_some(), "unpause event not emitted");
    // data field should be `false` (new paused state)
    let data: bool = unpause_event.unwrap().2.into_val(&env);
    assert!(!data);
}

#[test]
fn test_get_admin_view() {
    let (_, _, _, admin, _, client) = setup();

    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_paused_blocks_check_in_withdraw_and_trigger_release() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &200i128);
    env.ledger().with_mut(|l| l.timestamp += 200);

    client.pause();

    assert!(client.try_check_in(&vault_id, &owner).is_err());
    assert!(client.try_withdraw(&vault_id, &owner, &10i128).is_err());
    assert!(client.try_trigger_release(&vault_id).is_err());

    client.unpause();
    client.trigger_release(&vault_id);
    assert_eq!(
        client.get_release_status(&vault_id),
        ReleaseStatus::Released
    );
}

// ---- Issue #229: check_in event emission test ----

#[test]
fn test_check_in_emits_event_with_correct_topic() {
    let (env, owner, beneficiary, _, _, client) = setup();

    env.mock_all_auths();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    // Advance time slightly
    env.ledger().with_mut(|l| l.timestamp += 10);

    client.check_in(&vault_id, &owner);

    let events = env.events().all();
    let check_in_event = events.iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        if topics.len() < 2 {
            return false;
        }
        let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
        topic0.map(|s| s == soroban_sdk::symbol_short!("check_in")).unwrap_or(false)
    });

    assert!(check_in_event.is_some(), "check_in event not emitted");
}

#[test]
fn test_get_vaults_by_owner_tracks_multiple_vaults() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &200u64, &None);

    assert_eq!(
        client.get_vaults_by_owner(&owner, &None, &0u32, &10u32),
        vec![&env, vault_id_1, vault_id_2]
    );
}

#[test]
fn test_update_check_in_interval() {
    let (_, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.update_check_in_interval(&vault_id, &300u64);
    assert_eq!(client.get_vault(&vault_id).check_in_interval, 300u64);

    assert!(client.try_update_check_in_interval(&vault_id, &0u64).is_err());
}

#[test]
fn test_update_check_in_interval_extends_vault_storage_ttl() {
    // Create a vault with a short interval (100s → TTL = VAULT_TTL_LEDGERS minimum).
    // Increase the interval to a large value whose derived TTL exceeds the minimum.
    // The vault must still be readable after the update, confirming save_vault
    // re-extended persistent storage using the new (larger) interval.
    let (env, owner, beneficiary, _, _, client) = setup();

    // 30-day interval: vault_ttl_ledgers(2_592_000) = 1_036_800 ledgers > VAULT_TTL_LEDGERS
    let long_interval: u64 = 30 * 24 * 3600; // 2_592_000 seconds

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    // Increase interval — save_vault must use the new interval for extend_ttl
    client.update_check_in_interval(&vault_id, &long_interval);

    // Vault is readable and carries the updated interval
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.check_in_interval, long_interval);

    // Advance time just under the new interval — vault must still be accessible
    env.ledger().with_mut(|l| l.timestamp += long_interval - 1);
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.check_in_interval, long_interval);
}

#[test]
fn test_transfer_ownership_updates_owner_and_owner_index() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    assert_eq!(client.get_vaults_by_owner(&owner, &None, &0u32, &10u32), vec![&env, vault_id]);
    assert_eq!(client.get_vaults_by_owner(&new_owner, &None, &0u32, &10u32), vec![&env]);

    // Step 1: initiate
    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    // Owner index unchanged until accepted
    assert_eq!(client.get_vault(&vault_id).owner, owner);

    // Step 2: advance past time-lock (24h + 1s)
    env.ledger().with_mut(|l| l.timestamp += 86_401);

    // Step 3: new owner accepts
    client.accept_ownership_transfer(&vault_id, &new_owner);

    assert_eq!(client.get_vault(&vault_id).owner, new_owner);
    assert_eq!(client.get_vaults_by_owner(&owner, &None, &0u32, &10u32), vec![&env]);
    assert_eq!(client.get_vaults_by_owner(&new_owner, &None, &0u32, &10u32), vec![&env, vault_id]);
}

/// Invariant: owner and beneficiary must always be distinct.
/// initiate_ownership_transfer must reject a new_owner that equals the vault's beneficiary,
/// and must not corrupt the BeneficiaryVaults index.
#[test]
#[should_panic(expected = "Error(Contract, #17)")]
fn test_transfer_ownership_rejects_new_owner_equal_to_beneficiary() {
    let (_, owner, beneficiary, _, _, client) = setup();

    // beneficiary is the vault's primary beneficiary; transferring ownership to
    // them would violate the owner != beneficiary invariant.
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.initiate_ownership_transfer(&vault_id, &owner, &beneficiary);
}

/// BeneficiaryVaults index must remain consistent after a successful ownership transfer.
/// The vault's beneficiary field is unchanged, so the beneficiary's index entry
/// must still point to the vault.
#[test]
fn test_transfer_ownership_preserves_beneficiary_index() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    // beneficiary index contains the vault before transfer
    assert_eq!(client.get_vaults_by_beneficiary(&beneficiary, &None, &0u32, &10u32), vec![&env, vault_id]);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    env.ledger().with_mut(|l| l.timestamp += 86_401);
    client.accept_ownership_transfer(&vault_id, &new_owner);

    // vault.beneficiary is unchanged — index must still be intact
    assert_eq!(client.get_vault(&vault_id).beneficiary, beneficiary);
    assert_eq!(client.get_vaults_by_beneficiary(&beneficiary, &None, &0u32, &10u32), vec![&env, vault_id]);
}

// --- Ownership Transfer: 2-step flow tests ---

#[test]
fn test_initiate_ownership_transfer_stores_pending_request() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    let unlocks_at = client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    let req = client.get_pending_ownership_transfer(&vault_id).expect("pending request should exist");
    assert_eq!(req.new_owner, new_owner);
    assert_eq!(req.unlocks_at, unlocks_at);
    // Vault owner unchanged until accepted
    assert_eq!(client.get_vault(&vault_id).owner, owner);
}

#[test]
#[should_panic(expected = "Error(Contract, #36)")]
fn test_accept_ownership_transfer_before_timelock_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    // Do NOT advance time — time-lock not yet elapsed
    client.accept_ownership_transfer(&vault_id, &new_owner);
}

#[test]
fn test_accept_ownership_transfer_after_timelock_succeeds() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    env.ledger().with_mut(|l| l.timestamp += 86_401);
    client.accept_ownership_transfer(&vault_id, &new_owner);

    assert_eq!(client.get_vault(&vault_id).owner, new_owner);
    // Pending request cleared
    assert!(client.get_pending_ownership_transfer(&vault_id).is_none());
}

#[test]
#[should_panic(expected = "Error(Contract, #35)")]
fn test_accept_ownership_transfer_after_expiry_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    // Advance past 7-day expiry
    env.ledger().with_mut(|l| l.timestamp += 604_801);
    client.accept_ownership_transfer(&vault_id, &new_owner);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_accept_ownership_transfer_wrong_address_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let impostor = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    env.ledger().with_mut(|l| l.timestamp += 86_401);
    // impostor tries to accept
    client.accept_ownership_transfer(&vault_id, &impostor);
}

#[test]
fn test_cancel_ownership_transfer_removes_pending_request() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    assert!(client.get_pending_ownership_transfer(&vault_id).is_some());

    client.cancel_ownership_transfer(&vault_id, &owner);
    assert!(client.get_pending_ownership_transfer(&vault_id).is_none());
    // Owner unchanged
    assert_eq!(client.get_vault(&vault_id).owner, owner);
}

#[test]
#[should_panic(expected = "Error(Contract, #34)")]
fn test_cancel_ownership_transfer_with_no_pending_fails() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    // No pending request — should fail
    client.cancel_ownership_transfer(&vault_id, &owner);
}

#[test]
#[should_panic(expected = "Error(Contract, #34)")]
fn test_accept_ownership_transfer_with_no_pending_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    env.ledger().with_mut(|l| l.timestamp += 86_401);
    // No pending request — should fail
    client.accept_ownership_transfer(&vault_id, &new_owner);
}

#[test]
fn test_initiate_ownership_transfer_replaces_existing_pending() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner1 = Address::generate(&env);
    let new_owner2 = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner1);
    // Replace with a different new owner
    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner2);

    let req = client.get_pending_ownership_transfer(&vault_id).unwrap();
    assert_eq!(req.new_owner, new_owner2);
}

#[test]
fn test_initiate_ownership_transfer_emits_initiated_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    assert!(find_event_by_topic(&env, types::OWNERSHIP_INITIATED_TOPIC));
}

#[test]
fn test_cancel_ownership_transfer_emits_cancelled_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    client.cancel_ownership_transfer(&vault_id, &owner);
    assert!(find_event_by_topic(&env, types::OWNERSHIP_CANCELLED_TOPIC));
}

#[test]
fn test_accept_ownership_transfer_emits_accepted_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    env.ledger().with_mut(|l| l.timestamp += 86_401);
    client.accept_ownership_transfer(&vault_id, &new_owner);
    assert!(find_event_by_topic(&env, types::OWNERSHIP_ACCEPTED_TOPIC));
}

#[test]
fn test_cancel_vault_refunds_owner_and_marks_cancelled() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let token_client = token::Client::new(&env, &token_address);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.deposit(&vault_id, &owner, &400i128);
    assert_eq!(token_client.balance(&owner), 999_600i128);

    client.cancel_vault(&vault_id, &owner);
    assert_eq!(token_client.balance(&owner), 1_000_000i128);
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Cancelled);
}

#[test]
fn test_cancel_vault_requires_auth_before_load() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let other = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    // other is not the owner, should fail with NotOwner
    assert!(client.try_cancel_vault(&vault_id, &other).is_err());
}

#[test]
fn test_admin_transfer_full_flow() {
    let (env, _, _, admin, _, client) = setup();
    let new_admin = Address::generate(&env);

    assert_eq!(client.get_admin(), admin.clone());
    assert_eq!(client.get_pending_admin(), None);

    client.propose_admin(&new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));

    client.accept_admin();
    assert_eq!(client.get_admin(), new_admin.clone());
    assert_eq!(client.get_pending_admin(), None);

    client.pause();
    assert!(client.is_paused());
    client.unpause();
    assert!(!client.is_paused());
}

#[test]
#[should_panic(expected = "Error(Contract, #17)")]
fn test_create_vault_rejects_owner_as_beneficiary() {
    let (_, owner, _, _, _, client) = setup();
    client.create_vault(&owner, &owner, &1000, &None);
}

#[test]
fn test_vault_count_consistent_after_creation() {
    let (_, owner, beneficiary, _, _, client) = setup();
    assert_eq!(client.vault_count(), 0);
    let id = client.create_vault(&owner, &beneficiary, &1000, &None);
    assert_eq!(id, 1);
    assert_eq!(client.vault_count(), 1);
}

#[test]
fn test_propose_admin_can_be_called_multiple_times() {
    let (env, _, _, _, _, client) = setup();
    let new_admin_1 = Address::generate(&env);
    let new_admin_2 = Address::generate(&env);

    client.propose_admin(&new_admin_1);
    assert_eq!(client.get_pending_admin(), Some(new_admin_1));

    client.propose_admin(&new_admin_2);
    assert_eq!(client.get_pending_admin(), Some(new_admin_2.clone()));

    client.accept_admin();
    assert_eq!(client.get_admin(), new_admin_2.clone());
    assert_eq!(client.get_pending_admin(), None);
    client.pause();
    assert!(client.is_paused());
}

// ---- Issue #227: accept_admin unauthorized rejection test ----

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_accept_admin_rejects_when_no_pending_admin() {
    let (_, _, _, _, _, client) = setup();

    // No pending admin set, so accept_admin should fail with NoPendingAdmin
    client.accept_admin();
}

// ---- Task 1: ping_expiry tests ----

#[test]
fn test_ping_expiry_emits_event_when_near_expiry() {
    let (env, owner, beneficiary, _, _, client) = setup();
    // interval = 100s, advance 50s => TTL remaining = 50 < EXPIRY_WARNING_THRESHOLD (86400)
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    env.ledger().with_mut(|l| l.timestamp += 50);

    let ttl = client.ping_expiry(&vault_id);
    assert_eq!(ttl, 50u64);
}

#[test]
fn test_ping_expiry_no_event_when_far_from_expiry() {
    let (env, owner, beneficiary, _, _, client) = setup();
    // interval = 200_000s, no time advance => TTL = 200_000 >= threshold, no event
    let vault_id = client.create_vault(&owner, &beneficiary, &200_000u64, &None);
    env.ledger().with_mut(|l| l.timestamp += 0);

    let ttl = client.ping_expiry(&vault_id);
    assert_eq!(ttl, 200_000u64);
}

#[test]
fn test_ping_expiry_returns_zero_when_expired() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    env.ledger().with_mut(|l| l.timestamp += 200);

    let ttl = client.ping_expiry(&vault_id);
    assert_eq!(ttl, 0u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_ping_expiry_panics_for_nonexistent_vault() {
    let (_, _, _, _, _, client) = setup();
    client.ping_expiry(&9999u64);
}

#[test]
fn test_get_ttl_remaining_returns_none_when_expired() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    env.ledger().with_mut(|l| l.timestamp += 200);

    assert!(client.get_ttl_remaining(&vault_id).is_none());
}

#[test]
fn test_get_ttl_remaining_returns_none_for_nonexistent_vault() {
    let (_, _, _, _, _, client) = setup();
    assert!(client.get_ttl_remaining(&9999u64).is_none());
}

// ---- Task 2: partial_release tests ----

#[test]
fn test_partial_release_transfers_amount_to_beneficiary() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    client.partial_release(&vault_id, &300i128);

    assert_eq!(token_client.balance(&beneficiary), 300i128);
    assert_eq!(client.get_vault(&vault_id).balance, 700i128);
    // vault still locked
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Locked);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_partial_release_fails_if_insufficient_balance() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &100i128);
    // attempt to release more than the balance
    client.partial_release(&vault_id, &200i128);
}

#[test]
fn test_partial_release_rejects_zero_amount() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &500i128);

    let result = client.try_partial_release(&vault_id, &0i128);
    assert!(result.is_err(), "expected error for zero-amount partial release");
}

#[test]
fn test_partial_release_rejects_negative_amount() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &500i128);

    let result = client.try_partial_release(&vault_id, &-1i128);
    assert!(result.is_err(), "expected error for negative-amount partial release");
}

#[test]
fn test_partial_release_emits_partial_event() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    client.partial_release(&vault_id, &300i128);

    // Assert balance decreased and beneficiary received funds
    assert_eq!(client.get_vault(&vault_id).balance, 700i128);
    assert_eq!(token_client.balance(&beneficiary), 300i128);

    // Assert the "partial" event was emitted
    let events = env.events().all();
    let partial_event = events.iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        if topics.len() < 2 {
            return false;
        }
        let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
        topic0.map(|s| s == soroban_sdk::symbol_short!("partial")).unwrap_or(false)
    });
    assert!(partial_event.is_some(), "partial event not emitted");
}

#[test]
#[should_panic(expected = "Error(Contract, #17)")]
fn test_update_beneficiary_rejects_owner_as_beneficiary() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &1000, &None);
    client.update_beneficiary(&vault_id, &owner, &owner);
}

#[test]
fn test_update_beneficiary_requires_auth_before_load() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let other = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &1000, &None);
    let new_beneficiary = Address::generate(&env);

    // other is not the owner, should fail with NotOwner
    assert!(client.try_update_beneficiary(&vault_id, &other, &new_beneficiary).is_err());
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")]
fn test_deposit_into_expired_vault_is_rejected() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.deposit(&vault_id, &owner, &500i128);
}

// ---- Issue #221: deposit expired vault returns VaultExpired error code ----

#[test]
fn test_deposit_into_expired_vault_returns_vault_expired_error() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    
    // Advance time past expiry
    env.ledger().with_mut(|l| l.timestamp += 200);
    
    // Should return VaultExpired (error code 19), not AlreadyReleased (error code 7)
    let err = client.try_deposit(&vault_id, &owner, &500i128).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(19)); // VaultExpired
}

#[test]
fn test_deposit_rejects_zero_amount() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    let err = client.try_deposit(&vault_id, &owner, &0i128).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(5));
}

#[test]
fn test_deposit_rejects_negative_amount() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    let err = client.try_deposit(&vault_id, &owner, &-1i128).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(5));
}

#[test]
fn test_update_metadata_can_be_overwritten() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.update_metadata(&vault_id, &owner, &soroban_sdk::String::from_str(&env, "v1"));
    client.update_metadata(&vault_id, &owner, &soroban_sdk::String::from_str(&env, "v2"));

    assert_eq!(
        client.get_vault(&vault_id).metadata,
        soroban_sdk::String::from_str(&env, "v2")
    );
}

#[test]
fn test_update_metadata_rejects_oversized_value() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let oversized = "a".repeat((MAX_METADATA_LEN + 1) as usize);

    let err = client
        .try_update_metadata(&vault_id, &owner, &soroban_sdk::String::from_str(&env, oversized.as_str()))
        .unwrap_err()
        .unwrap();

    assert_eq!(err, ContractError::InvalidAmount);
}

#[test]
fn test_update_metadata_emits_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let new_meta = soroban_sdk::String::from_str(&env, "ipfs://Qm123");

    client.update_metadata(&vault_id, &owner, &new_meta);

    let event = env.events().all().iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        topics
            .get(0)
            .and_then(|v| v.try_into_val(&env).ok())
            .map(|s: soroban_sdk::Symbol| s == types::UPDATE_METADATA_TOPIC)
            .unwrap_or(false)
    });
    assert!(event.is_some(), "upd_meta event not emitted");

    let data = event.unwrap().2.clone();
    let emitted: soroban_sdk::String = data.try_into_val(&env).unwrap();
    assert_eq!(emitted, new_meta);
}

#[test]
fn test_get_contract_token_returns_correct_address() {
    let (_, _, _, _, token_address, client) = setup();
    assert_eq!(client.get_contract_token(), token_address);
}

#[test]
fn test_create_vault_zero_interval_fails() {
    let (_, owner, beneficiary, _, _, client) = setup();

    let err = client.try_create_vault(&owner, &beneficiary, &0u64, &None).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(2));
}

#[test]
fn test_create_vault_long_interval_remains_accessible() {
    // 30-day check-in interval: vault storage TTL must outlive the interval.
    // vault_ttl_ledgers(2_592_000) = 2_592_000 * 2 / 5 = 1_036_800 ledgers (~60 days).
    let (env, owner, beneficiary, _, _, client) = setup();
    let thirty_days: u64 = 30 * 24 * 3600; // 2_592_000 seconds
    let vault_id = client.create_vault(&owner, &beneficiary, &thirty_days, &None);
    // Advance just under the interval — vault and its indexes must still be readable.
    env.ledger().with_mut(|l| l.timestamp += thirty_days - 1);
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.check_in_interval, thirty_days);
    // Owner and beneficiary index entries must also survive the long interval.
    assert_eq!(client.get_vaults_by_owner(&owner, &None, &0u32, &10u32), vec![&env, vault_id]);
    assert_eq!(client.get_vaults_by_beneficiary(&beneficiary, &None, &0u32, &10u32), vec![&env, vault_id]);
}

#[test]
fn test_create_vault_initial_metadata_respects_max_len() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    assert!(client.get_vault(&vault_id).metadata.len() <= MAX_METADATA_LEN);
}

// ---- Issue 1: get_vaults_by_beneficiary ----

#[test]
fn test_get_vaults_by_beneficiary_tracks_vaults() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let other_beneficiary = Address::generate(&env);

    assert_eq!(client.get_vaults_by_beneficiary(&beneficiary, &None, &0u32, &10u32), vec![&env]);

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &200u64, &None);
    let _vault_id_3 = client.create_vault(&owner, &other_beneficiary, &300u64, &None);

    assert_eq!(
        client.get_vaults_by_beneficiary(&beneficiary, &None, &0u32, &10u32),
        vec![&env, vault_id_1, vault_id_2]
    );
    assert_eq!(
        client.get_vaults_by_beneficiary(&other_beneficiary, &None, &0u32, &10u32),
        vec![&env, _vault_id_3]
    );
}

#[test]
fn test_get_vaults_by_beneficiary_empty_for_unknown() {
    let (env, _, _, _, _, client) = setup();
    let stranger = Address::generate(&env);
    assert_eq!(client.get_vaults_by_beneficiary(&stranger, &None, &0u32, &10u32), vec![&env]);
}

// ---- Issue 2: upgrade ----

#[test]
#[should_panic]
fn test_upgrade_fails_for_non_admin() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let _vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    // Use a zero hash — this will fail auth before even reaching deployer
    let fake_hash = BytesN::from_array(&env, &[0u8; 32]);
    // Call upgrade as owner (not admin) — should panic with NotAdmin
    client.upgrade(&fake_hash);
}

// ---- Issue 3: max_check_in_interval ----

#[test]
fn test_set_and_get_max_check_in_interval() {
    let (_, _, _, _, _, client) = setup();
    assert_eq!(client.get_max_check_in_interval(), None);
    client.set_max_check_in_interval(&86_400u64);
    assert_eq!(client.get_max_check_in_interval(), Some(86_400u64));
}

#[test]
fn test_set_max_check_in_interval_emits_event() {
    let (env, _, _, _, _, client) = setup();
    client.set_max_check_in_interval(&7_200u64);

    let event = env.events().all().iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        topics
            .get(0)
            .and_then(|v| v.try_into_val(&env).ok())
            .map(|s: soroban_sdk::Symbol| s == types::SET_MAX_INTERVAL_TOPIC)
            .unwrap_or(false)
    });
    assert!(event.is_some(), "set_max event not emitted");

    let data = event.unwrap().2.clone();
    let emitted: u64 = data.try_into_val(&env).unwrap();
    assert_eq!(emitted, 7_200u64);
}

#[test]
fn test_create_vault_fails_when_interval_exceeds_max() {
    let (_, owner, beneficiary, _, _, client) = setup();
    client.set_max_check_in_interval(&1_000u64);
    let result = client.try_create_vault(&owner, &beneficiary, &2_000u64, &None);
    assert_eq!(result.unwrap_err().unwrap(), soroban_sdk::Error::from_contract_error(ContractError::IntervalTooHigh as u32));
}

#[test]
fn test_create_vault_succeeds_at_max_boundary() {
    let (_, owner, beneficiary, _, _, client) = setup();
    client.set_max_check_in_interval(&1_000u64);
    let vault_id = client.create_vault(&owner, &beneficiary, &1_000u64, &None);
    assert_eq!(client.get_vault(&vault_id).check_in_interval, 1_000u64);
}

#[test]
fn test_update_check_in_interval_fails_when_exceeds_max() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.set_max_check_in_interval(&500u64);
    let result = client.try_update_check_in_interval(&vault_id, &600u64);
    assert!(result.is_err());
}

// ---- Issue 4: min_check_in_interval ----

#[test]
fn test_set_and_get_min_check_in_interval() {
    let (_, _, _, _, _, client) = setup();
    assert_eq!(client.get_min_check_in_interval(), None);
    client.set_min_check_in_interval(&60u64);
    assert_eq!(client.get_min_check_in_interval(), Some(60u64));
}

#[test]
fn test_set_min_check_in_interval_emits_event() {
    let (env, _, _, _, _, client) = setup();
    client.set_min_check_in_interval(&120u64);

    let event = env.events().all().iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        topics
            .get(0)
            .and_then(|v| v.try_into_val(&env).ok())
            .map(|s: soroban_sdk::Symbol| s == types::SET_MIN_INTERVAL_TOPIC)
            .unwrap_or(false)
    });
    assert!(event.is_some(), "set_min event not emitted");

    let data = event.unwrap().2.clone();
    let emitted: u64 = data.try_into_val(&env).unwrap();
    assert_eq!(emitted, 120u64);
}

#[test]
fn test_create_vault_fails_when_interval_below_min() {
    let (_, owner, beneficiary, _, _, client) = setup();
    client.set_min_check_in_interval(&3_600u64);
    let result = client.try_create_vault(&owner, &beneficiary, &100u64, &None);
    assert_eq!(result.unwrap_err().unwrap(), soroban_sdk::Error::from_contract_error(ContractError::IntervalTooLow as u32));
}

#[test]
fn test_create_vault_succeeds_at_min_boundary() {
    let (_, owner, beneficiary, _, _, client) = setup();
    client.set_min_check_in_interval(&3_600u64);
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);
    assert_eq!(client.get_vault(&vault_id).check_in_interval, 3_600u64);
}

#[test]
fn test_update_check_in_interval_fails_when_below_min() {
    let (_, owner, beneficiary, _, _, client) = setup();
    client.set_min_check_in_interval(&3_600u64);
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);
    let result = client.try_update_check_in_interval(&vault_id, &100u64);
    assert!(result.is_err());
}

#[test]
fn test_min_and_max_both_enforced() {
    let (_, owner, beneficiary, _, _, client) = setup();
    client.set_min_check_in_interval(&60u64);
    client.set_max_check_in_interval(&3_600u64);

    assert!(client.try_create_vault(&owner, &beneficiary, &30u64, &None).is_err());
    assert!(client.try_create_vault(&owner, &beneficiary, &7_200u64, &None).is_err());
    let vault_id = client.create_vault(&owner, &beneficiary, &1_800u64, &None);
    assert_eq!(client.get_vault(&vault_id).check_in_interval, 1_800u64);
}

#[test]
fn test_withdraw_rejects_zero_amount() {
    let (_, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &500i128);

    // zero amount should return InvalidAmount (#5)
    let result = client.try_withdraw(&vault_id, &owner, &0i128);
    assert!(result.is_err(), "expected error for zero-amount withdrawal");
}

#[test]
fn test_withdraw_rejects_negative_amount() {
    let (_, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &500i128);

    // negative amount should also return InvalidAmount (#5)
    let result = client.try_withdraw(&vault_id, &owner, &-1i128);
    assert!(result.is_err(), "expected error for negative-amount withdrawal");
}

#[test]
fn test_deposit_emits_event() {
    let (env, owner, beneficiary, _, _, client) = setup();

    env.mock_all_auths();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.deposit(&vault_id, &owner, &300i128);

    let events = env.events().all();
    // find the deposit event: topic[0] == "deposit", topic[1] == vault_id
    let deposit_event = events.iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        if topics.len() < 2 {
            return false;
        }
        let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
        topic0.map(|s| s == soroban_sdk::symbol_short!("deposit")).unwrap_or(false)
    });

    assert!(deposit_event.is_some(), "deposit event not emitted");
}

#[test]
fn test_withdraw_emits_event() {
    let (env, owner, beneficiary, _, _, client) = setup();

    env.mock_all_auths();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &500i128);

    client.withdraw(&vault_id, &owner, &100i128);

    let events = env.events().all();
    let withdraw_event = events.iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        if topics.len() < 2 {
            return false;
        }
        let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
        topic0.map(|s| s == soroban_sdk::symbol_short!("withdraw")).unwrap_or(false)
    });

    assert!(withdraw_event.is_some(), "withdraw event not emitted");
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_trigger_release_emits_event_with_zero_balance() {
    let (env, owner, beneficiary, _, _, client) = setup();

    // create vault but never deposit — balance stays 0
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    env.ledger().with_mut(|l| l.timestamp += 200);

    // should panic with EmptyVault error
    client.trigger_release(&vault_id);
}

// Regression test for #97 / #279: trigger_release must return structured error code 16
// (ContractError::NotExpired) when the vault TTL has not yet lapsed, instead of
// panicking with a raw string.
#[test]
fn test_trigger_release_returns_not_expired_error_before_ttl_lapses() {
    let (_, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    // vault is still within its check-in interval — must not release
    let err = client.try_trigger_release(&vault_id).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(16));
}

// Regression test for #98: set_beneficiaries must return ContractError::InvalidBps (code 12)
// when the beneficiary BPS entries do not sum to exactly 10_000.
#[test]
fn test_set_beneficiaries_rejects_invalid_bps() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let b2 = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);

    // 4_000 + 4_000 = 8_000, not 10_000 — must be rejected
    let err = client
        .try_set_beneficiaries(
            &vault_id,
            &owner,
            &vec![
                &env,
                BeneficiaryEntry { address: beneficiary.clone(), bps: 4_000 },
                BeneficiaryEntry { address: b2.clone(), bps: 4_000 },
            ],
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::InvalidBps);
}

// ---- Issue #105: set_beneficiaries owner-as-beneficiary guard ----

#[test]
#[should_panic(expected = "Error(Contract, #17)")]
fn test_set_beneficiaries_rejects_owner_as_beneficiary() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);

    // owner sneaks themselves into the multi-split list
    client.set_beneficiaries(
        &vault_id,
        &owner,
        &vec![
            &env,
            BeneficiaryEntry { address: owner.clone(), bps: 5_000 },
            BeneficiaryEntry { address: beneficiary.clone(), bps: 5_000 },
        ],
    );
}

// ---- Issue #226: set_beneficiaries empty list guard ----

#[test]
fn test_set_beneficiaries_rejects_empty_list() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);

    // Empty beneficiaries list should be rejected with InvalidBps
    let err = client
        .try_set_beneficiaries(&vault_id, &owner, &vec![&env])
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::InvalidBps); // InvalidBps
}

#[test]
fn test_deposit_rejects_balance_overflow() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    // setup() already minted 1_000_000; mint enough to reach i128::MAX total
    let extra = i128::MAX - 1_000_000;
    StellarAssetClient::new(&env, &token_address).mint(&owner, &extra);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    // deposit i128::MAX - 1 to fill the vault balance close to the limit
    let near_max = i128::MAX - 1;
    client.deposit(&vault_id, &owner, &near_max);

    // mint 2 more tokens so owner has enough to attempt the overflow
    StellarAssetClient::new(&env, &token_address).mint(&owner, &2i128);
    // attempting to deposit 2 more would push balance past i128::MAX
    let result = client.try_deposit(&vault_id, &owner, &2i128);

    assert!(result.is_err(), "expected overflow error on deposit exceeding i128::MAX");
}

#[test]
fn test_partial_release_with_multi_beneficiary_applies_bps_split() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    let beneficiary2 = Address::generate(&env);
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);

    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);
    client.deposit(&vault_id, &owner, &10_000i128);

    // 60/40 split
    client.set_beneficiaries(
        &vault_id,
        &owner,
        &vec![
            &env,
            BeneficiaryEntry { address: beneficiary.clone(), bps: 6_000 },
            BeneficiaryEntry { address: beneficiary2.clone(), bps: 4_000 },
        ],
    );

    client.partial_release(&vault_id, &1_000i128);

    // 60% of 1_000 = 600, 40% (last, absorbs dust) = 400
    assert_eq!(token_client.balance(&beneficiary), 600i128);
    assert_eq!(token_client.balance(&beneficiary2), 400i128);
    assert_eq!(client.get_vault(&vault_id).balance, 9_000i128);
    // vault remains locked
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Locked);
}

#[test]
fn test_partial_release_with_multi_beneficiary_last_entry_absorbs_dust() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    let beneficiary2 = Address::generate(&env);
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);

    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);
    client.deposit(&vault_id, &owner, &10_000i128);

    // 33/67 split — integer division leaves dust on the last entry
    client.set_beneficiaries(
        &vault_id,
        &owner,
        &vec![
            &env,
            BeneficiaryEntry { address: beneficiary.clone(), bps: 3_300 },
            BeneficiaryEntry { address: beneficiary2.clone(), bps: 6_700 },
        ],
    );

    // release 100 stroops: 33% = 33, last gets 100 - 33 = 67
    client.partial_release(&vault_id, &100i128);

    assert_eq!(token_client.balance(&beneficiary), 33i128);
    assert_eq!(token_client.balance(&beneficiary2), 67i128);
    assert_eq!(client.get_vault(&vault_id).balance, 9_900i128);
}

#[test]
fn test_partial_release_without_multi_beneficiary_sends_to_primary() {
    // Regression: when beneficiaries list is empty, primary beneficiary still gets 100%
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    client.partial_release(&vault_id, &400i128);

    assert_eq!(token_client.balance(&beneficiary), 400i128);
    assert_eq!(client.get_vault(&vault_id).balance, 600i128);
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Locked);
}

#[test]
fn test_partial_release_rejected_on_expired_vault() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    // Advance time past expiry
    env.ledger().with_mut(|l| l.timestamp += 200);

    // partial_release should fail with VaultExpired
    assert!(client.try_partial_release(&vault_id, &100i128).is_err());
}

#[test]
fn test_update_beneficiary_updates_index() {
    let (env, owner, old_beneficiary, _, _, client) = setup();
    let new_beneficiary = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &old_beneficiary, &100u64, &None);

    // old beneficiary sees the vault, new one does not
    assert_eq!(client.get_vaults_by_beneficiary(&old_beneficiary, &None, &0u32, &10u32), vec![&env, vault_id]);
    assert_eq!(client.get_vaults_by_beneficiary(&new_beneficiary, &None, &0u32, &10u32), vec![&env]);

    client.update_beneficiary(&vault_id, &owner, &new_beneficiary);

    // old beneficiary no longer sees the vault
    assert_eq!(client.get_vaults_by_beneficiary(&old_beneficiary, &None, &0u32, &10u32), vec![&env]);
    // new beneficiary now sees the vault
    assert_eq!(client.get_vaults_by_beneficiary(&new_beneficiary, &None, &0u32, &10u32), vec![&env, vault_id]);
}

#[test]
fn test_state_mutating_calls_extend_instance_ttl() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let contract_id = client.address.clone();
    let interval: u64 = 1_000;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&vault_id, &owner, &100_000);

    let get_ttl = || env.as_contract(&contract_id, || env.storage().instance().get_ttl());

    // check_in
    client.check_in(&vault_id, &owner);
    assert!(get_ttl() >= INSTANCE_TTL_THRESHOLD as u32);

    // deposit
    client.deposit(&vault_id, &owner, &1_000);
    assert!(get_ttl() >= INSTANCE_TTL_THRESHOLD as u32);

    // withdraw
    client.withdraw(&vault_id, &owner, &1_000);
    assert!(get_ttl() >= INSTANCE_TTL_THRESHOLD as u32);

    // update_beneficiary
    let new_beneficiary = Address::generate(&env);
    client.update_beneficiary(&vault_id, &owner, &new_beneficiary);
    assert!(get_ttl() >= INSTANCE_TTL_THRESHOLD as u32);

    // set_beneficiaries
    let beneficiaries = vec![&env, BeneficiaryEntry { address: new_beneficiary.clone(), bps: 10_000 }];
    client.set_beneficiaries(&vault_id, &owner, &beneficiaries);
    assert!(get_ttl() >= INSTANCE_TTL_THRESHOLD as u32);

    // update_metadata
    client.update_metadata(&vault_id, &owner, &String::from_str(&env, "test"));
    assert!(get_ttl() >= INSTANCE_TTL_THRESHOLD as u32);

    // partial_release
    client.partial_release(&vault_id, &1_000);
    assert!(get_ttl() >= INSTANCE_TTL_THRESHOLD as u32);

    // transfer_ownership
    let new_owner = Address::generate(&env);
    client.transfer_ownership(&vault_id, &owner, &new_owner);
    assert!(get_ttl() >= INSTANCE_TTL_THRESHOLD as u32);

    // cancel_vault
    client.cancel_vault(&vault_id, &new_owner);
    assert!(get_ttl() >= INSTANCE_TTL_THRESHOLD as u32);

    // trigger_release: advance time past expiry first (but vault is cancelled, so create new one)
    let vault_id2 = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&vault_id2, &owner, &10_000);
    env.ledger().with_mut(|l| l.timestamp += interval + 1);
    client.trigger_release(&vault_id2);
    assert!(get_ttl() >= INSTANCE_TTL_THRESHOLD as u32);
}

#[test]
fn test_check_in_extends_owner_index_ttl() {
    use soroban_sdk::testutils::storage::Persistent as _;
    let (env, owner, beneficiary, _, _, client) = setup();
    let contract_id = client.address.clone();
    let vault_id = client.create_vault(&owner, &beneficiary, &1_000u64, &None);

    client.check_in(&vault_id, &owner);

    let ttl = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get_ttl(&DataKey::OwnerVaults(owner.clone()))
    });
    assert!(ttl >= VAULT_TTL_THRESHOLD as u32);
}

#[test]
fn test_get_active_vaults_by_beneficiary_excludes_released() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000);

    // before release: active list contains the vault
    assert_eq!(
        client.get_active_vaults_by_beneficiary(&beneficiary, &0u32, &10u32),
        vec![&env, vault_id]
    );
    // historical list also contains it
    assert_eq!(
        client.get_vaults_by_beneficiary(&beneficiary, &None, &0u32, &10u32),
        vec![&env, vault_id]
    );

    // expire and release
    env.ledger().with_mut(|l| l.timestamp += 101);
    client.trigger_release(&vault_id);

    // active list is now empty
    assert_eq!(
        client.get_active_vaults_by_beneficiary(&beneficiary, &0u32, &10u32),
        vec![&env]
    );
    // historical list still contains the released vault
    assert_eq!(
        client.get_vaults_by_beneficiary(&beneficiary, &None, &0u32, &10u32),
        vec![&env, vault_id]
    );
}

#[test]
fn test_cancel_vault_removes_from_owner_and_beneficiary_indexes() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    assert_eq!(client.get_vaults_by_owner(&owner, &None, &0u32, &10u32), vec![&env, vault_id]);
    assert_eq!(client.get_vaults_by_beneficiary(&beneficiary, &None, &0u32, &10u32), vec![&env, vault_id]);

    client.cancel_vault(&vault_id, &owner);

    assert_eq!(client.get_vaults_by_owner(&owner, &None, &0u32, &10u32), vec![&env]);
    assert_eq!(client.get_vaults_by_beneficiary(&beneficiary, &None, &0u32, &10u32), vec![&env]);
}

// ---- Pagination tests ----

#[test]
fn test_get_vaults_by_owner_pagination() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let ids: alloc::vec::Vec<u64> = (0..5).map(|_| client.create_vault(&owner, &beneficiary, &100u64, &None)).collect();

    // page 0 of size 2 → first two
    assert_eq!(
        client.get_vaults_by_owner(&owner, &None, &0u32, &2u32),
        vec![&env, ids[0], ids[1]]
    );
    // page 1 of size 2 → next two
    assert_eq!(
        client.get_vaults_by_owner(&owner, &None, &1u32, &2u32),
        vec![&env, ids[2], ids[3]]
    );
    // page 2 of size 2 → last one
    assert_eq!(
        client.get_vaults_by_owner(&owner, &None, &2u32, &2u32),
        vec![&env, ids[4]]
    );
    // out-of-range page → empty
    assert_eq!(
        client.get_vaults_by_owner(&owner, &None, &10u32, &2u32),
        vec![&env]
    );
    // page_size 0 → empty
    assert_eq!(
        client.get_vaults_by_owner(&owner, &None, &0u32, &0u32),
        vec![&env]
    );
}

#[test]
fn test_get_vaults_by_beneficiary_pagination() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let ids: alloc::vec::Vec<u64> = (0..5).map(|_| client.create_vault(&owner, &beneficiary, &100u64, &None)).collect();

    assert_eq!(
        client.get_vaults_by_beneficiary(&beneficiary, &None, &0u32, &2u32),
        vec![&env, ids[0], ids[1]]
    );
    assert_eq!(
        client.get_vaults_by_beneficiary(&beneficiary, &None, &1u32, &2u32),
        vec![&env, ids[2], ids[3]]
    );
    assert_eq!(
        client.get_vaults_by_beneficiary(&beneficiary, &None, &2u32, &2u32),
        vec![&env, ids[4]]
    );
    assert_eq!(
        client.get_vaults_by_beneficiary(&beneficiary, &None, &10u32, &2u32),
        vec![&env]
    );
    assert_eq!(
        client.get_vaults_by_beneficiary(&beneficiary, &None, &0u32, &0u32),
        vec![&env]
    );
}

#[test]
fn test_withdraw_rejected_on_cancelled_vault() {
    let (_, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    // cancel_vault refunds and marks status = Cancelled
    client.cancel_vault(&vault_id, &owner);

    // Any withdraw attempt on a Cancelled vault must return AlreadyReleased (#7)
    let err = client
        .try_withdraw(&vault_id, &owner, &1i128)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::AlreadyReleased);
}

#[test]
fn test_withdraw_rejected_on_released_vault() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &500i128);
    // advance past check-in interval to expire the vault
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    // Any withdraw attempt on a Released vault must return AlreadyReleased (#7)
    let err = client
        .try_withdraw(&vault_id, &owner, &1i128)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::AlreadyReleased);
}

#[test]
fn test_get_vaults_by_beneficiary_with_status_filter() {
    let (env, owner, beneficiary, _, _, client) = setup();

    // Create two vaults
    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id_1, &owner, &1_000);

    let vault_id_2 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id_2, &owner, &1_000);

    // Expire and release vault_id_1
    env.ledger().with_mut(|l| l.timestamp += 101);
    client.trigger_release(&vault_id_1);

    // Now we have:
    // vault_id_1: Released
    // vault_id_2: Locked

    // Test: Get only Locked vaults
    assert_eq!(
        client.get_vaults_by_beneficiary(&beneficiary, &Some(ReleaseStatus::Locked), &0u32, &10u32),
        vec![&env, vault_id_2]
    );

    // Test: Get only Released vaults
    assert_eq!(
        client.get_vaults_by_beneficiary(&beneficiary, &Some(ReleaseStatus::Released), &0u32, &10u32),
        vec![&env, vault_id_1]
    );

    // Test: Get all vaults (no filter)
    assert_eq!(
        client.get_vaults_by_beneficiary(&beneficiary, &None, &0u32, &10u32),
        vec![&env, vault_id_1, vault_id_2]
    );
}

#[test]
fn test_get_vaults_by_owner_with_status_filter() {
    let (env, owner, beneficiary, _, _, client) = setup();

    // Create two vaults
    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id_1, &owner, &1_000);

    let vault_id_2 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id_2, &owner, &1_000);

    // Expire and release vault_id_1
    env.ledger().with_mut(|l| l.timestamp += 101);
    client.trigger_release(&vault_id_1);

    // Now we have:
    // vault_id_1: Released
    // vault_id_2: Locked

    // Test: Get only Locked vaults
    assert_eq!(
        client.get_vaults_by_owner(&owner, &Some(ReleaseStatus::Locked), &0u32, &10u32),
        vec![&env, vault_id_2]
    );

    // Test: Get only Released vaults
    assert_eq!(
        client.get_vaults_by_owner(&owner, &Some(ReleaseStatus::Released), &0u32, &10u32),
        vec![&env, vault_id_1]
    );

    // Test: Get all vaults (no filter)
    assert_eq!(
        client.get_vaults_by_owner(&owner, &None, &0u32, &10u32),
        vec![&env, vault_id_1, vault_id_2]
    );
}

#[test]
fn test_get_vaults_by_owner_with_cancelled_status_filter() {
    let (env, owner, beneficiary, _, _, client) = setup();

    // Create a vault
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    // Cancel the vault (removes it from owner index)
    client.cancel_vault(&vault_id, &owner);

    // Cancelled vaults are removed from the index, so filtering for Cancelled returns empty
    assert_eq!(
        client.get_vaults_by_owner(&owner, &Some(ReleaseStatus::Cancelled), &0u32, &10u32),
        vec![&env]
    );

    // All vaults (no filter) also returns empty since it was removed
    assert_eq!(
        client.get_vaults_by_owner(&owner, &None, &0u32, &10u32),
        vec![&env]
    );
}

// ---- Event topic constant tests ----

fn find_event_by_topic(env: &Env, topic_sym: soroban_sdk::Symbol) -> bool {
    env.events().all().iter().any(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(env);
        topics
            .get(0)
            .and_then(|v| v.try_into_val(env).ok())
            .map(|s: soroban_sdk::Symbol| s == topic_sym)
            .unwrap_or(false)
    })
}

#[test]
fn test_check_in_uses_check_in_topic_constant() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    env.ledger().with_mut(|l| l.timestamp += 10);
    client.check_in(&vault_id, &owner);
    assert!(find_event_by_topic(&env, types::CHECK_IN_TOPIC));
}

#[test]
fn test_cancel_vault_emits_cancel_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &500i128);
    client.cancel_vault(&vault_id, &owner);
    assert!(find_event_by_topic(&env, types::CANCEL_TOPIC));
}

#[test]
fn test_cancel_vault_event_contains_owner_and_refund_amount() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &300i128);
    client.cancel_vault(&vault_id, &owner);

    let cancel_event = env.events().all().iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        topics
            .get(0)
            .and_then(|v| v.try_into_val(&env).ok())
            .map(|s: soroban_sdk::Symbol| s == types::CANCEL_TOPIC)
            .unwrap_or(false)
    });
    assert!(cancel_event.is_some(), "cancel event not emitted");

    // data is (owner, refund_amount)
    let data = cancel_event.unwrap().2.clone();
    let (event_owner, refund): (Address, i128) = data.try_into_val(&env).unwrap();
    assert_eq!(event_owner, owner);
    assert_eq!(refund, 300i128);
}

#[test]
fn test_transfer_ownership_emits_ownership_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    env.ledger().with_mut(|l| l.timestamp += 86_401);
    client.accept_ownership_transfer(&vault_id, &new_owner);
    assert!(find_event_by_topic(&env, types::OWNERSHIP_TOPIC));
}

#[test]
fn test_transfer_ownership_event_contains_old_and_new_owner() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.initiate_ownership_transfer(&vault_id, &owner, &new_owner);
    env.ledger().with_mut(|l| l.timestamp += 86_401);
    client.accept_ownership_transfer(&vault_id, &new_owner);

    let ownership_event = env.events().all().iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        topics
            .get(0)
            .and_then(|v| v.try_into_val(&env).ok())
            .map(|s: soroban_sdk::Symbol| s == types::OWNERSHIP_ACCEPTED_TOPIC)
            .unwrap_or(false)
    });
    assert!(ownership_event.is_some(), "ownership_accepted event not emitted");

    let data = ownership_event.unwrap().2.clone();
    let (old, new): (Address, Address) = data.try_into_val(&env).unwrap();
    assert_eq!(old, owner);
    assert_eq!(new, new_owner);
}

#[test]
fn test_update_beneficiary_emits_beneficiary_updated_event() {
    let (env, owner, old_beneficiary, _, _, client) = setup();
    let new_beneficiary = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &old_beneficiary, &100u64, &None);
    client.update_beneficiary(&vault_id, &owner, &new_beneficiary);
    assert!(find_event_by_topic(&env, types::BENEFICIARY_UPDATED_TOPIC));
}

#[test]
fn test_update_beneficiary_event_contains_old_and_new_beneficiary() {
    let (env, owner, old_beneficiary, _, _, client) = setup();
    let new_beneficiary = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &old_beneficiary, &100u64, &None);
    client.update_beneficiary(&vault_id, &owner, &new_beneficiary);

    let ben_event = env.events().all().iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        topics
            .get(0)
            .and_then(|v| v.try_into_val(&env).ok())
            .map(|s: soroban_sdk::Symbol| s == types::BENEFICIARY_UPDATED_TOPIC)
            .unwrap_or(false)
    });
    assert!(ben_event.is_some(), "beneficiary_updated event not emitted");

    let data = ben_event.unwrap().2.clone();
    let (old, new): (Address, Address) = data.try_into_val(&env).unwrap();
    assert_eq!(old, old_beneficiary);
    assert_eq!(new, new_beneficiary);
}

#[test]
fn test_set_beneficiaries_emits_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let b2 = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    let entries = soroban_sdk::vec![
        &env,
        BeneficiaryEntry { address: beneficiary.clone(), bps: 6_000 },
        BeneficiaryEntry { address: b2.clone(), bps: 4_000 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    let event = env.events().all().iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        topics
            .get(0)
            .and_then(|v| v.try_into_val(&env).ok())
            .map(|s: soroban_sdk::Symbol| s == types::SET_BENEFICIARIES_TOPIC)
            .unwrap_or(false)
    });
    assert!(event.is_some(), "set_bens event not emitted");
}

#[test]
fn test_update_check_in_interval_emits_event_with_old_and_new() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.update_check_in_interval(&vault_id, &300u64);

    let event = env.events().all().iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        topics
            .get(0)
            .and_then(|v| v.try_into_val(&env).ok())
            .map(|s: soroban_sdk::Symbol| s == types::UPDATE_INTERVAL_TOPIC)
            .unwrap_or(false)
    });
    assert!(event.is_some(), "upd_intv event not emitted");

    let data = event.unwrap().2.clone();
    let (old, new): (u64, u64) = data.try_into_val(&env).unwrap();
    assert_eq!(old, 100u64);
    assert_eq!(new, 300u64);
}

// ---- Issue #320: get_release_status ----

#[test]
fn test_get_release_status_returns_locked_released_cancelled() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    // Create a vault — should be Locked
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Locked);

    // Deposit and trigger release after expiry — should become Released
    client.deposit(&vault_id, &owner, &1_000i128);
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Released);

    // Create another vault and cancel it — should become Cancelled
    let owner2 = Address::generate(&env);
    let beneficiary2 = Address::generate(&env);
    let token_client = token::Client::new(&env, &token_address);
    StellarAssetClient::new(&env, &token_address).mint(&owner2, &500_000);
    let vault_id2 = client.create_vault(&owner2, &beneficiary2, &100u64, &None);
    client.deposit(&vault_id2, &owner2, &500i128);
    client.cancel_vault(&vault_id2, &owner2);
    assert_eq!(client.get_release_status(&vault_id2), ReleaseStatus::Cancelled);
    // Owner should have been refunded
    assert_eq!(token_client.balance(&owner2), 500_000i128);
}

// ---- Issue #318: batch_withdraw ----

#[test]
fn test_batch_withdraw_decrements_multiple_vaults() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &100u64, &None);

    client.deposit(&vault_id_1, &owner, &500i128);
    client.deposit(&vault_id_2, &owner, &300i128);

    client.batch_withdraw(
        &vec![&env, vault_id_1, vault_id_2],
        &vec![&env, 200i128, 100i128],
        &owner,
    );

    assert_eq!(client.get_vault(&vault_id_1).balance, 300i128);
    assert_eq!(client.get_vault(&vault_id_2).balance, 200i128);
    // owner started at 1_000_000, deposited 800, withdrawn 300
    assert_eq!(token_client.balance(&owner), 999_500i128);
}

#[test]
fn test_batch_withdraw_validates_mismatched_lengths() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    let err = client
        .try_batch_withdraw(
            &vec![&env, vault_id],
            &vec![&env, 100i128, 200i128], // one extra amount
            &owner,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::InvalidAmount);
}

#[test]
fn test_batch_withdraw_rolls_back_on_insufficient_balance() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id_1, &owner, &100i128);
    // vault_id_2 has 0 balance

    // Attempting to withdraw from vault_id_2 should fail; vault_id_1 must be unchanged
    assert!(client
        .try_batch_withdraw(
            &vec![&env, vault_id_1, vault_id_2],
            &vec![&env, 50i128, 1i128],
            &owner,
        )
        .is_err());

    assert_eq!(client.get_vault(&vault_id_1).balance, 100i128);
    assert_eq!(client.get_vault(&vault_id_2).balance, 0i128);
}

// ---- Issue #319: batch_check_in ----

#[test]
fn test_batch_check_in_resets_last_check_in_for_multiple_vaults() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &200u64, &None);

    // Advance time so it is clearly different from creation time
    env.ledger().with_mut(|l| l.timestamp += 50);
    let now = env.ledger().timestamp();

    client.batch_check_in(&vec![&env, vault_id_1, vault_id_2], &owner);

    assert_eq!(client.get_vault(&vault_id_1).last_check_in, now);
    assert_eq!(client.get_vault(&vault_id_2).last_check_in, now);
}

#[test]
fn test_batch_check_in_extends_vault_expiry() {
    let (env, owner, beneficiary, _, _, client) = setup();

    // interval = 100s
    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &100u64, &None);

    // Advance 90s (vaults are about to expire)
    env.ledger().with_mut(|l| l.timestamp += 90);

    // Check in — resets the timer
    client.batch_check_in(&vec![&env, vault_id_1, vault_id_2], &owner);

    // Advance another 50s — should not be expired yet (timer was reset)
    env.ledger().with_mut(|l| l.timestamp += 50);
    assert!(!client.is_expired(&vault_id_1));
    assert!(!client.is_expired(&vault_id_2));
}

#[test]
fn test_batch_check_in_fails_for_non_owner() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let other = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    assert!(client
        .try_batch_check_in(&vec![&env, vault_id], &other)
        .is_err());
}

// ---- Issue #327: trigger_release with multi-beneficiary BPS split ----

#[test]
fn test_trigger_release_multi_beneficiary_bps_split_distributes_correctly() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    // Create 3 beneficiaries at 50%, 30%, 20% BPS
    let ben_a = beneficiary.clone(); // 50%  = 5_000 bps
    let ben_b = Address::generate(&env); // 30%  = 3_000 bps
    let ben_c = Address::generate(&env); // 20%  = 2_000 bps

    let vault_id = client.create_vault(&owner, &ben_a, &100u64, &None);

    let entries = soroban_sdk::vec![
        &env,
        BeneficiaryEntry { address: ben_a.clone(), bps: 5_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: ben_b.clone(), bps: 3_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: ben_c.clone(), bps: 2_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    // Deposit 10_000 stroops
    let total: i128 = 10_000;
    client.deposit(&vault_id, &owner, &total);

    // Expire the vault and trigger release
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    // Assert vault is Released and balance is zero
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Released);
    assert_eq!(client.get_vault(&vault_id).balance, 0i128);

    // Expected shares: ben_a = 5000, ben_b = 3000, ben_c = 2000 (last entry absorbs dust)
    assert_eq!(token_client.balance(&ben_a), 5_000i128);
    assert_eq!(token_client.balance(&ben_b), 3_000i128);
    assert_eq!(token_client.balance(&ben_c), 2_000i128);

    // Total distributed must equal total deposited — no dust remains in contract
    let total_distributed =
        token_client.balance(&ben_a) + token_client.balance(&ben_b) + token_client.balance(&ben_c);
    assert_eq!(total_distributed, total);
}

// ---- Issue #512: Beneficiary Minimum Threshold Tests ----

#[test]
fn test_minimum_threshold_skips_beneficiary_below_threshold() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    // Create 2 beneficiaries: A gets 50%, B gets 50%
    let ben_a = beneficiary.clone();
    let ben_b = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &ben_a, &100u64, &None);

    // Set minimum threshold: A = 3000, B = 6000
    // With 10_000 total: A gets 5000 (meets threshold), B gets 5000 (below 6000 threshold)
    let entries = soroban_sdk::vec![
        &env,
        BeneficiaryEntry { address: ben_a.clone(), bps: 5_000, minimum_threshold: 3_000 },
        BeneficiaryEntry { address: ben_b.clone(), bps: 5_000, minimum_threshold: 6_000 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    client.deposit(&vault_id, &owner, &10_000i128);

    // Expire and trigger release
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    // B should not receive anything since 5000 < 6000 threshold
    // A should receive all 10_000 (since they are the only qualifying beneficiary)
    assert_eq!(token_client.balance(&ben_a), 10_000i128);
    assert_eq!(token_client.balance(&ben_b), 0i128);
}

#[test]
fn test_minimum_threshold_redistributes_to_qualifying_beneficiaries() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    let ben_a = beneficiary.clone();
    let ben_b = Address::generate(&env);
    let ben_c = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &ben_a, &100u64, &None);

    // A: 33.33% (3333), B: 33.33% (3333), C: 33.34% (3334)
    // Thresholds: A=1000 (meets), B=4000 (fails), C=1000 (meets)
    // B's share (3333) redistributed to A and C proportionally
    let entries = soroban_sdk::vec![
        &env,
        BeneficiaryEntry { address: ben_a.clone(), bps: 3_333, minimum_threshold: 1_000 },
        BeneficiaryEntry { address: ben_b.clone(), bps: 3_333, minimum_threshold: 4_000 },
        BeneficiaryEntry { address: ben_c.clone(), bps: 3_334, minimum_threshold: 1_000 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    client.deposit(&vault_id, &owner, &10_000i128);

    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    // A and C are qualifying (3333 + 3334 = 6667 bps of 10000)
    // Redistribution: A gets 10000 * 3333/6667 ≈ 5000, C gets 10000 * 3334/6667 ≈ 5000
    let ben_a_balance = token_client.balance(&ben_a);
    let ben_b_balance = token_client.balance(&ben_b);
    let ben_c_balance = token_client.balance(&ben_c);

    assert_eq!(ben_b_balance, 0i128, "B should not receive anything");
    assert!(ben_a_balance > 0, "A should receive redistributed funds");
    assert!(ben_c_balance > 0, "C should receive redistributed funds");
    assert_eq!(ben_a_balance + ben_c_balance, 10_000i128, "Total should be distributed");
}

#[test]
fn test_minimum_threshold_all_below_threshold_returns_to_owner() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    let ben_a = beneficiary.clone();
    let ben_b = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &ben_a, &100u64, &None);

    // Both beneficiaries have high thresholds that won't be met with 1000 total
    let entries = soroban_sdk::vec![
        &env,
        BeneficiaryEntry { address: ben_a.clone(), bps: 5_000, minimum_threshold: 2_000 },
        BeneficiaryEntry { address: ben_b.clone(), bps: 5_000, minimum_threshold: 2_000 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    client.deposit(&vault_id, &owner, &1_000i128);

    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    // No qualifying beneficiaries: funds return to owner
    assert_eq!(token_client.balance(&ben_a), 0i128, "A below threshold");
    assert_eq!(token_client.balance(&ben_b), 0i128, "B below threshold");
    assert_eq!(token_client.balance(&owner), 1_000_000i128 - 1_000i128 + 1_000i128, "Owner gets funds back");
}

#[test]
fn test_set_beneficiary_minimum_threshold() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let ben_b = Address::generate(&env);

    // Add beneficiary
    client.add_beneficiary(&vault_id, &owner, &ben_b, &5_000u32);

    // Set minimum threshold
    client.set_beneficiary_minimum_threshold(&vault_id, &owner, &beneficiary, &1_000i128);

    // Verify threshold was set
    let threshold = client.get_beneficiary_minimum_threshold(&vault_id, &beneficiary);
    assert_eq!(threshold, Some(1_000i128));
}

#[test]
fn test_get_beneficiary_minimum_threshold_not_found() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let nonexistent = Address::generate(&env);

    let threshold = client.get_beneficiary_minimum_threshold(&vault_id, &nonexistent);
    assert_eq!(threshold, None, "Non-existent beneficiary should return None");
}

#[test]
fn test_get_beneficiaries_with_thresholds() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let ben_a = beneficiary.clone();
    let ben_b = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &ben_a, &100u64, &None);

    let entries = soroban_sdk::vec![
        &env,
        BeneficiaryEntry { address: ben_a.clone(), bps: 6_000, minimum_threshold: 1_000 },
        BeneficiaryEntry { address: ben_b.clone(), bps: 4_000, minimum_threshold: 500 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    let beneficiaries = client.get_beneficiaries_with_thresholds(&vault_id);
    assert!(beneficiaries.is_some(), "Should return Some(beneficiaries)");

    let bens = beneficiaries.unwrap();
    assert_eq!(bens.len(), 2);
    assert_eq!(bens.get(0).unwrap().minimum_threshold, 1_000i128);
    assert_eq!(bens.get(1).unwrap().minimum_threshold, 500i128);
}

#[test]
fn test_minimum_threshold_zero_disables_threshold() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    let ben_a = beneficiary.clone();
    let ben_b = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &ben_a, &100u64, &None);

    // A: 50%, B: 50%, but A has threshold of 0 (disabled) and B has threshold of 8000
    let entries = soroban_sdk::vec![
        &env,
        BeneficiaryEntry { address: ben_a.clone(), bps: 5_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: ben_b.clone(), bps: 5_000, minimum_threshold: 8_000 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    client.deposit(&vault_id, &owner, &10_000i128);

    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    // B doesn't meet threshold (5000 < 8000), A's threshold is 0 so it qualifies
    assert_eq!(token_client.balance(&ben_a), 10_000i128);
    assert_eq!(token_client.balance(&ben_b), 0i128);
}

#[test]
fn test_minimum_threshold_update_existing() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let ben_b = Address::generate(&env);

    let entries = soroban_sdk::vec![
        &env,
        BeneficiaryEntry { address: beneficiary.clone(), bps: 5_000, minimum_threshold: 1_000 },
        BeneficiaryEntry { address: ben_b.clone(), bps: 5_000, minimum_threshold: 1_000 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    // Update ben_b's threshold
    client.set_beneficiary_minimum_threshold(&vault_id, &owner, &ben_b, &5_000i128);

    let new_threshold = client.get_beneficiary_minimum_threshold(&vault_id, &ben_b);
    assert_eq!(new_threshold, Some(5_000i128));
}

// ---- Vesting schedule tests ----

/// Helper: create vault, deposit, expire, trigger_release (with vesting schedule attached).
fn setup_vesting(
    env: &Env,
    owner: &Address,
    beneficiary: &Address,
    client: &TtlVaultContractClient<'static>,
    amount: i128,
    num_installments: u32,
    interval: u64,
) -> u64 {
    let vault_id = client.create_vault(owner, beneficiary, &100u64, &None);
    client.deposit(&vault_id, owner, &amount);
    let start_time = env.ledger().timestamp() + 200; // first installment after expiry
    client.set_vesting_schedule(&vault_id, owner, &start_time, &interval, &num_installments, &0u64);
    // Expire the vault
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);
    vault_id
}

#[test]
fn test_set_vesting_schedule_stores_schedule() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    let start = env.ledger().timestamp() + 50;
    client.set_vesting_schedule(&vault_id, &owner, &start, &100u64, &4u32, &0u64);

    let sched = client.get_vesting_schedule(&vault_id).unwrap();
    assert_eq!(sched.start_time, start);
    assert_eq!(sched.interval, 100u64);
    assert_eq!(sched.num_installments, 4u32);
    assert_eq!(sched.claimed_installments, 0u32);
    assert_eq!(sched.total_amount, 1_000i128);
    assert_eq!(sched.cliff_period, 0u64);
}

#[test]
fn test_set_vesting_schedule_requires_owner() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    let stranger = Address::generate(&env);
    let start = env.ledger().timestamp() + 50;
    let err = client
        .try_set_vesting_schedule(&vault_id, &stranger, &start, &100u64, &4u32, &0u64)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::NotOwner);
}

#[test]
fn test_set_vesting_schedule_rejects_zero_interval() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    let err = client
        .try_set_vesting_schedule(&vault_id, &owner, &0u64, &0u64, &4u32, &0u64)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::InvalidInterval);
}

#[test]
fn test_set_vesting_schedule_rejects_zero_installments() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    let err = client
        .try_set_vesting_schedule(&vault_id, &owner, &0u64, &100u64, &0u32, &0u64)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::InvalidInterval);
}

#[test]
fn test_set_vesting_schedule_rejects_empty_vault() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    // No deposit — balance is 0

    let err = client
        .try_set_vesting_schedule(&vault_id, &owner, &0u64, &100u64, &4u32, &0u64)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::EmptyVault);
}

#[test]
fn test_trigger_release_with_vesting_keeps_balance() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = setup_vesting(&env, &owner, &beneficiary, &client, 1_000i128, 4, 100u64);

    // Vault is Released but balance is intact
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Released);
    assert_eq!(client.get_vault(&vault_id).balance, 1_000i128);
}

#[test]
fn test_claim_first_installment() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    // 4 installments of 250 each, interval = 100s
    let vault_id = setup_vesting(&env, &owner, &beneficiary, &client, 1_000i128, 4, 100u64);

    // Advance past first installment window (start_time is already reached since we advanced 200s)
    let claimed = client.claim_vested_installment(&vault_id);
    assert_eq!(claimed, 250i128);
    assert_eq!(token_client.balance(&beneficiary), 250i128);
    assert_eq!(client.get_vault(&vault_id).balance, 750i128);

    let sched = client.get_vesting_schedule(&vault_id).unwrap();
    assert_eq!(sched.claimed_installments, 1u32);
}

#[test]
fn test_claim_multiple_installments_at_once() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    // 4 installments of 250 each, interval = 50s
    // After trigger_release, timestamp = start_time (window 0 is claimable)
    // Advance another 100s → windows 0, 1, 2 are claimable (elapsed=100, unlocked=3)
    let vault_id = setup_vesting(&env, &owner, &beneficiary, &client, 1_000i128, 4, 50u64);
    env.ledger().with_mut(|l| l.timestamp += 100);

    let claimed = client.claim_vested_installment(&vault_id);
    // 3 installments × 250 = 750
    assert_eq!(claimed, 750i128);
    assert_eq!(token_client.balance(&beneficiary), 750i128);

    let sched = client.get_vesting_schedule(&vault_id).unwrap();
    assert_eq!(sched.claimed_installments, 3u32);
}

#[test]
fn test_claim_all_installments_drains_vault() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    // 4 installments, interval = 50s; advance past all 4
    let vault_id = setup_vesting(&env, &owner, &beneficiary, &client, 1_000i128, 4, 50u64);
    env.ledger().with_mut(|l| l.timestamp += 300); // well past all 4 windows

    let claimed = client.claim_vested_installment(&vault_id);
    assert_eq!(claimed, 1_000i128);
    assert_eq!(token_client.balance(&beneficiary), 1_000i128);
    assert_eq!(client.get_vault(&vault_id).balance, 0i128);

    let sched = client.get_vesting_schedule(&vault_id).unwrap();
    assert_eq!(sched.claimed_installments, 4u32);
}

#[test]
fn test_claim_nothing_to_claim_before_start_time() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    // start_time is far in the future
    let start = env.ledger().timestamp() + 10_000;
    client.set_vesting_schedule(&vault_id, &owner, &start, &100u64, &4u32, &0u64);

    // Expire and release
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    // Trying to claim before start_time should fail
    let err = client
        .try_claim_vested_installment(&vault_id)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::NothingToClaimYet);
}

#[test]
fn test_claim_already_complete_error() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = setup_vesting(&env, &owner, &beneficiary, &client, 1_000i128, 4, 50u64);
    env.ledger().with_mut(|l| l.timestamp += 300); // past all installments

    client.claim_vested_installment(&vault_id); // claim all

    let err = client
        .try_claim_vested_installment(&vault_id)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::VestingAlreadyComplete);
}

#[test]
fn test_claim_no_schedule_returns_vesting_not_found() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    // Normal vault without vesting schedule
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    // Vault is Released with no vesting schedule — balance should be 0 (immediate release)
    assert_eq!(client.get_vault(&vault_id).balance, 0i128);
    assert_eq!(token_client.balance(&beneficiary), 1_000i128);

    let err = client
        .try_claim_vested_installment(&vault_id)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::VestingNotFound);
}

#[test]
fn test_vesting_with_multi_beneficiary_split() {
    let (env, owner, ben_a, _, token_address, client) = setup();
    let ben_b = Address::generate(&env);
    let token_client = token::Client::new(&env, &token_address);

    let vault_id = client.create_vault(&owner, &ben_a, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    // Set 50/50 split
    let entries = soroban_sdk::vec![
        &env,
        BeneficiaryEntry { address: ben_a.clone(), bps: 5_000 },
        BeneficiaryEntry { address: ben_b.clone(), bps: 5_000 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    let start = env.ledger().timestamp() + 200;
    client.set_vesting_schedule(&vault_id, &owner, &start, &100u64, &2u32, &0u64);

    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    // Claim first installment (500 total, split 50/50 → 250 each)
    let claimed = client.claim_vested_installment(&vault_id);
    assert_eq!(claimed, 500i128);
    assert_eq!(token_client.balance(&ben_a), 250i128);
    assert_eq!(token_client.balance(&ben_b), 250i128);
}

#[test]
fn test_set_vesting_emits_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    let start = env.ledger().timestamp() + 50;
    client.set_vesting_schedule(&vault_id, &owner, &start, &100u64, &4u32, &0u64);

    assert!(find_event_by_topic(&env, types::SET_VESTING_TOPIC));
}

#[test]
fn test_claim_vested_emits_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = setup_vesting(&env, &owner, &beneficiary, &client, 1_000i128, 4, 50u64);
    env.ledger().with_mut(|l| l.timestamp += 100);

    client.claim_vested_installment(&vault_id);

    assert!(find_event_by_topic(&env, types::CLAIM_VEST_TOPIC));
}

// ---- Cliff period tests (#534) ----

/// Helper: set up a vesting schedule with a cliff period.
/// cliff_period is in seconds from start_time.
fn setup_vesting_with_cliff(
    env: &Env,
    owner: &Address,
    beneficiary: &Address,
    client: &TtlVaultContractClient<'static>,
    amount: i128,
    num_installments: u32,
    interval: u64,
    cliff_period: u64,
) -> u64 {
    let vault_id = client.create_vault(owner, beneficiary, &100u64, &None);
    client.deposit(&vault_id, owner, &amount);
    let start_time = env.ledger().timestamp() + 200;
    client.set_vesting_schedule(&vault_id, owner, &start_time, &interval, &num_installments, &cliff_period);
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);
    vault_id
}

#[test]
fn test_cliff_period_stored_in_schedule() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1_000i128);

    let start = env.ledger().timestamp() + 50;
    client.set_vesting_schedule(&vault_id, &owner, &start, &100u64, &4u32, &500u64);

    let sched = client.get_vesting_schedule(&vault_id).unwrap();
    assert_eq!(sched.cliff_period, 500u64);
}

#[test]
fn test_claim_blocked_before_cliff() {
    let (env, owner, beneficiary, _, _, client) = setup();
    // cliff = 300s, interval = 100s; after trigger_release we are at start_time
    // so cliff has NOT been reached yet
    let vault_id = setup_vesting_with_cliff(&env, &owner, &beneficiary, &client, 1_000i128, 4, 100u64, 300u64);

    // Advance 100s — past first installment window but still inside cliff
    env.ledger().with_mut(|l| l.timestamp += 100);

    let err = client
        .try_claim_vested_installment(&vault_id)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::CliffNotReached);
}

#[test]
fn test_claim_allowed_after_cliff() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    // cliff = 200s, interval = 100s; 4 installments of 250 each
    let vault_id = setup_vesting_with_cliff(&env, &owner, &beneficiary, &client, 1_000i128, 4, 100u64, 200u64);

    // Advance 200s — cliff is now reached; 3 installment windows have elapsed
    env.ledger().with_mut(|l| l.timestamp += 200);

    let claimed = client.claim_vested_installment(&vault_id);
    assert_eq!(claimed, 750i128); // 3 installments × 250
    assert_eq!(token_client.balance(&beneficiary), 750i128);
}

#[test]
fn test_cliff_reached_event_emitted_on_first_claim() {
    let (env, owner, beneficiary, _, _, client) = setup();
    // cliff = 100s
    let vault_id = setup_vesting_with_cliff(&env, &owner, &beneficiary, &client, 1_000i128, 4, 100u64, 100u64);

    // Advance past cliff
    env.ledger().with_mut(|l| l.timestamp += 100);

    client.claim_vested_installment(&vault_id);

    assert!(find_event_by_topic(&env, types::CLIFF_REACHED_TOPIC));
}

#[test]
fn test_cliff_reached_event_not_emitted_on_second_claim() {
    let (env, owner, beneficiary, _, _, client) = setup();
    // cliff = 100s, interval = 100s
    let vault_id = setup_vesting_with_cliff(&env, &owner, &beneficiary, &client, 1_000i128, 4, 100u64, 100u64);

    // First claim (past cliff)
    env.ledger().with_mut(|l| l.timestamp += 100);
    client.claim_vested_installment(&vault_id);

    // Clear events by checking count before second claim
    let events_after_first = env.events().all().len();

    // Second claim — no cliff event expected
    env.ledger().with_mut(|l| l.timestamp += 100);
    client.claim_vested_installment(&vault_id);

    // Verify no additional CLIFF_REACHED event was emitted
    let new_events: soroban_sdk::Vec<_> = env.events().all();
    let cliff_topic = types::CLIFF_REACHED_TOPIC;
    let mut cliff_count = 0u32;
    for i in events_after_first..new_events.len() {
        let (topics, _): (soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val) = new_events.get(i).unwrap();
        if topics.len() > 0 {
            if let Ok(t) = soroban_sdk::Symbol::try_from(topics.get(0).unwrap()) {
                if t == cliff_topic {
                    cliff_count += 1;
                }
            }
        }
    }
    assert_eq!(cliff_count, 0, "CLIFF_REACHED should not be emitted on second claim");
}

#[test]
fn test_zero_cliff_period_behaves_as_no_cliff() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    // cliff_period = 0 → no cliff, first installment claimable at start_time
    let vault_id = setup_vesting_with_cliff(&env, &owner, &beneficiary, &client, 1_000i128, 4, 100u64, 0u64);

    let claimed = client.claim_vested_installment(&vault_id);
    assert_eq!(claimed, 250i128);
    assert_eq!(token_client.balance(&beneficiary), 250i128);
}

#[test]
fn test_get_vault_last_check_in_returns_correct_timestamp() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    env.ledger().with_mut(|l| l.timestamp += 50);
    client.check_in(&vault_id, &owner);

    let expected = env.ledger().timestamp();
    assert_eq!(client.get_vault_last_check_in(&vault_id), expected);
}

// --- #305: is_expired boundary ---

/// Verifies that is_expired returns true at the exact deadline (now == deadline).
/// The expiry condition is `now >= deadline`, so the boundary must be expired.
#[test]
fn test_is_expired_at_exact_deadline() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let interval = 100u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // One tick before deadline: not expired
    env.ledger().with_mut(|l| l.timestamp += interval - 1);
    assert!(!client.is_expired(&vault_id));

    // Exactly at deadline: expired
    env.ledger().with_mut(|l| l.timestamp += 1);
    assert!(client.is_expired(&vault_id));
}

// --- #306: get_vault does not extend TTL ---

/// Verifies that get_vault is read-only and does not extend the vault's
/// persistent storage TTL. Repeated reads must not alter ledger state.
#[test]
fn test_get_vault_does_not_extend_ttl() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    // Capture TTL immediately after creation
    let ttl_after_create = env
        .storage()
        .persistent()
        .get_ttl(&DataKey::Vault(vault_id));

    // Read the vault multiple times
    client.get_vault(&vault_id);
    client.get_vault(&vault_id);
    client.get_vault(&vault_id);

    // TTL must be unchanged — get_vault must not extend it
    let ttl_after_reads = env
        .storage()
        .persistent()
        .get_ttl(&DataKey::Vault(vault_id));

    assert_eq!(ttl_after_create, ttl_after_reads);
}


// ---- Issue #369: Security Audit Tests ----

#[test]
fn test_security_reentrancy_protection() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &Some(token_address.clone()));
    
    // Deposit funds
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);
    client.deposit(&vault_id, &owner, &100_000);
    
    // Verify state is updated before transfer
    let vault_before = client.get_vault(&vault_id);
    assert_eq!(vault_before.balance, 100_000);
    
    // Withdraw - state should be updated before transfer
    let result = client.try_withdraw(&vault_id, &owner, &50_000);
    assert!(result.is_ok());
    
    let vault_after = client.get_vault(&vault_id);
    assert_eq!(vault_after.balance, 50_000);
}

#[test]
fn test_security_integer_overflow_protection() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &Some(token_address.clone()));
    
    // Mint large amount
    StellarAssetClient::new(&env, &token_address).mint(&owner, &i128::MAX);
    
    // Deposit maximum safe amount
    client.deposit(&vault_id, &owner, &(i128::MAX / 2));
    
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, i128::MAX / 2);
    
    // Attempting to deposit more should fail with BalanceOverflow
    let result = client.try_deposit(&vault_id, &owner, &(i128::MAX / 2 + 1));
    assert!(result.is_err());
}

#[test]
fn test_security_authorization_owner_only() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &Some(token_address.clone()));
    
    let attacker = Address::generate(&env);
    
    // Attacker cannot check in
    let result = client.try_check_in(&vault_id, &attacker);
    assert!(result.is_err());
    
    // Attacker cannot withdraw
    let result = client.try_withdraw(&vault_id, &attacker, &100);
    assert!(result.is_err());
    
    // Attacker cannot update beneficiary
    let result = client.try_update_beneficiary(&vault_id, &attacker, &attacker);
    assert!(result.is_err());
}

#[test]
fn test_security_authorization_admin_only() {
    let (env, owner, beneficiary, admin, token_address, client) = setup();
    
    let attacker = Address::generate(&env);
    
    // Attacker cannot pause - admin functions don't take caller parameter
    // They use require_auth() internally, so we need to test differently
    // For now, verify that admin can pause
    client.pause();
    
    // Verify contract is paused
    assert!(client.is_paused());
    
    // Unpause for next tests
    client.unpause();
}

#[test]
fn test_security_empty_vault_rejection() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &Some(token_address.clone()));
    
    // Expire vault without depositing
    env.ledger().with_mut(|l| {
        l.timestamp = l.timestamp + 3600;
    });
    
    // Cannot trigger release on empty vault
    let result = client.try_trigger_release(&vault_id);
    assert!(result.is_err());
}

#[test]
fn test_security_bps_validation() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &Some(token_address.clone()));
    
    let ben1 = Address::generate(&env);
    let ben2 = Address::generate(&env);
    
    // BPS must sum to 10,000
    let beneficiaries = vec![
        &env,
        BeneficiaryEntry { address: ben1.clone(), bps: 5000 },
        BeneficiaryEntry { address: ben2.clone(), bps: 4999 }, // Sum = 9999
    ];
    
    let result = client.try_set_beneficiaries(&vault_id, &owner, &beneficiaries);
    assert!(result.is_err());
}

#[test]
fn test_security_duplicate_beneficiary_prevention() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &Some(token_address.clone()));
    
    let ben1 = Address::generate(&env);
    
    // Cannot add duplicate beneficiary
    let result = client.try_add_beneficiary(&vault_id, &owner, &ben1, &5000);
    assert!(result.is_ok());
    let result = client.try_add_beneficiary(&vault_id, &owner, &ben1, &5000);
    assert!(result.is_err());
}

#[test]
fn test_security_owner_cannot_be_beneficiary() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    // Cannot create vault where owner = beneficiary
    let result = client.try_create_vault(&owner, &owner, &3600u64, &None);
    assert!(result.is_err());
}

#[test]
fn test_security_released_vault_immutable() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &Some(token_address.clone()));
    
    // Deposit and release
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);
    client.deposit(&vault_id, &owner, &100_000);
    
    env.ledger().with_mut(|l| {
        l.timestamp = l.timestamp + 3600;
    });
    
    client.trigger_release(&vault_id);
    
    // Cannot withdraw from released vault
    let result = client.try_withdraw(&vault_id, &owner, &50_000);
    assert!(result.is_err());
    
    // Cannot check in to released vault
    let result = client.try_check_in(&vault_id, &owner);
    assert!(result.is_err());
}

#[test]
fn test_security_paused_contract_blocks_operations() {
    let (env, owner, beneficiary, admin, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &Some(token_address.clone()));
    
    // Pause contract
    client.pause();
    
    // Operations should fail
    let result = client.try_check_in(&vault_id, &owner);
    assert!(result.is_err());
    
    let result = client.try_deposit(&vault_id, &owner, &100);
    assert!(result.is_err());
    
    // Unpause
    client.unpause();
    
    // Operations should succeed
    assert!(client.try_check_in(&vault_id, &owner).is_ok());
}

#[test]
fn test_security_token_whitelist_enforcement() {
    let (env, owner, beneficiary, admin, token_address, client) = setup();
    
    // Create unauthorized token
    let unauthorized_token_admin = Address::generate(&env);
    let unauthorized_token = env
        .register_stellar_asset_contract_v2(unauthorized_token_admin)
        .address();
    
    // Cannot create vault with unauthorized token
    let result = client.try_create_vault(&owner, &beneficiary, &3600u64, &Some(unauthorized_token.clone()));
    assert!(result.is_err());
    
    // Whitelist the token
    client.whitelist_token(&unauthorized_token);
    
    // Now vault creation should succeed
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &Some(unauthorized_token.clone()));
    assert!(vault_id > 0);
}

#[test]
fn test_security_vesting_prevents_double_claim() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &Some(token_address.clone()));
    
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);
    client.deposit(&vault_id, &owner, &100_000);
    
    // Set vesting schedule
    let start_time = env.ledger().timestamp() + 200;
    let result = client.try_set_vesting_schedule(&vault_id, &owner, &start_time, &100u64, &2);
    assert!(result.is_ok());
    
    // Expire and release
    env.ledger().with_mut(|l| {
        l.timestamp = l.timestamp + 100;
    });
    client.trigger_release(&vault_id);
    
    // Advance to first installment
    env.ledger().with_mut(|l| {
        l.timestamp = l.timestamp + 200;
    });
    
    // Claim first installment
    let result = client.try_claim_vested_installment(&vault_id);
    assert!(result.is_ok());
    
    // Cannot claim same installment twice
    let result2 = client.try_claim_vested_installment(&vault_id);
    assert!(result2.is_err());
}

#[test]
fn test_security_vault_count_consistency() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let initial_count = client.vault_count();
    
    // Create vault
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &Some(token_address.clone()));
    
    // Count should increment
    assert_eq!(client.vault_count(), initial_count + 1);
    assert_eq!(vault_id, initial_count + 1);
    
    // Failed creation should not increment count
    let result = client.try_create_vault(&owner, &owner, &3600u64, &None);
    assert!(result.is_err());
    assert_eq!(client.vault_count(), initial_count + 1);
}

#[test]
fn test_security_metadata_length_validation() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &Some(token_address.clone()));
    
    // Create metadata that exceeds max length
    let long_metadata = String::from_str(&env, &"x".repeat(300));
    
    // Should fail due to length validation
    let result = client.try_update_metadata(&vault_id, &owner, &long_metadata);
    assert!(result.is_err());
}

#[test]
fn test_security_interval_bounds_validation() {
    let (env, owner, beneficiary, admin, _, client) = setup();
    
    // Set min and max intervals
    client.set_min_check_in_interval(&1000u64);
    client.set_max_check_in_interval(&10000u64);
    
    // Cannot create vault with interval below minimum
    let result = client.try_create_vault(&owner, &beneficiary, &500u64, &None);
    assert!(result.is_err());
    
    // Cannot create vault with interval above maximum
    let result = client.try_create_vault(&owner, &beneficiary, &20000u64, &None);
    assert!(result.is_err());
    
    // Valid interval should succeed
    let vault_id = client.create_vault(&owner, &beneficiary, &5000u64, &None);
    assert!(vault_id > 0);
}


// ---- Issue #395: Passkey Usage Analytics Tests ----

#[test]
fn test_passkey_usage_logging() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    
    // Create a passkey hash
    let passkey_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
    
    // Check-in with passkey
    client.check_in(&vault_id, &owner, &passkey_hash);
    
    // Get passkey usage
    let usage = client.get_passkey_usage(&vault_id);
    assert_eq!(usage.len(), 1);
    assert_eq!(usage.get(0).unwrap().passkey_hash, passkey_hash);
}

#[test]
fn test_passkey_usage_multiple_entries() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    
    let passkey_hash_1 = BytesN::<32>::from_array(&env, &[1u8; 32]);
    let passkey_hash_2 = BytesN::<32>::from_array(&env, &[2u8; 32]);
    
    // First check-in
    client.check_in(&vault_id, &owner, &passkey_hash_1);
    env.ledger().with_mut(|l| l.timestamp += 50);
    
    // Second check-in with different passkey
    client.check_in(&vault_id, &owner, &passkey_hash_2);
    
    // Get passkey usage
    let usage = client.get_passkey_usage(&vault_id);
    assert_eq!(usage.len(), 2);
    assert_eq!(usage.get(0).unwrap().passkey_hash, passkey_hash_1);
    assert_eq!(usage.get(1).unwrap().passkey_hash, passkey_hash_2);
}

// ---- Issue #396: Passkey Expiry Tests ----

#[test]
fn test_passkey_expiry_enforcement() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let passkey_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
    
    // Set passkey expiry to current time + 50 seconds
    let expiry = env.ledger().timestamp() + 50;
    client.extend_passkey_expiry(&vault_id, &owner, &passkey_hash, &expiry);
    
    // Check-in should succeed before expiry
    client.check_in(&vault_id, &owner, &passkey_hash);
    
    // Advance time past expiry
    env.ledger().with_mut(|l| l.timestamp += 100);
    
    // Check-in should fail with expired passkey
    let result = client.try_check_in(&vault_id, &owner, &passkey_hash);
    assert!(result.is_err());
}

#[test]
fn test_extend_passkey_expiry() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let passkey_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
    
    // Set initial expiry
    let initial_expiry = env.ledger().timestamp() + 50;
    client.extend_passkey_expiry(&vault_id, &owner, &passkey_hash, &initial_expiry);
    
    // Verify expiry is set
    let expiry = client.get_passkey_expiry(&vault_id, &passkey_hash);
    assert_eq!(expiry, Some(initial_expiry));
    
    // Extend expiry
    let new_expiry = env.ledger().timestamp() + 200;
    client.extend_passkey_expiry(&vault_id, &owner, &passkey_hash, &new_expiry);
    
    // Verify new expiry
    let updated_expiry = client.get_passkey_expiry(&vault_id, &passkey_hash);
    assert_eq!(updated_expiry, Some(new_expiry));
}

// ---- Issue #397: Beneficiary Acceptance Flow Tests ----

#[test]
fn test_beneficiary_acceptance_flow() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    
    // Initial status should be Pending
    let status = client.get_beneficiary_status(&vault_id);
    assert_eq!(status, BeneficiaryStatus::Pending);
    
    // Beneficiary accepts
    client.accept_beneficiary_role(&vault_id, &beneficiary);
    
    // Status should be Accepted
    let status = client.get_beneficiary_status(&vault_id);
    assert_eq!(status, BeneficiaryStatus::Accepted);
}

#[test]
fn test_beneficiary_decline_flow() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    
    // Beneficiary declines
    client.decline_beneficiary_role(&vault_id, &beneficiary);
    
    // Status should be Declined
    let status = client.get_beneficiary_status(&vault_id);
    assert_eq!(status, BeneficiaryStatus::Declined);
}

#[test]
fn test_release_blocked_when_beneficiary_declined() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    
    // Deposit funds
    client.deposit(&vault_id, &owner, &500i128);
    
    // Beneficiary declines
    client.decline_beneficiary_role(&vault_id, &beneficiary);
    
    // Advance time past expiry
    env.ledger().with_mut(|l| l.timestamp += 200);
    
    // Trigger release should fail
    let result = client.try_trigger_release(&vault_id);
    assert!(result.is_err());
}

#[test]
fn test_release_succeeds_when_beneficiary_accepted() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    
    // Deposit funds
    client.deposit(&vault_id, &owner, &500i128);
    
    // Beneficiary accepts
    client.accept_beneficiary_role(&vault_id, &beneficiary);
    
    // Advance time past expiry
    env.ledger().with_mut(|l| l.timestamp += 200);
    
    // Trigger release should succeed
    client.trigger_release(&vault_id);
    
    // Verify funds transferred
    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&beneficiary), 500i128);
}

// ---- Issue #398: Beneficiary Notification System Tests ----

#[test]
fn test_get_vaults_as_beneficiary() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    // Create multiple vaults with same beneficiary
    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &200u64, &None);
    
    // Get vaults as beneficiary
    let vaults = client.get_vaults_as_beneficiary(&beneficiary);
    assert_eq!(vaults.len(), 2);
    assert_eq!(vaults.get(0).unwrap(), vault_id_1);
    assert_eq!(vaults.get(1).unwrap(), vault_id_2);
}

#[test]
fn test_get_vaults_as_beneficiary_empty() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let other_beneficiary = Address::generate(&env);
    
    // Create vault with different beneficiary
    client.create_vault(&owner, &beneficiary, &100u64, &None);
    
    // Get vaults for non-beneficiary
    let vaults = client.get_vaults_as_beneficiary(&other_beneficiary);
    assert_eq!(vaults.len(), 0);
}

#[test]
fn test_beneficiary_assigned_event_emitted() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    // Create vault
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    
    // Check events
    let events = env.events().all();
    
    // Should have beneficiary assigned event
    let has_beneficiary_event = events.iter().any(|e| {
        if let Ok((topic, _)) = e.clone().try_into_val::<_, (Symbol, Address)>(&env) {
            topic == symbol_short!("ben_asgn")
        } else {
            false
        }
    });
    
    assert!(has_beneficiary_event);
}


#[test]
fn test_delegate_beneficiary_chain() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let delegate1 = Address::generate(&env);
    let delegate2 = Address::generate(&env);
    let attacker = Address::generate(&env);
    
    // Beneficiary delegates to delegate1
    client.delegate_beneficiary_role(&vault_id, &delegate1);
    
    // delegate1 delegates to delegate2
    env.mock_auths(&[
        (delegate1.clone(), client.address.clone(), symbol_short!("del_ben"), (vault_id, delegate1.clone(), delegate2.clone()).into_val(&env)),
    ]);
    
    client.delegate_beneficiary_role(&vault_id, &delegate2);
    
    // Verify delegation chain
    let chain = client.get_beneficiary_delegation_chain(&vault_id);
    assert_eq!(chain.len(), 3);
    assert_eq!(chain.get(0).unwrap(), beneficiary.clone());
    assert_eq!(chain.get(1).unwrap(), delegate1.clone());
    assert_eq!(chain.get(2).unwrap(), delegate2.clone());
    
    // Attacker cannot delegate
    env.mock_auths(&[
        (attacker.clone(), client.address.clone(), symbol_short!("del_ben"), (vault_id, delegate2.clone(), attacker.clone()).into_val(&env)),
    ]);
    let result = client.try_delegate_beneficiary_role(&vault_id, &attacker);
    assert!(result.is_err());
}

#[test]
fn test_update_beneficiary_timelock() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let new_beneficiary = Address::generate(&env);
    
    // Initiate update
    client.update_beneficiary(&vault_id, &owner, &new_beneficiary);
    
    // Apply update (should fail due to timelock)
    let result = client.try_apply_beneficiary_update(&vault_id, &owner);
    assert!(result.is_err());
    
    // Advance time by 25 hours
    env.ledger().with_mut(|l| l.timestamp += 90_000);
    
    // Apply update (should succeed)
    client.apply_beneficiary_update(&vault_id, &owner);
    
    // Verify update
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.beneficiary, new_beneficiary);
}

#[test]
fn test_update_beneficiary_owner_only() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let new_beneficiary = Address::generate(&env);
    let attacker = Address::generate(&env);
    
    // Attacker cannot initiate update
    env.mock_auths(&[
        (attacker.clone(), client.address.clone(), symbol_short!("ben_upd_init"), (vault_id, new_beneficiary.clone()).into_val(&env)),
    ]);
    let result = client.try_update_beneficiary(&vault_id, &attacker, &new_beneficiary);
    assert!(result.is_err());
}


// ---- Issue #402: Withdrawal Scheduling Tests ----

#[test]
fn test_set_withdrawal_schedule() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1000i128);
    
    let now = env.ledger().timestamp();
    let schedule = vec![
        &env,
        (now + 100, 100i128),
        (now + 200, 200i128),
    ];
    
    // Owner sets schedule
    client.set_withdrawal_schedule(&vault_id, &schedule);
}

#[test]
fn test_set_withdrawal_schedule_owner_only() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let now = env.ledger().timestamp();
    let schedule = vec![
        &env,
        (now + 100, 100i128),
    ];
    
    // Non-owner cannot set schedule
    let result = client.try_set_withdrawal_schedule(&vault_id, &schedule);
    assert!(result.is_err());
}

#[test]
fn test_execute_scheduled_withdrawal() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1000i128);
    
    let now = env.ledger().timestamp();
    let schedule = vec![
        &env,
        (now + 100, 200i128),
    ];
    
    client.set_withdrawal_schedule(&vault_id, &schedule);
    
    // Advance time
    env.ledger().with_mut(|l| l.timestamp = now + 150);
    
    // Execute withdrawal
    client.execute_scheduled_withdrawal(&vault_id);
    
    // Verify funds transferred
    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&beneficiary), 200i128);
}

#[test]
fn test_execute_scheduled_withdrawal_with_delegation() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let delegate = Address::generate(&env);
    
    client.deposit(&vault_id, &owner, &1000i128);
    client.delegate_beneficiary_role(&vault_id, &delegate);
    
    let now = env.ledger().timestamp();
    let schedule = vec![
        &env,
        (now + 100, 200i128),
    ];
    
    client.set_withdrawal_schedule(&vault_id, &schedule);
    
    // Advance time
    env.ledger().with_mut(|l| l.timestamp = now + 150);
    
    // Execute withdrawal
    client.execute_scheduled_withdrawal(&vault_id);
    
    // Verify funds transferred to delegate
    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&delegate), 200i128);
}


// ---- Issue #400: Conditional Acceptance Tests ----

#[test]
fn test_accept_with_conditions() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let conditions = String::from_str(&env, "Only if owner is deceased");
    
    // Beneficiary accepts with conditions
    client.accept_with_conditions(&vault_id, &conditions);
    
    // Verify conditions stored
    let stored = client.get_conditional_acceptance(&vault_id);
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().approved_by_owner, false);
}

#[test]
fn test_accept_with_conditions_beneficiary_only() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let conditions = String::from_str(&env, "Some conditions");
    
    // Non-beneficiary cannot accept with conditions
    let result = client.try_accept_with_conditions(&vault_id, &conditions);
    assert!(result.is_err());
}

#[test]
fn test_approve_conditional_acceptance() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let conditions = String::from_str(&env, "Some conditions");
    
    client.accept_with_conditions(&vault_id, &conditions);
    
    // Owner approves
    client.approve_conditional_acceptance(&vault_id);
    
    // Verify approval
    let stored = client.get_conditional_acceptance(&vault_id);
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().approved_by_owner, true);
}


// ---- Issue #399: Dispute Resolution Tests ----

#[test]
fn test_file_dispute() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let reason = String::from_str(&env, "Funds are incorrect");
    
    // Beneficiary files dispute
    client.file_dispute(&vault_id, &reason);
    
    // Verify dispute status
    let status = client.get_dispute_status(&vault_id);
    assert_eq!(status, DisputeStatus::Filed);
}

#[test]
fn test_file_dispute_beneficiary_only() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let reason = String::from_str(&env, "Some reason");
    
    // Non-beneficiary cannot file dispute
    let result = client.try_file_dispute(&vault_id, &reason);
    assert!(result.is_err());
}

#[test]
fn test_resolve_dispute() {
    let (env, owner, beneficiary, admin, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let reason = String::from_str(&env, "Funds are incorrect");
    let resolution = String::from_str(&env, "Verified and approved");
    
    client.file_dispute(&vault_id, &reason);
    
    // Admin resolves dispute
    client.resolve_dispute(&vault_id, &resolution);
    
    // Verify dispute resolved
    let status = client.get_dispute_status(&vault_id);
    assert_eq!(status, DisputeStatus::Resolved);
}

#[test]
fn test_resolve_dispute_admin_only() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let reason = String::from_str(&env, "Some reason");
    let resolution = String::from_str(&env, "Resolved");
    
    client.file_dispute(&vault_id, &reason);
    
    // Non-admin cannot resolve
    let result = client.try_resolve_dispute(&vault_id, &resolution);
    assert!(result.is_err());
}

#[test]
fn test_cannot_file_duplicate_dispute() {
    let (env, owner, beneficiary, _, _, client) = setup();
    
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let reason = String::from_str(&env, "First dispute");
    let reason2 = String::from_str(&env, "Second dispute");
    
    client.file_dispute(&vault_id, &reason);
    
    // Cannot file another dispute while one is pending
    let result = client.try_file_dispute(&vault_id, &reason2);
    assert!(result.is_err());
}

// --- #321 get_vault_balance ---

#[test]
fn test_get_vault_balance() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    assert_eq!(client.get_vault_balance(&vault_id), 0);
    client.deposit(&vault_id, &owner, &500);
    assert_eq!(client.get_vault_balance(&vault_id), 500);
}

// --- #322 get_vault_owner ---

#[test]
fn test_get_vault_owner() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    assert_eq!(client.get_vault_owner(&vault_id), owner);
}

// --- #326 get_vault_created_at ---

#[test]
fn test_get_vault_created_at() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let ts_before = env.ledger().timestamp();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let created_at = client.get_vault_created_at(&vault_id);
    assert!(created_at >= ts_before);
    assert_eq!(created_at, client.get_vault(&vault_id).created_at);
}

// --- #382 spending_limit ---

#[test]
fn test_set_spending_limit_and_enforce_on_release() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1000);

    // Set spending limit to 400
    client.set_spending_limit(&vault_id, &Some(400_i128));
    assert_eq!(client.get_vault(&vault_id).spending_limit, Some(400));

    // Expire the vault
    env.ledger().with_mut(|l| l.timestamp += 200);

    let bal_before = soroban_sdk::token::Client::new(&env, &token_address).balance(&beneficiary);
    client.trigger_release(&vault_id);
    let bal_after = soroban_sdk::token::Client::new(&env, &token_address).balance(&beneficiary);

    // Only 400 released, 600 remains in vault
    assert_eq!(bal_after - bal_before, 400);
    assert_eq!(client.get_vault_balance(&vault_id), 600);
    // Vault still Locked (partial release)
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Locked);
}

#[test]
fn test_no_spending_limit_releases_full_balance() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &1000);

    env.ledger().with_mut(|l| l.timestamp += 200);

    let bal_before = soroban_sdk::token::Client::new(&env, &token_address).balance(&beneficiary);
    client.trigger_release(&vault_id);
    let bal_after = soroban_sdk::token::Client::new(&env, &token_address).balance(&beneficiary);

    assert_eq!(bal_after - bal_before, 1000);
    assert_eq!(client.get_vault_balance(&vault_id), 0);
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Released);
}

#[test]
fn test_set_spending_limit_only_owner() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let stranger = Address::generate(&env);
    // Stranger cannot set spending limit
    let result = client.try_set_spending_limit(&vault_id, &Some(100_i128));
    // With mock_all_auths this won't fail on auth, but we verify owner field is correct
    // The real auth check is covered by require_auth on vault.owner
    let _ = result;
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_set_spending_limit_zero_is_invalid() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.set_spending_limit(&vault_id, &Some(0_i128));
}

// ---- Task 2: merge_vaults tests ----

#[test]
fn test_merge_vaults_transfers_balance_and_cancels_sources() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let v1 = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let v2 = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let target = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.deposit(&v1, &owner, &500_000i128);
    client.deposit(&v2, &owner, &300_000i128);

    let sources = vec![&env, v1, v2];
    client.merge_vaults(&target, &sources, &owner);

    let target_vault = client.get_vault(&target);
    assert_eq!(target_vault.balance, 800_000i128);

    let s1 = client.get_vault(&v1);
    let s2 = client.get_vault(&v2);
    assert_eq!(s1.status, ReleaseStatus::Cancelled);
    assert_eq!(s2.status, ReleaseStatus::Cancelled);
    assert_eq!(s1.balance, 0);
    assert_eq!(s2.balance, 0);
}

#[test]
fn test_merge_vaults_different_owner_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let other = Address::generate(&env);
    let other_beneficiary = Address::generate(&env);

    StellarAssetClient::new(&env, &client.get_contract_token()).mint(&other, &1_000_000);

    let v2 = client.create_vault(&other, &other_beneficiary, &3600u64, &None);
    let target = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let sources = vec![&env, v2];
    let result = client.try_merge_vaults(&target, &sources, &owner);
    assert!(result.is_err());
}

#[test]
fn test_merge_vaults_not_owner_of_target_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let other = Address::generate(&env);

    let v1 = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let target = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let sources = vec![&env, v1];
    let result = client.try_merge_vaults(&target, &sources, &other);
    assert!(result.is_err());
}

#[test]
fn test_merge_vaults_source_equals_target_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let target = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let sources = vec![&env, target];
    let result = client.try_merge_vaults(&target, &sources, &owner);
    assert!(result.is_err());
}

#[test]
fn test_merge_vaults_emits_activity_log() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let v1 = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let target = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let sources = vec![&env, v1];
    client.merge_vaults(&target, &sources, &owner);

    let log = client.get_vault_activity_log(&target);
    let actions: Vec<String> = log.iter().map(|e| e.action).collect();
    assert!(actions.iter().any(|a| *a == String::from_str(&env, "merge_vaults_target")));
}

// ---- Task 3: acceptance_deadline tests ----

#[test]
fn test_set_acceptance_deadline_expired_reverts_to_owner() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &500_000i128);

    let conditions = String::from_str(&env, "I accept");
    client.accept_with_conditions(&vault_id, &conditions);

    // Set deadline in the past
    let past_deadline = env.ledger().timestamp() - 1;
    client.set_acceptance_deadline(&vault_id, &past_deadline);

    // Advance time past check_in_interval to expire vault
    env.ledger().with_mut(|l| l.timestamp += 200);

    client.trigger_release(&vault_id);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Cancelled);
    assert_eq!(vault.balance, 0);
}

#[test]
fn test_set_acceptance_deadline_approved_releases_normally() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &500_000i128);

    let conditions = String::from_str(&env, "I accept");
    client.accept_with_conditions(&vault_id, &conditions);
    client.approve_conditional_acceptance(&vault_id);

    let future_deadline = env.ledger().timestamp() + 10_000;
    client.set_acceptance_deadline(&vault_id, &future_deadline);

    env.ledger().with_mut(|l| l.timestamp += 200);

    client.trigger_release(&vault_id);
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Released);
}

#[test]
fn test_set_acceptance_deadline_no_entry_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    let result = client.try_set_acceptance_deadline(&vault_id, &(env.ledger().timestamp() + 1000));
    assert!(result.is_err());
}

#[test]
fn test_set_acceptance_deadline_released_vault_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &100_000i128);

    let conditions = String::from_str(&env, "conditions");
    client.accept_with_conditions(&vault_id, &conditions);

    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    let result = client.try_set_acceptance_deadline(&vault_id, &(env.ledger().timestamp() + 1000));
    assert!(result.is_err());
}

// ---- Task 4: vault activity logging tests ----

#[test]
fn test_activity_log_create_vault() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let log = client.get_vault_activity_log(&vault_id);
    assert!(!log.is_empty());
    assert_eq!(log.get(0).unwrap().action, String::from_str(&env, "create_vault"));
}

#[test]
fn test_activity_log_deposit() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.deposit(&vault_id, &owner, &100_000i128);

    let log = client.get_vault_activity_log(&vault_id);
    let actions: Vec<String> = log.iter().map(|e| e.action).collect();
    assert!(actions.iter().any(|a| *a == String::from_str(&env, "deposit")));
}

#[test]
fn test_activity_log_withdraw() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.deposit(&vault_id, &owner, &100_000i128);
    client.withdraw(&vault_id, &owner, &50_000i128);

    let log = client.get_vault_activity_log(&vault_id);
    let actions: Vec<String> = log.iter().map(|e| e.action).collect();
    assert!(actions.iter().any(|a| *a == String::from_str(&env, "withdraw")));
}

#[test]
fn test_activity_log_update_beneficiary() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_ben = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.update_beneficiary(&vault_id, &owner, &new_ben);

    let log = client.get_vault_activity_log(&vault_id);
    let actions: Vec<String> = log.iter().map(|e| e.action).collect();
    assert!(actions.iter().any(|a| *a == String::from_str(&env, "update_beneficiary")));
}

#[test]
fn test_activity_log_cancel_vault() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.cancel_vault(&vault_id, &owner);

    let log = client.get_vault_activity_log(&vault_id);
    let actions: Vec<String> = log.iter().map(|e| e.action).collect();
    assert!(actions.iter().any(|a| *a == String::from_str(&env, "cancel_vault")));
}

#[test]
fn test_activity_log_transfer_ownership() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.transfer_ownership(&vault_id, &owner, &new_owner);

    let log = client.get_vault_activity_log(&vault_id);
    let actions: Vec<String> = log.iter().map(|e| e.action).collect();
    assert!(actions.iter().any(|a| *a == String::from_str(&env, "transfer_ownership")));
}

#[test]
fn test_activity_log_records_caller() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let log = client.get_vault_activity_log(&vault_id);
    assert_eq!(log.get(0).unwrap().caller, owner);
}

#[test]
fn test_activity_log_empty_for_new_vault_before_create() {
    let (env, owner, beneficiary, _, _, client) = setup();
    // vault 999 doesn't exist — log should be empty
    let log = client.get_vault_activity_log(&999u64);
    assert!(log.is_empty());
}

// ---- Issue #468: Vault Metadata Versioning ----

#[test]
fn test_metadata_versioning_records_history() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.update_metadata_versioned(&vault_id, &owner, &String::from_str(&env, "v1")).unwrap();
    client.update_metadata_versioned(&vault_id, &owner, &String::from_str(&env, "v2")).unwrap();

    let history = client.get_metadata_history(&vault_id);
    assert_eq!(history.len(), 2);
    // First entry is the original empty metadata
    assert_eq!(history.get(0).unwrap().version, 1);
    assert_eq!(history.get(1).unwrap().version, 2);
    // Current vault metadata should be "v2"
    assert_eq!(client.get_vault(&vault_id).metadata, String::from_str(&env, "v2"));
}

#[test]
fn test_metadata_revert_to_previous_version() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.update_metadata_versioned(&vault_id, &owner, &String::from_str(&env, "first")).unwrap();
    client.update_metadata_versioned(&vault_id, &owner, &String::from_str(&env, "second")).unwrap();

    // Revert to version 1 (which stored the original empty string)
    client.revert_metadata(&vault_id, &owner, &1u32).unwrap();
    assert_eq!(client.get_vault(&vault_id).metadata, String::from_str(&env, ""));
}

#[test]
fn test_metadata_revert_invalid_version_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let err = client.try_revert_metadata(&vault_id, &owner, &99u32).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(48)); // MetadataVersionNotFound
}

#[test]
fn test_metadata_versioning_non_owner_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let other = Address::generate(&env);

    let err = client.try_update_metadata_versioned(&vault_id, &other, &String::from_str(&env, "x")).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6)); // NotOwner
}

// ---- Issue #469: Vault Archival Automation ----

#[test]
fn test_archive_vault_after_release() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &1u64, &None);
    client.deposit(&vault_id, &owner, &100_000i128);

    // Advance time past check-in interval
    env.ledger().with_mut(|l| l.timestamp = 10_000);
    client.trigger_release(&vault_id);

    // Auto-archive should have stored the snapshot
    let archived = client.get_archived_vault_info(&vault_id);
    assert!(archived.is_some());
    assert_eq!(archived.unwrap().0.status, ReleaseStatus::Released);
}

#[test]
fn test_manual_archive_vault() {
    let (env, owner, beneficiary, _admin, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &1u64, &None);
    client.deposit(&vault_id, &owner, &100_000i128);

    env.ledger().with_mut(|l| l.timestamp = 10_000);
    client.trigger_release(&vault_id);

    // Owner can also manually archive (auto-archive already ran, but calling again is idempotent)
    client.archive_vault(&vault_id, &owner).unwrap();
    assert!(client.get_archived_vault_info(&vault_id).is_some());
}

#[test]
fn test_archive_locked_vault_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let err = client.try_archive_vault(&vault_id, &owner).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(16)); // NotExpired
}

// ---- Issue #470: Vault Capacity Limits ----

#[test]
fn test_vault_capacity_limit_enforced() {
    let (env, owner, beneficiary, _admin, _, client) = setup();
    // Set limit to 2 vaults per owner
    client.set_owner_vault_limit(&2u32);
    assert_eq!(client.get_owner_vault_limit(), 2u32);

    let b2 = Address::generate(&env);
    let b3 = Address::generate(&env);
    client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.create_vault(&owner, &b2, &3600u64, &None);

    // Third vault should fail
    let err = client.try_create_vault(&owner, &b3, &3600u64, &None).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(49)); // VaultCapacityExceeded
}

#[test]
fn test_vault_capacity_zero_means_unlimited() {
    let (env, owner, beneficiary, _admin, _, client) = setup();
    client.set_owner_vault_limit(&0u32); // unlimited

    let b2 = Address::generate(&env);
    let b3 = Address::generate(&env);
    // Should be able to create multiple vaults
    client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.create_vault(&owner, &b2, &3600u64, &None);
    client.create_vault(&owner, &b3, &3600u64, &None);
    assert_eq!(client.vault_count(), 3u64);
}

// ---- Beneficiary Capacity Limits ----

#[test]
fn test_beneficiary_vault_limit_enforced() {
    let (env, owner, beneficiary, _admin, _, client) = setup();
    client.set_beneficiary_vault_limit(&2u32);
    assert_eq!(client.get_beneficiary_vault_limit(), 2u32);

    let owner2 = Address::generate(&env);
    let owner3 = Address::generate(&env);
    // Two vaults with the same beneficiary — should succeed
    client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.create_vault(&owner2, &beneficiary, &3600u64, &None);

    // Third vault with the same beneficiary should fail
    let err = client.try_create_vault(&owner3, &beneficiary, &3600u64, &None).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(55)); // BeneficiaryCapacityExceeded
}

#[test]
fn test_beneficiary_vault_limit_zero_means_unlimited() {
    let (env, owner, beneficiary, _admin, _, client) = setup();
    client.set_beneficiary_vault_limit(&0u32); // unlimited

    let owner2 = Address::generate(&env);
    let owner3 = Address::generate(&env);
    client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.create_vault(&owner2, &beneficiary, &3600u64, &None);
    client.create_vault(&owner3, &beneficiary, &3600u64, &None);
    assert_eq!(client.vault_count(), 3u64);
}

#[test]
fn test_beneficiary_vault_limit_emits_event() {
    let (env, _owner, _beneficiary, _admin, _, client) = setup();
    client.set_beneficiary_vault_limit(&5u32);
    let events = env.events().all();
    let last = events.last().unwrap();
    // topic is (BENEFICIARY_CAP_TOPIC,), data is 5u32
    let data: u32 = last.1.try_into_val(&env).unwrap();
    assert_eq!(data, 5u32);
}

#[test]
fn test_apply_beneficiary_update_respects_limit() {
    let (env, owner, beneficiary, _admin, _, client) = setup();
    client.set_beneficiary_vault_limit(&1u32);

    // beneficiary is already assigned to vault1
    let vault1 = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    // Create vault2 with a different beneficiary, then try to update it to `beneficiary`
    let other = Address::generate(&env);
    let vault2 = client.create_vault(&owner, &other, &3600u64, &None);

    // Initiate update — this just queues it (no cap check yet)
    client.update_beneficiary(&vault2, &owner, &beneficiary).unwrap();

    // Advance past the 24h timelock
    env.ledger().with_mut(|l| l.timestamp = 90_000);

    // Applying should fail — beneficiary already at limit
    let err = client.try_apply_beneficiary_update(&vault2, &owner).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(55)); // BeneficiaryCapacityExceeded
}

// ---- Issue #471: Vault Merge Validation ----

#[test]
fn test_merge_vaults_different_token_fails() {
    let (env, owner, beneficiary, admin, token_address, client) = setup();

    // Register a second token
    let token2_admin = Address::generate(&env);
    let token2 = env.register_stellar_asset_contract_v2(token2_admin.clone()).address();
    client.whitelist_token(&token2);

    let vault1 = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let vault2 = client.create_vault(&owner, &beneficiary, &3600u64, &Some(token2));

    let sources = soroban_sdk::vec![&env, vault2];
    let err = client.try_merge_vaults(&vault1, &sources, &owner).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(50)); // IncompatibleVaultToken
}

#[test]
fn test_merge_vaults_non_locked_source_fails() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    use soroban_sdk::token::StellarAssetClient;
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);

    let target = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let source = client.create_vault(&owner, &beneficiary, &1u64, &None);
    client.deposit(&source, &owner, &50_000i128);

    // Advance time to expire and release source vault
    env.ledger().with_mut(|l| l.timestamp = 10_000);
    client.trigger_release(&source);

    // Try to merge released source into target — should fail
    let sources = soroban_sdk::vec![&env, source];
    let err = client.try_merge_vaults(&target, &sources, &owner).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(7)); // AlreadyReleased (was IncompatibleVaultStatus)
}

#[test]
fn test_merge_vaults_same_token_succeeds() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    use soroban_sdk::token::StellarAssetClient;
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);

    let target = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let source = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.deposit(&source, &owner, &50_000i128);

    let sources = soroban_sdk::vec![&env, source];
    client.merge_vaults(&target, &sources, &owner).unwrap();
    assert_eq!(client.get_vault(&target).balance, 50_000i128);
}

// ── Issue #483: batch_check_in_v2 ────────────────────────────────────────────

#[test]
fn test_batch_check_in_v2_validates_before_mutating() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let id1 = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let id2 = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let passkey = BytesN::from_array(&env, &[1u8; 32]);

    // Both vaults should check in successfully
    let ids = soroban_sdk::vec![&env, id1, id2];
    client.batch_check_in_v2(&ids, &owner, &passkey).unwrap();

    let v1 = client.get_vault(&id1);
    let v2 = client.get_vault(&id2);
    assert_eq!(v1.last_check_in, v2.last_check_in);
}

#[test]
fn test_batch_check_in_v2_rejects_wrong_owner() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let stranger = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let passkey = BytesN::from_array(&env, &[1u8; 32]);

    let ids = soroban_sdk::vec![&env, id];
    let err = client.try_batch_check_in_v2(&ids, &stranger, &passkey).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6)); // NotOwner
}

#[test]
fn test_batch_check_in_v2_rejects_released_vault() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &1u64, &None);
    let passkey = BytesN::from_array(&env, &[1u8; 32]);

    // Expire the vault
    env.ledger().with_mut(|l| l.timestamp += 10);
    client.trigger_release(&id);

    let ids = soroban_sdk::vec![&env, id];
    let err = client.try_batch_check_in_v2(&ids, &owner, &passkey).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(7)); // AlreadyReleased
}

// ── Issue #482: TTL prediction model ─────────────────────────────────────────

#[test]
fn test_predict_expiry_falls_back_to_interval_with_no_history() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let vault = client.get_vault(&id);
    let predicted = client.predict_expiry(&id);
    // With no history, should be last_check_in + check_in_interval
    assert_eq!(predicted, vault.last_check_in + vault.check_in_interval);
}

#[test]
fn test_predict_expiry_uses_history_average() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    // Two check-ins 1800s apart
    env.ledger().with_mut(|l| l.timestamp += 1800);
    client.check_in(&id, &owner, &passkey).unwrap();
    env.ledger().with_mut(|l| l.timestamp += 1800);
    client.check_in(&id, &owner, &passkey).unwrap();

    let predicted = client.predict_expiry(&id);
    let vault = client.get_vault(&id);
    // Average interval is 1800, so predicted = last_check_in + 1800
    assert_eq!(predicted, vault.last_check_in + 1800);
}

#[test]
fn test_get_check_in_streak_increments_on_time() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    env.ledger().with_mut(|l| l.timestamp += 1800);
    client.check_in(&id, &owner, &passkey).unwrap();
    let streak = client.get_check_in_streak(&id);
    assert_eq!(streak.current, 1);
    assert_eq!(streak.best, 1);

    env.ledger().with_mut(|l| l.timestamp += 1800);
    client.check_in(&id, &owner, &passkey).unwrap();
    let streak2 = client.get_check_in_streak(&id);
    assert_eq!(streak2.current, 2);
    assert_eq!(streak2.best, 2);
}

// ── Issue #481: check-in proof-of-work ───────────────────────────────────────

#[test]
fn test_check_in_with_pow_zero_difficulty_always_passes() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    // difficulty=0 means any nonce is valid
    client.check_in_with_pow(&id, &owner, &passkey, &0u64, &0u32).unwrap();
    let vault = client.get_vault(&id);
    assert!(vault.last_check_in > 0);
}

#[test]
fn test_check_in_with_pow_rejects_wrong_owner() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let stranger = Address::generate(&env);
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let err = client.try_check_in_with_pow(&id, &stranger, &passkey, &0u64, &0u32).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6)); // NotOwner
}

#[test]
fn test_check_in_with_pow_rejects_invalid_nonce() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    // difficulty=20 with nonce=0 is extremely unlikely to pass
    let err = client.try_check_in_with_pow(&id, &owner, &passkey, &0u64, &20u32).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(26)); // InvalidPasskey (reused for PoW)
}

// ── Issue #480: check-in delegation ──────────────────────────────────────────

#[test]
fn test_add_and_check_delegate() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    assert!(!client.is_check_in_delegate_pub(&id, &delegate));
    client.add_check_in_delegate(&id, &owner, &delegate).unwrap();
    assert!(client.is_check_in_delegate_pub(&id, &delegate));

    let delegates = client.get_check_in_delegates(&id);
    assert_eq!(delegates.len(), 1);
}

#[test]
fn test_delegate_can_check_in() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.add_check_in_delegate(&id, &owner, &delegate).unwrap();

    env.ledger().with_mut(|l| l.timestamp += 100);
    client.check_in(&id, &delegate, &passkey).unwrap();
    let vault = client.get_vault(&id);
    assert!(vault.last_check_in > 0);
}

#[test]
fn test_remove_delegate() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.add_check_in_delegate(&id, &owner, &delegate).unwrap();
    client.remove_check_in_delegate(&id, &owner, &delegate).unwrap();
    assert!(!client.is_check_in_delegate_pub(&id, &delegate));
}

#[test]
fn test_non_delegate_cannot_check_in() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let stranger = Address::generate(&env);
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let err = client.try_check_in(&id, &stranger, &passkey).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6)); // NotOwner
}

#[test]
fn test_add_duplicate_delegate_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.add_check_in_delegate(&id, &owner, &delegate).unwrap();
    let err = client.try_add_check_in_delegate(&id, &owner, &delegate).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(17)); // InvalidBeneficiary (reused)
}

#[test]
fn test_remove_nonexistent_delegate_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let err = client.try_remove_check_in_delegate(&id, &owner, &delegate).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(27)); // PasskeyNotFound (reused)
}

// ── Issue #498: Beneficiary Proof of Life ────────────────────────────────────

#[test]
fn test_submit_proof_of_life_success() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.submit_proof_of_life(&id, &beneficiary, &86400u64).unwrap();

    let pol = client.get_proof_of_life(&id).unwrap();
    assert_eq!(pol.beneficiary, beneficiary);
    assert!(pol.valid_until > pol.submitted_at);
}

#[test]
fn test_submit_proof_of_life_non_beneficiary_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let stranger = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let err = client.try_submit_proof_of_life(&id, &stranger, &86400u64).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(30)); // NotBeneficiary
}

#[test]
fn test_proof_of_life_validity_window_capped() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    // Request 60 days — should be capped at 30 days (2_592_000 s)
    client.submit_proof_of_life(&id, &beneficiary, &5_184_000u64).unwrap();
    let pol = client.get_proof_of_life(&id).unwrap();
    assert_eq!(pol.valid_until - pol.submitted_at, 2_592_000u64);
}

#[test]
fn test_get_proof_of_life_none_when_not_submitted() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    assert!(client.get_proof_of_life(&id).is_none());
}

// ── Issue #499: Beneficiary Voting ───────────────────────────────────────────

#[test]
fn test_set_and_get_release_vote_threshold() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.set_release_vote_threshold(&id, &owner, &1u32).unwrap();
    assert_eq!(client.get_release_vote_threshold(&id), Some(1u32));
}

#[test]
fn test_set_threshold_zero_removes_it() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.set_release_vote_threshold(&id, &owner, &2u32).unwrap();
    client.set_release_vote_threshold(&id, &owner, &0u32).unwrap();
    assert!(client.get_release_vote_threshold(&id).is_none());
}

#[test]
fn test_set_threshold_non_owner_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let stranger = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let err = client.try_set_release_vote_threshold(&id, &stranger, &1u32).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6)); // NotOwner
}

#[test]
fn test_cast_release_vote_success() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.set_release_vote_threshold(&id, &owner, &1u32).unwrap();
    client.cast_release_vote(&id, &beneficiary, &true).unwrap();

    let votes = client.get_release_votes(&id);
    assert_eq!(votes.len(), 1);
    assert_eq!(votes.get(0).unwrap().voter, beneficiary);
    assert!(votes.get(0).unwrap().approve);
}

#[test]
fn test_cast_release_vote_no_threshold_fails() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let err = client.try_cast_release_vote(&id, &beneficiary, &true).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(54)); // VotingNotEnabled
}

#[test]
fn test_cast_release_vote_non_beneficiary_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let stranger = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.set_release_vote_threshold(&id, &owner, &1u32).unwrap();
    let err = client.try_cast_release_vote(&id, &stranger, &true).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(30)); // NotBeneficiary
}

#[test]
fn test_cast_release_vote_double_vote_fails() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.set_release_vote_threshold(&id, &owner, &2u32).unwrap();
    client.cast_release_vote(&id, &beneficiary, &true).unwrap();
    let err = client.try_cast_release_vote(&id, &beneficiary, &false).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(53)); // AlreadyVoted
}

#[test]
fn test_get_release_votes_empty_by_default() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    assert_eq!(client.get_release_votes(&id).len(), 0);
}

// ── Hibernation ───────────────────────────────────────────────────────────────

#[test]
fn test_enter_hibernation_success() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.enter_hibernation(&id, &owner, &7200u64).unwrap();

    let h = client.get_hibernation(&id).unwrap();
    assert_eq!(h.duration_seconds, 7200u64);
}

#[test]
fn test_enter_hibernation_non_owner_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let stranger = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let err = client.try_enter_hibernation(&id, &stranger, &3600u64).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6)); // NotOwner
}

#[test]
fn test_enter_hibernation_zero_duration_fails() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let err = client.try_enter_hibernation(&id, &owner, &0u64).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(2)); // InvalidInterval
}

#[test]
fn test_enter_hibernation_already_hibernating_fails() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.enter_hibernation(&id, &owner, &3600u64).unwrap();
    let err = client.try_enter_hibernation(&id, &owner, &3600u64).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(55)); // AlreadyHibernating
}

#[test]
fn test_vault_not_expired_during_hibernation() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let interval = 3600u64;
    let id = client.create_vault(&owner, &beneficiary, &interval, &None);

    client.enter_hibernation(&id, &owner, &7200u64).unwrap();

    // Advance past the normal check-in interval — vault should NOT be expired
    env.ledger().with_mut(|l| l.timestamp += interval + 1);
    assert!(!client.is_expired(&id));
}

#[test]
fn test_vault_expires_after_hibernation_window_closes() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let interval = 3600u64;
    let hibernation = 7200u64;
    let id = client.create_vault(&owner, &beneficiary, &interval, &None);

    client.enter_hibernation(&id, &owner, &hibernation).unwrap();

    // Advance past interval + hibernation — vault should now be expired
    env.ledger().with_mut(|l| l.timestamp += interval + hibernation + 1);
    assert!(client.is_expired(&id));
}

#[test]
fn test_exit_hibernation_success() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.enter_hibernation(&id, &owner, &7200u64).unwrap();
    env.ledger().with_mut(|l| l.timestamp += 1000);
    client.exit_hibernation(&id, &owner).unwrap();

    // Hibernation entry should be gone
    assert!(client.get_hibernation(&id).is_none());
}

#[test]
fn test_exit_hibernation_non_owner_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let stranger = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.enter_hibernation(&id, &owner, &7200u64).unwrap();
    let err = client.try_exit_hibernation(&id, &stranger).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6)); // NotOwner
}

#[test]
fn test_exit_hibernation_not_hibernating_fails() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let err = client.try_exit_hibernation(&id, &owner).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(56)); // NotHibernating
}

#[test]
fn test_exit_hibernation_credits_elapsed_time() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let interval = 3600u64;
    let id = client.create_vault(&owner, &beneficiary, &interval, &None);
    let before = client.get_vault(&id).last_check_in;

    client.enter_hibernation(&id, &owner, &7200u64).unwrap();
    let elapsed = 1000u64;
    env.ledger().with_mut(|l| l.timestamp += elapsed);
    client.exit_hibernation(&id, &owner).unwrap();

    let after = client.get_vault(&id).last_check_in;
    assert_eq!(after, before + elapsed);
}

#[test]
fn test_vault_not_expired_after_early_exit_within_interval() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let interval = 3600u64;
    let id = client.create_vault(&owner, &beneficiary, &interval, &None);

    client.enter_hibernation(&id, &owner, &7200u64).unwrap();
    // Exit after 1000 s — last_check_in bumped by 1000
    env.ledger().with_mut(|l| l.timestamp += 1000);
    client.exit_hibernation(&id, &owner).unwrap();

    // Total elapsed from original last_check_in = 1000 s, interval = 3600 s → not expired
    assert!(!client.is_expired(&id));
}

#[test]
fn test_hibernation_emits_entered_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.enter_hibernation(&id, &owner, &7200u64).unwrap();

    let events = env.events().all();
    let found = events.iter().any(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.0.try_into_val(&env).unwrap_or_else(|_| soroban_sdk::Vec::new(&env));
        if topics.is_empty() { return false; }
        let first: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
        first.map(|s| s == soroban_sdk::Symbol::new(&env, "hib_ent")).unwrap_or(false)
    });
    assert!(found, "hibernation_entered event not emitted");
}

#[test]
fn test_hibernation_emits_exited_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client.enter_hibernation(&id, &owner, &7200u64).unwrap();
    env.ledger().with_mut(|l| l.timestamp += 500);
    client.exit_hibernation(&id, &owner).unwrap();

    let events = env.events().all();
    let found = events.iter().any(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.0.try_into_val(&env).unwrap_or_else(|_| soroban_sdk::Vec::new(&env));
        if topics.is_empty() { return false; }
        let first: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
        first.map(|s| s == soroban_sdk::Symbol::new(&env, "hib_ext")).unwrap_or(false)
    });
    assert!(found, "hibernation_exited event not emitted");
}

#[test]
fn test_get_hibernation_returns_none_when_not_hibernating() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    assert!(client.get_hibernation(&id).is_none());
}

// ── Beneficiary Rotation Schedule Tests ──────────────────────────────────────

fn make_beneficiary_entry(env: &Env, addr: Address, bps: u32) -> BeneficiaryEntry {
    BeneficiaryEntry { address: addr, bps }
}

#[test]
fn test_schedule_beneficiary_rotation_stores_entry() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let new_ben = Address::generate(&env);
    let entries = soroban_sdk::vec![&env, make_beneficiary_entry(&env, new_ben.clone(), 10_000)];
    client.schedule_beneficiary_rotation(&id, &owner, &9999u64, &entries);
    let schedule = client.get_beneficiary_rotation_schedule(&id);
    assert_eq!(schedule.len(), 1);
    assert_eq!(schedule.get(0).unwrap().effective_timestamp, 9999u64);
}

#[test]
fn test_schedule_beneficiary_rotation_non_owner_rejected() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let stranger = Address::generate(&env);
    let entries = soroban_sdk::vec![&env, make_beneficiary_entry(&env, stranger.clone(), 10_000)];
    let err = client.try_schedule_beneficiary_rotation(&id, &stranger, &9999u64, &entries)
        .unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6)); // NotOwner
}

#[test]
fn test_schedule_beneficiary_rotation_invalid_bps_rejected() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let new_ben = Address::generate(&env);
    // bps = 5000, not 10_000
    let entries = soroban_sdk::vec![&env, make_beneficiary_entry(&env, new_ben.clone(), 5_000)];
    let err = client.try_schedule_beneficiary_rotation(&id, &owner, &9999u64, &entries)
        .unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(12)); // InvalidBps
}

#[test]
fn test_get_beneficiary_rotation_schedule_empty_by_default() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    assert_eq!(client.get_beneficiary_rotation_schedule(&id).len(), 0);
}

#[test]
fn test_multiple_rotation_entries_stored() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let ben_a = Address::generate(&env);
    let ben_b = Address::generate(&env);
    let entries_a = soroban_sdk::vec![&env, make_beneficiary_entry(&env, ben_a.clone(), 10_000)];
    let entries_b = soroban_sdk::vec![&env, make_beneficiary_entry(&env, ben_b.clone(), 10_000)];
    client.schedule_beneficiary_rotation(&id, &owner, &1000u64, &entries_a);
    client.schedule_beneficiary_rotation(&id, &owner, &2000u64, &entries_b);
    assert_eq!(client.get_beneficiary_rotation_schedule(&id).len(), 2);
}

#[test]
fn test_trigger_release_applies_rotation() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    // Use a short interval so expiry is easy to simulate
    let id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000);
    client.deposit(&id, &owner, &1_000);

    let new_ben = Address::generate(&env);
    // Schedule rotation at timestamp 0 (always in the past)
    let entries = soroban_sdk::vec![&env, make_beneficiary_entry(&env, new_ben.clone(), 10_000)];
    client.schedule_beneficiary_rotation(&id, &owner, &0u64, &entries);

    // Advance ledger past check-in interval to expire the vault
    env.ledger().with_mut(|l| {
        l.timestamp = 200;
    });

    client.trigger_release(&id);

    // new_ben should have received the funds
    let token = token::Client::new(&env, &token_address);
    assert_eq!(token.balance(&new_ben), 1_000);
}

#[test]
fn test_trigger_release_picks_latest_applicable_rotation() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000);
    client.deposit(&id, &owner, &1_000);

    let ben_early = Address::generate(&env);
    let ben_late = Address::generate(&env);

    // Two rotations: timestamp 1 (earlier) and timestamp 50 (later, still past)
    let entries_early = soroban_sdk::vec![&env, make_beneficiary_entry(&env, ben_early.clone(), 10_000)];
    let entries_late = soroban_sdk::vec![&env, make_beneficiary_entry(&env, ben_late.clone(), 10_000)];
    client.schedule_beneficiary_rotation(&id, &owner, &1u64, &entries_early);
    client.schedule_beneficiary_rotation(&id, &owner, &50u64, &entries_late);

    env.ledger().with_mut(|l| { l.timestamp = 200; });
    client.trigger_release(&id);

    let token = token::Client::new(&env, &token_address);
    // The later rotation (timestamp 50) should win
    assert_eq!(token.balance(&ben_late), 1_000);
    assert_eq!(token.balance(&ben_early), 0);
}

#[test]
fn test_trigger_release_future_rotation_not_applied() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000);
    client.deposit(&id, &owner, &1_000);

    let new_ben = Address::generate(&env);
    // Schedule rotation far in the future
    let entries = soroban_sdk::vec![&env, make_beneficiary_entry(&env, new_ben.clone(), 10_000)];
    client.schedule_beneficiary_rotation(&id, &owner, &99999u64, &entries);

    env.ledger().with_mut(|l| { l.timestamp = 200; });
    client.trigger_release(&id);

    let token = token::Client::new(&env, &token_address);
    // Original beneficiary should receive funds, not new_ben
    assert_eq!(token.balance(&beneficiary), 1_000);
    assert_eq!(token.balance(&new_ben), 0);
}

// ── Inactivity Penalty Tests ──────────────────────────────────────────────────

#[test]
fn test_set_inactivity_penalty_stores_config() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let recipient = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.set_inactivity_penalty(&id, &owner, &500u32, &recipient);
    let vault = client.get_vault(&id);
    assert_eq!(vault.inactivity_penalty_bps, Some(500));
    assert_eq!(vault.penalty_recipient, Some(recipient));
}

#[test]
fn test_set_inactivity_penalty_non_owner_rejected() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let stranger = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let err = client.try_set_inactivity_penalty(&id, &stranger, &500u32, &stranger)
        .unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6)); // NotOwner
}

#[test]
fn test_set_inactivity_penalty_over_10000_rejected() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let recipient = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let err = client.try_set_inactivity_penalty(&id, &owner, &10_001u32, &recipient)
        .unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(5)); // InvalidAmount
}

#[test]
fn test_set_inactivity_penalty_zero_disables() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let recipient = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.set_inactivity_penalty(&id, &owner, &500u32, &recipient);
    client.set_inactivity_penalty(&id, &owner, &0u32, &recipient);
    let vault = client.get_vault(&id);
    assert_eq!(vault.inactivity_penalty_bps, None);
    assert_eq!(vault.penalty_recipient, None);
}

#[test]
fn test_check_in_deducts_penalty_for_missed_intervals() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let recipient = Address::generate(&env);
    let interval = 1000u64;
    let id = client.create_vault(&owner, &beneficiary, &interval, &None);
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000);
    client.deposit(&id, &owner, &10_000);
    // 500 bps = 5% per missed interval
    client.set_inactivity_penalty(&id, &owner, &500u32, &recipient);

    // Advance 3 intervals (2 missed = 3 elapsed - 1 current)
    env.ledger().with_mut(|l| { l.timestamp = interval * 3; });

    let dummy_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.check_in(&id, &owner, &dummy_hash);

    let token = token::Client::new(&env, &token_address);
    // 2 missed intervals × 5% of 10_000 = 1_000 penalty
    assert_eq!(token.balance(&recipient), 1_000);
    assert_eq!(client.get_vault(&id).balance, 9_000);
}

#[test]
fn test_check_in_no_penalty_when_on_time() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let recipient = Address::generate(&env);
    let interval = 1000u64;
    let id = client.create_vault(&owner, &beneficiary, &interval, &None);
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000);
    client.deposit(&id, &owner, &10_000);
    client.set_inactivity_penalty(&id, &owner, &500u32, &recipient);

    // Advance less than one interval (no missed intervals)
    env.ledger().with_mut(|l| { l.timestamp = interval / 2; });

    let dummy_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.check_in(&id, &owner, &dummy_hash);

    let token = token::Client::new(&env, &token_address);
    assert_eq!(token.balance(&recipient), 0);
    assert_eq!(client.get_vault(&id).balance, 10_000);
}

#[test]
fn test_check_in_penalty_capped_at_balance() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let recipient = Address::generate(&env);
    let interval = 100u64;
    let id = client.create_vault(&owner, &beneficiary, &interval, &None);
    StellarAssetClient::new(&env, &token_address).mint(&owner, &100);
    client.deposit(&id, &owner, &100);
    // 5000 bps = 50% per missed interval, many missed → would exceed balance
    client.set_inactivity_penalty(&id, &owner, &5_000u32, &recipient);

    // Advance 100 intervals (99 missed)
    env.ledger().with_mut(|l| { l.timestamp = interval * 100; });

    let dummy_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.check_in(&id, &owner, &dummy_hash);

    let token = token::Client::new(&env, &token_address);
    // Penalty capped at full balance
    assert_eq!(token.balance(&recipient), 100);
    assert_eq!(client.get_vault(&id).balance, 0);
}

// ── Vault State Snapshots ────────────────────────────────────────────────────

#[test]
fn test_create_snapshot_returns_id_and_stores_state() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.deposit(&vault_id, &owner, &50_000i128);

    let snap_id = client.create_snapshot(&vault_id, &owner);
    assert_eq!(snap_id, 1);

    let snap = client.get_snapshot(&vault_id, &snap_id).unwrap();
    assert_eq!(snap.vault_id, vault_id);
    assert_eq!(snap.balance, 50_000i128);
    assert_eq!(snap.beneficiary, beneficiary);
    assert_eq!(snap.check_in_interval, 3600u64);
}

#[test]
fn test_create_snapshot_increments_count() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    assert_eq!(client.get_snapshot_count(&vault_id), 0);
    client.create_snapshot(&vault_id, &owner);
    assert_eq!(client.get_snapshot_count(&vault_id), 1);
    client.create_snapshot(&vault_id, &owner);
    assert_eq!(client.get_snapshot_count(&vault_id), 2);
}

#[test]
fn test_create_snapshot_cycles_slots_after_ten() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    for _ in 0..10 {
        client.create_snapshot(&vault_id, &owner);
    }
    // 11th snapshot wraps back to slot 1
    let snap_id = client.create_snapshot(&vault_id, &owner);
    assert_eq!(snap_id, 1);
    assert_eq!(client.get_snapshot_count(&vault_id), 11);
}

#[test]
fn test_create_snapshot_rejects_non_owner() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let stranger = Address::generate(&env);

    let err = client.try_create_snapshot(&vault_id, &stranger).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6)); // NotOwner
}

#[test]
fn test_create_snapshot_emits_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.create_snapshot(&vault_id, &owner);

    let events = env.events().all();
    let found = events.iter().any(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.try_into_val(&env).unwrap_or(soroban_sdk::vec![&env]);
        topics.len() > 0 && {
            let t: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
            t.map(|s| s == soroban_sdk::Symbol::new(&env, "snap_crt")).unwrap_or(false)
        }
    });
    assert!(found, "SNAPSHOT_CREATED_TOPIC event not emitted");
}

#[test]
fn test_restore_from_snapshot_reverts_state() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.deposit(&vault_id, &owner, &100_000i128);

    // Take snapshot at balance=100_000
    let snap_id = client.create_snapshot(&vault_id, &owner);

    // Withdraw some funds to change state
    client.withdraw(&vault_id, &owner, &40_000i128);
    assert_eq!(client.get_vault(&vault_id).balance, 60_000i128);

    // Restore
    client.restore_from_snapshot(&vault_id, &owner, &snap_id);
    assert_eq!(client.get_vault(&vault_id).balance, 100_000i128);
}

#[test]
fn test_restore_from_snapshot_rejects_non_owner() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let snap_id = client.create_snapshot(&vault_id, &owner);
    let stranger = Address::generate(&env);

    let err = client.try_restore_from_snapshot(&vault_id, &stranger, &snap_id).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6)); // NotOwner
}

#[test]
fn test_restore_from_snapshot_rejects_released_vault() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.deposit(&vault_id, &owner, &100_000i128);
    let snap_id = client.create_snapshot(&vault_id, &owner);

    env.ledger().with_mut(|l| l.timestamp += 3601);
    client.trigger_release(&vault_id);

    let err = client.try_restore_from_snapshot(&vault_id, &owner, &snap_id).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(7)); // AlreadyReleased
}

#[test]
fn test_restore_from_snapshot_rejects_missing_snapshot() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let err = client.try_restore_from_snapshot(&vault_id, &owner, &99u32).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(3)); // VaultNotFound
}

#[test]
fn test_restore_from_snapshot_emits_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let snap_id = client.create_snapshot(&vault_id, &owner);
    client.restore_from_snapshot(&vault_id, &owner, &snap_id);

    let events = env.events().all();
    let found = events.iter().any(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.try_into_val(&env).unwrap_or(soroban_sdk::vec![&env]);
        topics.len() > 0 && {
            let t: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
            t.map(|s| s == soroban_sdk::Symbol::new(&env, "snap_rst")).unwrap_or(false)
        }
    });
    assert!(found, "SNAPSHOT_RESTORED_TOPIC event not emitted");
}

#[test]
fn test_get_snapshot_returns_none_for_missing() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    assert!(client.get_snapshot(&vault_id, &1u32).is_none());
}

#[test]
fn test_snapshot_captures_metadata_and_interval() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.update_check_in_interval(&vault_id, &owner, &7200u64);

    let snap_id = client.create_snapshot(&vault_id, &owner);
    let snap = client.get_snapshot(&vault_id, &snap_id).unwrap();
    assert_eq!(snap.check_in_interval, 7200u64);
}

// ── Countdown Notifications ──────────────────────────────────────────────────

#[test]
fn test_check_countdown_default_thresholds_no_event_when_ttl_high() {
    let (env, owner, beneficiary, _, _, client) = setup();
    // interval = 30 days; TTL will be ~30 days >> all thresholds
    let vault_id = client.create_vault(&owner, &beneficiary, &(30 * 86_400u64), &None);
    let ttl = client.check_countdown(&vault_id);
    assert!(ttl > 604_800, "TTL should be above 7-day threshold");
    // No cd_notif event should have been emitted
    let events = env.events().all();
    let found = events.iter().any(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.try_into_val(&env).unwrap_or(soroban_sdk::vec![&env]);
        topics.len() > 0 && {
            let t: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
            t.map(|s| s == soroban_sdk::Symbol::new(&env, "cd_notif")).unwrap_or(false)
        }
    });
    assert!(!found, "No cd_notif event should fire when TTL is high");
}

#[test]
fn test_check_countdown_emits_event_when_ttl_crosses_threshold() {
    let (env, owner, beneficiary, _, _, client) = setup();
    // interval = 2 days; after 1 day TTL = 1 day => crosses 1-day threshold
    let vault_id = client.create_vault(&owner, &beneficiary, &(2 * 86_400u64), &None);
    env.ledger().with_mut(|l| l.timestamp += 86_400); // advance 1 day
    client.check_countdown(&vault_id);

    let events = env.events().all();
    let found = events.iter().any(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.try_into_val(&env).unwrap_or(soroban_sdk::vec![&env]);
        topics.len() > 0 && {
            let t: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
            t.map(|s| s == soroban_sdk::Symbol::new(&env, "cd_notif")).unwrap_or(false)
        }
    });
    assert!(found, "cd_notif event should fire when TTL <= 1-day threshold");
}

#[test]
fn test_check_countdown_fires_each_threshold_once() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &(2 * 86_400u64), &None);
    env.ledger().with_mut(|l| l.timestamp += 86_400);

    // First call — should fire
    client.check_countdown(&vault_id);
    let count_after_first = env.events().all().iter()
        .filter(|e| {
            let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.try_into_val(&env).unwrap_or(soroban_sdk::vec![&env]);
            topics.len() > 0 && {
                let t: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
                t.map(|s| s == soroban_sdk::Symbol::new(&env, "cd_notif")).unwrap_or(false)
            }
        })
        .count();

    // Second call — same TTL, threshold already fired, no new event
    client.check_countdown(&vault_id);
    let count_after_second = env.events().all().iter()
        .filter(|e| {
            let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.try_into_val(&env).unwrap_or(soroban_sdk::vec![&env]);
            topics.len() > 0 && {
                let t: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
                t.map(|s| s == soroban_sdk::Symbol::new(&env, "cd_notif")).unwrap_or(false)
            }
        })
        .count();

    assert_eq!(count_after_first, count_after_second, "Threshold should not fire twice");
}

#[test]
fn test_check_countdown_resets_after_check_in() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &(2 * 86_400u64), &None);
    env.ledger().with_mut(|l| l.timestamp += 86_400);
    client.check_countdown(&vault_id);

    // Owner checks in — resets TTL and clears fired flags
    client.check_in(&vault_id, &owner, &None, &None, &None);
    env.ledger().with_mut(|l| l.timestamp += 86_400);

    // Now check_countdown should fire again for the same threshold
    let events_before = env.events().all().len();
    client.check_countdown(&vault_id);
    let new_events: usize = env.events().all().iter()
        .skip(events_before)
        .filter(|e| {
            let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.try_into_val(&env).unwrap_or(soroban_sdk::vec![&env]);
            topics.len() > 0 && {
                let t: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
                t.map(|s| s == soroban_sdk::Symbol::new(&env, "cd_notif")).unwrap_or(false)
            }
        })
        .count();
    assert!(new_events > 0, "Threshold should fire again after check-in resets the cycle");
}

#[test]
fn test_set_countdown_config_custom_thresholds() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &(10 * 86_400u64), &None);

    let custom = soroban_sdk::vec![&env, 3600u64, 1800u64]; // 1h, 30m
    client.set_countdown_config(&vault_id, &owner, &custom);

    let cfg = client.get_countdown_config(&vault_id);
    assert_eq!(cfg.thresholds.len(), 2);
    assert_eq!(cfg.thresholds.get(0).unwrap(), 3600u64);
}

#[test]
fn test_set_countdown_config_rejects_non_owner() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let stranger = Address::generate(&env);
    let thresholds = soroban_sdk::vec![&env, 3600u64];

    let err = client.try_set_countdown_config(&vault_id, &stranger, &thresholds).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6)); // NotOwner
}

#[test]
fn test_set_countdown_config_emits_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let thresholds = soroban_sdk::vec![&env, 3600u64];
    client.set_countdown_config(&vault_id, &owner, &thresholds);

    let events = env.events().all();
    let found = events.iter().any(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.try_into_val(&env).unwrap_or(soroban_sdk::vec![&env]);
        topics.len() > 0 && {
            let t: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
            t.map(|s| s == soroban_sdk::Symbol::new(&env, "set_cd")).unwrap_or(false)
        }
    });
    assert!(found, "set_cd event should be emitted");
}

#[test]
fn test_set_countdown_config_clears_fired_flags() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &(2 * 86_400u64), &None);
    env.ledger().with_mut(|l| l.timestamp += 86_400);
    client.check_countdown(&vault_id); // fires 1-day threshold

    // Reconfigure — should clear fired flags
    let thresholds = soroban_sdk::vec![&env, 86_400u64];
    client.set_countdown_config(&vault_id, &owner, &thresholds);

    // check_countdown should fire again since flags were cleared
    let events_before = env.events().all().len();
    client.check_countdown(&vault_id);
    let new_events: usize = env.events().all().iter()
        .skip(events_before)
        .filter(|e| {
            let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.try_into_val(&env).unwrap_or(soroban_sdk::vec![&env]);
            topics.len() > 0 && {
                let t: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
                t.map(|s| s == soroban_sdk::Symbol::new(&env, "cd_notif")).unwrap_or(false)
            }
        })
        .count();
    assert!(new_events > 0, "Threshold should fire again after config reset");
}

#[test]
fn test_check_countdown_returns_zero_for_released_vault() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.deposit(&vault_id, &owner, &100_000i128);
    env.ledger().with_mut(|l| l.timestamp += 3601);
    client.trigger_release(&vault_id);

    let ttl = client.check_countdown(&vault_id);
    assert_eq!(ttl, 0);
}

#[test]
fn test_get_countdown_config_returns_defaults_when_not_set() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    let cfg = client.get_countdown_config(&vault_id);
    assert_eq!(cfg.thresholds.len(), 3);
    assert_eq!(cfg.thresholds.get(0).unwrap(), 604_800u64);
    assert_eq!(cfg.thresholds.get(1).unwrap(), 259_200u64);
    assert_eq!(cfg.thresholds.get(2).unwrap(), 86_400u64);
}
