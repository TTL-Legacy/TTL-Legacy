//! Tests for structured event emission - Issues #1323, #1324, #1325, #1326.
//! Tests for structured event emission - Issues #1323, #1324, #1325, #1326.
//!
//! Verifies that:
//! - `create_vault` emits a `VaultCreatedEvent` (#1325)
//! - `check_in` emits a `CheckInEvent` (#1323)
//! - `trigger_release` emits a `ReleaseEvent` (#1324)
//! - `update_beneficiary` emits a `BeneficiaryUpdatedEvent` (#1326)

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, BytesN, Env, IntoVal, TryIntoVal,
};

// ── helpers ──────────────────────────────────────────────────────────────────

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

/// Find the first event whose first topic equals `target_topic` and deserialize its data.
fn find_event_by_topic<T>(env: &Env, target_topic: soroban_sdk::Symbol) -> Option<T>
where
    T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>,
{
    env.events().all().iter().find_map(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(env);
        let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0)?.try_into_val(env);
        if topic0.ok()? == target_topic {
            e.2.try_into_val(env).ok()
        } else {
            None
        }
    })
}

// ── Issue #1325: VaultCreatedEvent ───────────────────────────────────────────

/// `create_vault` must emit a `VaultCreatedEvent` containing the vault_id,
/// owner, beneficiary, and check_in_interval.
#[test]
fn test_create_vault_emits_vault_created_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let interval = 3_600u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let event: VaultCreatedEvent = find_event_by_topic(&env, types::VAULT_CREATED_TOPIC)
        .expect("VaultCreatedEvent not emitted by create_vault");

    assert_eq!(event.vault_id, vault_id, "vault_id mismatch");
    assert_eq!(event.owner, owner, "owner mismatch");
    assert_eq!(event.beneficiary, beneficiary, "beneficiary mismatch");
    assert_eq!(
        event.check_in_interval, interval,
        "check_in_interval mismatch"
    );
}

/// Each new vault creation emits its own `VaultCreatedEvent` with the correct vault_id.
#[test]
fn test_create_vault_event_vault_id_increments() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &3_600u64, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &7_200u64, &None);

    // Collect all VaultCreatedEvent data values
    let vault_events: alloc::vec::Vec<VaultCreatedEvent> = env
        .events()
        .all()
        .iter()
        .filter_map(|e| {
            let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
            let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0)?.try_into_val(&env);
            if topic0.ok()? == types::VAULT_CREATED_TOPIC {
                e.2.try_into_val(&env).ok()
            } else {
                None
            }
        })
        .collect();

    assert_eq!(vault_events.len(), 2, "expected 2 VaultCreatedEvent events");
    assert_eq!(vault_events[0].vault_id, vault_id_1);
    assert_eq!(vault_events[1].vault_id, vault_id_2);
    assert_eq!(vault_events[0].check_in_interval, 3_600u64);
    assert_eq!(vault_events[1].check_in_interval, 7_200u64);
}

// ── Issue #1323: CheckInEvent ─────────────────────────────────────────────────

/// `check_in` must emit a `CheckInEvent` via `CHECK_IN_RECORDED_TOPIC` containing
/// vault_id, new_ttl (last_check_in + check_in_interval), and caller.
#[test]
fn test_check_in_emits_check_in_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    env.ledger().with_mut(|l| l.timestamp += 10);
    let before_check_in = env.ledger().timestamp();

    client.check_in(
        &vault_id,
        &owner,
        &BytesN::from_array(&env, &[1u8; 32]),
        &0u64,
    );

    // CHECK_IN_RECORDED_TOPIC is the event that carries CheckInEvent data.
    let event: CheckInEvent = find_event_by_topic(&env, types::CHECK_IN_RECORDED_TOPIC)
        .expect("CheckInEvent not emitted by check_in");

    assert_eq!(event.vault_id, vault_id, "vault_id mismatch");
    assert_eq!(event.caller, owner, "caller mismatch");
    // new_ttl must equal the timestamp at check-in + interval
    assert_eq!(
        event.new_ttl,
        before_check_in + interval,
        "new_ttl should equal last_check_in + check_in_interval"
    );
}

