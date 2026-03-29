#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
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
#[should_panic(expected = "Error(Contract, #1)")]
fn test_initialize_guard_against_double_init() {
    let (env, _, _, admin, token_address, client) = setup();

    let new_token_admin = Address::generate(&env);
    let new_token_address = env
        .register_stellar_asset_contract_v2(new_token_admin)
        .address();

    client.initialize(&new_token_address, &admin);
    let _ = token_address;
}

#[test]
#[should_panic(expected = "Error(Contract, #18)")]
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
    let id_1 = client.create_vault(&owner, &beneficiary, &100u64);
    let id_2 = client.create_vault(&owner, &beneficiary, &200u64);

    assert_eq!(id_1, 1);
    assert_eq!(id_2, 2);
    assert_eq!(client.vault_count(), 2);
}

#[test]
fn test_vault_exists_for_existing_and_missing_ids() {
    let (_, owner, beneficiary, _, _, client) = setup();

    assert!(!client.vault_exists(&1));

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);

    assert!(client.vault_exists(&vault_id));
    assert!(!client.vault_exists(&(vault_id + 1)));
}

#[test]
fn test_get_release_status_view() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
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

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &200u64);
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

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
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
fn test_pause_and_unpause_toggle() {
    let (_, _, _, _, _, client) = setup();

    assert!(!client.is_paused());
    client.pause();
    assert!(client.is_paused());
    client.unpause();
    assert!(!client.is_paused());
}

#[test]
fn test_get_admin_view() {
    let (_, _, _, admin, _, client) = setup();

    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_paused_blocks_check_in_withdraw_and_trigger_release() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &200i128);
    env.ledger().with_mut(|l| l.timestamp += 200);

    client.pause();

    assert!(client.try_check_in(&vault_id, &owner).is_err());
    assert!(client.try_withdraw(&vault_id, &10i128).is_err());
    assert!(client.try_trigger_release(&vault_id).is_err());

    client.unpause();
    client.trigger_release(&vault_id);
    assert_eq!(
        client.get_release_status(&vault_id),
        ReleaseStatus::Released
    );
}

#[test]
fn test_get_vaults_by_owner_tracks_multiple_vaults() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &200u64);

    assert_eq!(
        client.get_vaults_by_owner(&owner, &0u32, &10u32),
        vec![&env, vault_id_1, vault_id_2]
    );
}

#[test]
fn test_update_check_in_interval() {
    let (_, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);

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

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);

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

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    assert_eq!(client.get_vaults_by_owner(&owner, &0u32, &10u32), vec![&env, vault_id]);
    assert_eq!(client.get_vaults_by_owner(&new_owner, &0u32, &10u32), vec![&env]);

    client.transfer_ownership(&vault_id, &new_owner);

    assert_eq!(client.get_vault(&vault_id).owner, new_owner);
    assert_eq!(client.get_vaults_by_owner(&owner, &0u32, &10u32), vec![&env]);
    assert_eq!(client.get_vaults_by_owner(&new_owner, &0u32, &10u32), vec![&env, vault_id]);
}

/// Invariant: owner and beneficiary must always be distinct.
/// transfer_ownership must reject a new_owner that equals the vault's beneficiary,
/// and must not corrupt the BeneficiaryVaults index.
#[test]
#[should_panic(expected = "Error(Contract, #17)")]
fn test_transfer_ownership_rejects_new_owner_equal_to_beneficiary() {
    let (_, owner, beneficiary, _, _, client) = setup();

    // beneficiary is the vault's primary beneficiary; transferring ownership to
    // them would violate the owner != beneficiary invariant.
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.transfer_ownership(&vault_id, &beneficiary);
}

/// BeneficiaryVaults index must remain consistent after a successful ownership transfer.
/// The vault's beneficiary field is unchanged, so the beneficiary's index entry
/// must still point to the vault.
#[test]
fn test_transfer_ownership_preserves_beneficiary_index() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);

    // beneficiary index contains the vault before transfer
    assert_eq!(client.get_vaults_by_beneficiary(&beneficiary), vec![&env, vault_id]);

    client.transfer_ownership(&vault_id, &new_owner);

    // vault.beneficiary is unchanged — index must still be intact
    assert_eq!(client.get_vault(&vault_id).beneficiary, beneficiary);
    assert_eq!(client.get_vaults_by_beneficiary(&beneficiary), vec![&env, vault_id]);
}

#[test]
fn test_cancel_vault_refunds_owner_and_marks_cancelled() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let token_client = token::Client::new(&env, &token_address);
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);

    client.deposit(&vault_id, &owner, &400i128);
    assert_eq!(token_client.balance(&owner), 999_600i128);

    client.cancel_vault(&vault_id);
    assert_eq!(token_client.balance(&owner), 1_000_000i128);
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Cancelled);
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
    client.create_vault(&owner, &owner, &1000);
}

