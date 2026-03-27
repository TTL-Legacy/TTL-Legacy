#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{self, StellarAssetClient},
    vec, Address, BytesN, Env,
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
        client.get_vaults_by_owner(&owner),
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
fn test_transfer_ownership_updates_owner_and_owner_index() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let new_owner = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    assert_eq!(client.get_vaults_by_owner(&owner), vec![&env, vault_id]);
    assert_eq!(client.get_vaults_by_owner(&new_owner), vec![&env]);

    client.transfer_ownership(&vault_id, &new_owner);

    assert_eq!(client.get_vault(&vault_id).owner, new_owner);
    assert_eq!(client.get_vaults_by_owner(&owner), vec![&env]);
    assert_eq!(client.get_vaults_by_owner(&new_owner), vec![&env, vault_id]);
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

    client.with_source_address(&new_admin).accept_admin();
    assert_eq!(client.get_admin(), new_admin.clone());
    assert_eq!(client.get_pending_admin(), None);

    client.with_source_address(&new_admin).pause();
    assert!(client.is_paused());
    client.with_source_address(&new_admin).unpause();
    assert!(!client.is_paused());
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_accept_admin_fails_when_no_pending_admin() {
    let (env, _, _, _, _, client) = setup();
    let new_admin = Address::generate(&env);

    client.with_source_address(&new_admin).accept_admin();
}

#[test]
#[should_panic]
fn test_accept_admin_fails_when_called_by_wrong_address() {
    let (env, _, _, _, _, client) = setup();
    let new_admin = Address::generate(&env);
    let wrong_admin = Address::generate(&env);

    client.propose_admin(&new_admin);

    client.with_source_address(&wrong_admin).accept_admin();
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

    client.with_source_address(&new_admin_2).accept_admin();
    assert_eq!(client.get_admin(), new_admin_2.clone());
    assert_eq!(client.get_pending_admin(), None);

    client.with_source_address(&new_admin_2).pause();
    assert!(client.is_paused());
}

// ---- Task 1: ping_expiry tests ----

#[test]
fn test_ping_expiry_emits_event_when_near_expiry() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    env.ledger().with_mut(|l| l.timestamp += 50);

    let ttl = client.ping_expiry(&vault_id);
    assert_eq!(ttl, 50u64);
}

#[test]
fn test_ping_expiry_no_event_when_far_from_expiry() {
    let (env, owner, beneficiary, _, _, client) = setup();
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
    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Locked);
}

#[test]
fn test_partial_release_fails_if_insufficient_balance() {
    let (_, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &100i128);

    assert!(client.try_partial_release(&vault_id, &500i128).is_err());
}

#[test]
fn test_partial_release_fails_after_release() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &500i128);
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    assert!(client.try_partial_release(&vault_id, &100i128).is_err());
}

#[test]
fn test_partial_release_multiple_times_reduces_balance() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &1_000i128);

    client.partial_release(&vault_id, &200i128);
    client.partial_release(&vault_id, &300i128);

    assert_eq!(token_client.balance(&beneficiary), 500i128);
    assert_eq!(client.get_vault(&vault_id).balance, 500i128);
    let _ = env;
}

// ---- Task 3: set_beneficiaries / multi-split tests ----

#[test]
fn test_set_beneficiaries_and_trigger_release_splits_funds() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &10_000i128);

    let entries = vec![
        &env,
        types::BeneficiaryEntry { address: b1.clone(), bps: 6_000 },
        types::BeneficiaryEntry { address: b2.clone(), bps: 4_000 },
    ];
    client.set_beneficiaries(&vault_id, &entries);

    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    assert_eq!(token_client.balance(&b1), 6_000i128);
    assert_eq!(token_client.balance(&b2), 4_000i128);
    assert_eq!(client.get_vault(&vault_id).balance, 0i128);
}

#[test]
fn test_set_beneficiaries_rejects_invalid_bps() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let b1 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);

    let entries = vec![
        &env,
        types::BeneficiaryEntry { address: b1.clone(), bps: 5_000 },
    ];
    assert!(client.try_set_beneficiaries(&vault_id, &entries).is_err());
}

#[test]
fn test_set_beneficiaries_three_way_split_remainder_goes_to_last() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let token_client = token::Client::new(&env, &token_address);

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);
    let b3 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &10_001i128);

    let entries = vec![
        &env,
        types::BeneficiaryEntry { address: b1.clone(), bps: 3_333 },
        types::BeneficiaryEntry { address: b2.clone(), bps: 3_333 },
        types::BeneficiaryEntry { address: b3.clone(), bps: 3_334 },
    ];
    client.set_beneficiaries(&vault_id, &entries);

    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    let total = token_client.balance(&b1) + token_client.balance(&b2) + token_client.balance(&b3);
    assert_eq!(total, 10_001i128);
}

// ---- Task 4: metadata tests ----

#[test]
fn test_create_vault_has_empty_metadata() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    assert_eq!(
        client.get_vault(&vault_id).metadata,
        soroban_sdk::String::from_str(&env, "")
    );
}

#[test]
fn test_update_metadata_stores_value() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);

    let label = soroban_sdk::String::from_str(&env, "ipfs://QmTestHash");
    client.update_metadata(&vault_id, &label);

    assert_eq!(client.get_vault(&vault_id).metadata, label);
}

#[test]
fn test_update_metadata_fails_after_release() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &500i128);
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    let label = soroban_sdk::String::from_str(&env, "too late");
    assert!(client.try_update_metadata(&vault_id, &label).is_err());
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

// ---- trigger_release / check_in guard tests ----

#[test]
#[should_panic(expected = "vault not yet expired")]
fn test_trigger_release_panics_before_expiry() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &500i128);
    env.ledger().with_mut(|l| l.timestamp += 50);

    client.trigger_release(&vault_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_check_in_panics_after_release() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64);
    client.deposit(&vault_id, &owner, &500i128);
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    client.check_in(&vault_id, &owner);
}

// ---- Issue 1: get_vaults_by_beneficiary ----

#[test]
fn test_get_vaults_by_beneficiary_tracks_vaults() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let other_beneficiary = Address::generate(&env);

    assert_eq!(client.get_vaults_by_beneficiary(&beneficiary), vec![&env]);

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &100u64);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &200u64);
    let vault_id_3 = client.create_vault(&owner, &other_beneficiary, &300u64);

    assert_eq!(
        client.get_vaults_by_beneficiary(&beneficiary),
        vec![&env, vault_id_1, vault_id_2]
    );
    assert_eq!(
        client.get_vaults_by_beneficiary(&other_beneficiary),
        vec![&env, vault_id_3]
    );
}

#[test]
fn test_get_vaults_by_beneficiary_empty_for_unknown() {
    let (env, _, _, _, _, client) = setup();
    let stranger = Address::generate(&env);
    assert_eq!(client.get_vaults_by_beneficiary(&stranger), vec![&env]);
}

// ---- Issue 2: upgrade ----

#[test]
#[should_panic]
fn test_upgrade_fails_for_non_admin() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let _vault_id = client.create_vault(&owner, &beneficiary, &100u64);

    let fake_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.with_source_address(&owner).upgrade(&fake_hash);
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