/// `check_in` emits CHECK_IN_TOPIC (timestamp) in addition to the structured CheckInEvent.
#[test]
fn test_check_in_emits_both_check_in_topic_and_check_in_event() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3_600u64, &None);

    env.ledger().with_mut(|l| l.timestamp += 10);
    client.check_in(
        &vault_id,
        &owner,
        &BytesN::from_array(&env, &[1u8; 32]),
        &0u64,
    );

    let has_check_in_topic = env.events().all().iter().any(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        topics
            .get(0)
            .and_then(|v| v.try_into_val(&env).ok())
            .map(|s: soroban_sdk::Symbol| s == types::CHECK_IN_TOPIC)
            .unwrap_or(false)
    });
    assert!(has_check_in_topic, "CHECK_IN_TOPIC not emitted");

    let has_recorded_topic = env.events().all().iter().any(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        topics
            .get(0)
            .and_then(|v| v.try_into_val(&env).ok())
            .map(|s: soroban_sdk::Symbol| s == types::CHECK_IN_RECORDED_TOPIC)
            .unwrap_or(false)
    });
    assert!(has_recorded_topic, "CHECK_IN_RECORDED_TOPIC not emitted");
}

/// Successive check-ins each emit a `CheckInEvent` with an updated new_ttl.
#[test]
fn test_check_in_event_new_ttl_advances_with_each_check_in() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // First check-in
    env.ledger().with_mut(|l| l.timestamp += 10);
    let ts1 = env.ledger().timestamp();
    client.check_in(
        &vault_id,
        &owner,
        &BytesN::from_array(&env, &[1u8; 32]),
        &0u64,
    );

    let e1: CheckInEvent = find_event_by_topic(&env, types::CHECK_IN_RECORDED_TOPIC)
        .expect("first CheckInEvent missing");
    assert_eq!(e1.new_ttl, ts1 + interval);

    // Second check-in
    env.ledger().with_mut(|l| l.timestamp += 100);
    let ts2 = env.ledger().timestamp();
    client.check_in(
        &vault_id,
        &owner,
        &BytesN::from_array(&env, &[1u8; 32]),
        &1u64,
    );

    // Find the last CHECK_IN_RECORDED_TOPIC event
    let last_event: Option<CheckInEvent> = env.events().all().iter().rev().find_map(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0)?.try_into_val(&env);
        if topic0.ok()? == types::CHECK_IN_RECORDED_TOPIC {
            e.2.try_into_val(&env).ok()
        } else {
            None
        }
    });

    let e2 = last_event.expect("second CheckInEvent missing");
    assert_eq!(e2.new_ttl, ts2 + interval);
    assert!(
        e2.new_ttl > e1.new_ttl,
        "new_ttl should advance after each check-in"
    );
}

// ── Issue #1324: ReleaseEvent ─────────────────────────────────────────────────

/// `trigger_release` must emit a `ReleaseEvent` containing vault_id, beneficiary,
/// and amount when funds are transferred to the beneficiary.
#[test]
fn test_trigger_release_emits_release_event() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    StellarAssetClient::new(&env, &token_address).mint(&owner, &5_000i128);
    client.deposit(&vault_id, &owner, &5_000i128);

    // Expire the vault
    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    let event: ReleaseEvent = find_event_by_topic(&env, types::RELEASE_TOPIC)
        .expect("ReleaseEvent not emitted by trigger_release");

    assert_eq!(event.vault_id, vault_id, "vault_id mismatch");
    assert_eq!(event.beneficiary, beneficiary, "beneficiary mismatch");
    assert_eq!(event.amount, 5_000i128, "amount mismatch");
}