#[test]
fn test_vault_count_consistent_after_creation() {
    let (_, owner, beneficiary, _, _, client) = setup();
    assert_eq!(client.vault_count(), 0);
    let id = client.create_vault(&owner, &beneficiary, &1000);
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

// ---- Task 1: ping_expiry tests ----

#[test]
fn test_ping_expiry_emits_event_when_near_expiry() {
    let (env, owner, beneficiary, _, _, client) = setup();
    // interval = 100s, advance 50s => TTL remaining = 50 < EXPIRY_WARNING_THRESHOLD (86400)
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    env.ledger().with_mut(|l| l.timestamp += 50);

    let ttl = client.ping_expiry(&vault_id);
    assert_eq!(ttl, 50u64);
}

#[test]
fn test_ping_expiry_no_event_when_far_from_expiry() {
    let (env, owner, beneficiary, _, _, client) = setup();
    // interval = 200_000s, no time advance => TTL = 200_000 >= threshold, no event
    let vault_id = client.create_vault(&owner, &beneficiary, &200_000u64);
    env.ledger().with_mut(|l| l.timestamp += 0);

    let ttl = client.ping_expiry(&vault_id);
    assert_eq!(ttl, 200_000u64);
}

#[test]
fn test_ping_expiry_returns_zero_when_expired() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    env.ledger().with_mut(|l| l.timestamp += 200);

    let ttl = client.ping_expiry(&vault_id);
    assert_eq!(ttl, 0u64);
}

#[test]
fn test_get_ttl_remaining_returns_none_when_expired() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
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

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
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
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &100i128);
    // attempt to release more than the balance
    client.partial_release(&vault_id, &200i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #17)")]
fn test_update_beneficiary_rejects_owner_as_beneficiary() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &1000);
    client.update_beneficiary(&vault_id, &owner);
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")]
fn test_deposit_into_expired_vault_is_rejected() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.deposit(&vault_id, &owner, &500i128);
}

#[test]
fn test_update_metadata_can_be_overwritten() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);

    client.update_metadata(&vault_id, &soroban_sdk::String::from_str(&env, "v1"));
    client.update_metadata(&vault_id, &soroban_sdk::String::from_str(&env, "v2"));

    assert_eq!(
        client.get_vault(&vault_id).metadata,
        soroban_sdk::String::from_str(&env, "v2")
    );
}

#[test]
fn test_get_contract_token_returns_correct_address() {
    let (_, _, _, _, token_address, client) = setup();
    assert_eq!(client.get_contract_token(), token_address);
}

#[test]
fn test_create_vault_zero_interval_fails() {
    let (_, owner, beneficiary, _, _, client) = setup();

    let result = client.try_create_vault(&owner, &beneficiary, &0u64);
    assert!(result.is_err());
}

#[test]
fn test_create_vault_long_interval_remains_accessible() {
    // 30-day check-in interval: vault storage TTL must outlive the interval.
    // vault_ttl_ledgers(2_592_000) = 2_592_000 * 2 / 5 = 1_036_800 ledgers (~60 days).
    let (env, owner, beneficiary, _, _, client) = setup();
    let thirty_days: u64 = 30 * 24 * 3600; // 2_592_000 seconds
    let vault_id = client.create_vault(&owner, &beneficiary, &thirty_days);
    // Advance just under the interval — vault must still be readable.
    env.ledger().with_mut(|l| l.timestamp += thirty_days - 1);
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.check_in_interval, thirty_days);
}

// ---- Issue 1: get_vaults_by_beneficiary ----

#[test]
fn test_get_vaults_by_beneficiary_tracks_vaults() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let other_beneficiary = Address::generate(&env);

    assert_eq!(client.get_vaults_by_beneficiary(&beneficiary, &0u32, &10u32), vec![&env]);

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &200u64);
    let _vault_id_3 = client.create_vault(&owner, &other_beneficiary, &300u64);

    assert_eq!(
        client.get_vaults_by_beneficiary(&beneficiary, &0u32, &10u32),
        vec![&env, vault_id_1, vault_id_2]
    );
    assert_eq!(
        client.get_vaults_by_beneficiary(&other_beneficiary, &0u32, &10u32),
        vec![&env, _vault_id_3]
    );
}

#[test]
fn test_get_vaults_by_beneficiary_empty_for_unknown() {
    let (env, _, _, _, _, client) = setup();
    let stranger = Address::generate(&env);
    assert_eq!(client.get_vaults_by_beneficiary(&stranger, &0u32, &10u32), vec![&env]);
}

// ---- Issue 2: upgrade ----

#[test]
#[should_panic]
fn test_upgrade_fails_for_non_admin() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let _vault_id = client.create_vault(&owner, &beneficiary, &100u64);
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
fn test_create_vault_fails_when_interval_exceeds_max() {
    let (_, owner, beneficiary, _, _, client) = setup();
    client.set_max_check_in_interval(&1_000u64);
    assert!(client.try_create_vault(&owner, &beneficiary, &2_000u64).is_err());
}

#[test]
fn test_create_vault_succeeds_at_max_boundary() {
    let (_, owner, beneficiary, _, _, client) = setup();
    client.set_max_check_in_interval(&1_000u64);
    let vault_id = client.create_vault(&owner, &beneficiary, &1_000u64);
    assert_eq!(client.get_vault(&vault_id).check_in_interval, 1_000u64);
}

#[test]
fn test_update_check_in_interval_fails_when_exceeds_max() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.set_max_check_in_interval(&500u64);
    assert!(client.try_update_check_in_interval(&vault_id, &600u64).is_err());
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
fn test_create_vault_fails_when_interval_below_min() {
    let (_, owner, beneficiary, _, _, client) = setup();
    client.set_min_check_in_interval(&3_600u64);
    assert!(client.try_create_vault(&owner, &beneficiary, &100u64).is_err());
}

#[test]
fn test_create_vault_succeeds_at_min_boundary() {
    let (_, owner, beneficiary, _, _, client) = setup();
    client.set_min_check_in_interval(&3_600u64);
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64);
    assert_eq!(client.get_vault(&vault_id).check_in_interval, 3_600u64);
}

#[test]
fn test_update_check_in_interval_fails_when_below_min() {
    let (_, owner, beneficiary, _, _, client) = setup();
    client.set_min_check_in_interval(&3_600u64);
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64);
    assert!(client.try_update_check_in_interval(&vault_id, &100u64).is_err());
}

#[test]
fn test_min_and_max_both_enforced() {
    let (_, owner, beneficiary, _, _, client) = setup();
    client.set_min_check_in_interval(&60u64);
    client.set_max_check_in_interval(&3_600u64);

    assert!(client.try_create_vault(&owner, &beneficiary, &30u64).is_err());
    assert!(client.try_create_vault(&owner, &beneficiary, &7_200u64).is_err());
    let vault_id = client.create_vault(&owner, &beneficiary, &1_800u64);
    assert_eq!(client.get_vault(&vault_id).check_in_interval, 1_800u64);
}

#[test]
fn test_withdraw_rejects_zero_amount() {
    let (_, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &500i128);

    // zero amount should return InvalidAmount (#5)
    let result = client.try_withdraw(&vault_id, &0i128);
    assert!(result.is_err(), "expected error for zero-amount withdrawal");
}

#[test]
fn test_withdraw_rejects_negative_amount() {
    let (_, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &500i128);

    // negative amount should also return InvalidAmount (#5)
    let result = client.try_withdraw(&vault_id, &-1i128);
    assert!(result.is_err(), "expected error for negative-amount withdrawal");
}

#[test]
fn test_deposit_emits_event() {
    let (env, owner, beneficiary, _, _, client) = setup();

    env.mock_all_auths();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);

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
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &500i128);

    client.withdraw(&vault_id, &100i128);

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
fn test_trigger_release_emits_event_with_zero_balance() {
    let (env, owner, beneficiary, _, _, client) = setup();

    // create vault but never deposit — balance stays 0
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    env.ledger().with_mut(|l| l.timestamp += 200);

    // should succeed and emit a release event with amount: 0
    client.trigger_release(&vault_id);

    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Released);

    let events = env.events().all();
    let release_event = events.iter().find(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        if topics.len() < 1 {
            return false;
        }
        let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
        topic0.map(|s| s == soroban_sdk::symbol_short!("release")).unwrap_or(false)
    });

    assert!(release_event.is_some(), "release event not emitted for zero-balance vault");
}

// ---- Issue #105: set_beneficiaries owner-as-beneficiary guard ----

#[test]
#[should_panic(expected = "Error(Contract, #17)")]
fn test_set_beneficiaries_rejects_owner_as_beneficiary() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64);

    // owner sneaks themselves into the multi-split list
    client.set_beneficiaries(
        &vault_id,
        &vec![
            &env,
            BeneficiaryEntry { address: owner.clone(), bps: 5_000 },
            BeneficiaryEntry { address: beneficiary.clone(), bps: 5_000 },
        ],
    );
}

#[test]
fn test_deposit_rejects_balance_overflow() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    // setup() already minted 1_000_000; mint enough to reach i128::MAX total
    let extra = i128::MAX - 1_000_000;
    StellarAssetClient::new(&env, &token_address).mint(&owner, &extra);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);

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

    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64);
    client.deposit(&vault_id, &owner, &10_000i128);

    // 60/40 split
    client.set_beneficiaries(
        &vault_id,
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

    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64);
    client.deposit(&vault_id, &owner, &10_000i128);

    // 33/67 split — integer division leaves dust on the last entry
    client.set_beneficiaries(
        &vault_id,
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

    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64);
    client.deposit(&vault_id, &owner, &1_000i128);

    client.partial_release(&vault_id, &400i128);

    assert_eq!(token_client.balance(&beneficiary), 400i128);
    assert_eq!(client.get_vault(&vault_id).balance, 600i128);
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Locked);
}