/// The `ReleaseEvent` amount matches the actual vault balance at the time of release.
#[test]
fn test_trigger_release_event_amount_matches_balance() {
    let (env, owner, beneficiary, _, token_address, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    let deposit_amount = 12_345i128;
    StellarAssetClient::new(&env, &token_address).mint(&owner, &deposit_amount);
    client.deposit(&vault_id, &owner, &deposit_amount);

    env.ledger().with_mut(|l| l.timestamp += 200);
    client.trigger_release(&vault_id);

    let event: ReleaseEvent =
        find_event_by_topic(&env, types::RELEASE_TOPIC).expect("ReleaseEvent not emitted");

    assert_eq!(event.amount, deposit_amount);
    assert_eq!(event.beneficiary, beneficiary);
}

// ── Issue #1326: BeneficiaryUpdatedEvent ──────────────────────────────────────

/// `update_beneficiary` must emit a `BeneficiaryUpdatedEvent` containing vault_id,
/// old_beneficiary, and new_beneficiary.
#[test]
fn test_update_beneficiary_emits_beneficiary_updated_event() {
    let (env, owner, old_beneficiary, _, _, client) = setup();
    let new_beneficiary = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &old_beneficiary, &100u64, &None);
    client.update_beneficiary(&vault_id, &owner, &new_beneficiary);

    let event: BeneficiaryUpdatedEvent =
        find_event_by_topic(&env, types::BENEFICIARY_UPDATED_TOPIC)
            .expect("BeneficiaryUpdatedEvent not emitted by update_beneficiary");

    assert_eq!(event.vault_id, vault_id, "vault_id mismatch");
    assert_eq!(
        event.old_beneficiary, old_beneficiary,
        "old_beneficiary mismatch"
    );
    assert_eq!(
        event.new_beneficiary, new_beneficiary,
        "new_beneficiary mismatch"
    );
}

/// The `BeneficiaryUpdatedEvent` correctly captures the before-and-after addresses.
#[test]
fn test_update_beneficiary_event_captures_old_and_new_address() {
    let (env, owner, original_beneficiary, _, _, client) = setup();
    let first_new_beneficiary = Address::generate(&env);
    let second_new_beneficiary = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &original_beneficiary, &100u64, &None);

    // First update: original → first_new
    client.update_beneficiary(&vault_id, &owner, &first_new_beneficiary);

    let first_event: BeneficiaryUpdatedEvent =
        find_event_by_topic(&env, types::BENEFICIARY_UPDATED_TOPIC)
            .expect("first BeneficiaryUpdatedEvent not emitted");
    assert_eq!(first_event.old_beneficiary, original_beneficiary);
    assert_eq!(first_event.new_beneficiary, first_new_beneficiary);
    assert_eq!(first_event.vault_id, vault_id);

    // Apply the pending update to actually switch the beneficiary
    env.ledger().with_mut(|l| l.timestamp += 86_401);
    client.apply_beneficiary_update(&vault_id, &owner);

    // Second update: first_new → second_new
    client.update_beneficiary(&vault_id, &owner, &second_new_beneficiary);

    // Find the latest BENEFICIARY_UPDATED_TOPIC event
    let second_event: Option<BeneficiaryUpdatedEvent> =
        env.events().all().iter().rev().find_map(|e| {
            let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
            let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0)?.try_into_val(&env);
            if topic0.ok()? == types::BENEFICIARY_UPDATED_TOPIC {
                e.2.try_into_val(&env).ok()
            } else {
                None
            }
        });

    let second_event = second_event.expect("second BeneficiaryUpdatedEvent not emitted");
    assert_eq!(second_event.old_beneficiary, first_new_beneficiary);
    assert_eq!(second_event.new_beneficiary, second_new_beneficiary);
}

/// `update_beneficiary` emits the event with the vault_id in the topic.
#[test]
fn test_update_beneficiary_event_topic_contains_vault_id() {
    let (env, owner, old_beneficiary, _, _, client) = setup();
    let new_beneficiary = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &old_beneficiary, &100u64, &None);

    client.update_beneficiary(&vault_id, &owner, &new_beneficiary);

    let found = env.events().all().iter().any(|e| {
        let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(&env);
        if topics.len() < 2 {
            return false;
        }
        let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
        let topic1: Result<u64, _> = topics.get(1).unwrap().try_into_val(&env);
        topic0
            .map(|s| s == types::BENEFICIARY_UPDATED_TOPIC)
            .unwrap_or(false)
            && topic1.map(|id| id == vault_id).unwrap_or(false)
    });

    assert!(
        found,
        "BENEFICIARY_UPDATED_TOPIC topic does not contain vault_id"
    );
}
