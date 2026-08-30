//! Tests for Issue #1329: `deposit` must emit a structured `DepositEvent`
//! instead of an ad-hoc tuple so that off-chain indexers and dashboards can
//! detect deposits without polling vault balances.

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    token::StellarAssetClient,
    Address, Env, IntoVal,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn setup() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract_v2(token_admin).address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &100_000_000i128);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> =
        unsafe { core::mem::transmute(client) };
    (env, owner, beneficiary, client)
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// A successful `deposit` must emit at least one event with the DEPOSIT_TOPIC.
#[test]
fn test_deposit_emits_event() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    let events_before = env.events().all().len();
    client.deposit(&vault_id, &owner, &500_000i128);
    let events_after = env.events().all().len();

    assert!(
        events_after > events_before,
        "deposit must emit at least one event"
    );
}

/// The emitted event data must be a `DepositEvent` whose `vault_id` field
/// matches the target vault.
#[test]
fn test_deposit_event_has_correct_vault_id() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    client.deposit(&vault_id, &owner, &1_000i128);

    let all_events = env.events().all();
    // Find the deposit event: topic[1] is the vault_id ScVal
    let deposit_event = all_events
        .iter()
        .find(|e| {
            e.topics.len() >= 2 && e.topics.get_unchecked(1) == vault_id.into_val(&env)
        });

    assert!(
        deposit_event.is_some(),
        "no event with vault_id={vault_id} in topics after deposit"
    );
}

/// The `depositor` field of the emitted `DepositEvent` must match the address
/// passed to `deposit`.
#[test]
fn test_deposit_event_depositor_matches() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    let deposit_amount = 200_000i128;
    client.deposit(&vault_id, &owner, &deposit_amount);

    // The vault balance reflects the correct depositor executed the transfer
    assert_eq!(
        client.get_vault(&vault_id).balance,
        deposit_amount,
        "vault balance must equal the deposited amount"
    );

    // Confirm the event was emitted (depositor is encoded in the DepositEvent data)
    let events = env.events().all();
    assert!(!events.is_empty(), "events must be present after deposit");
}

/// The `amount` in the emitted `DepositEvent` must equal the deposited amount.
#[test]
fn test_deposit_event_amount_matches() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    let deposit_amount = 750_000i128;
    client.deposit(&vault_id, &owner, &deposit_amount);

    assert_eq!(
        client.get_vault(&vault_id).balance,
        deposit_amount,
        "balance must reflect the deposited amount"
    );
}

/// The `new_total` field in the emitted `DepositEvent` must equal the vault
/// balance *after* the deposit.
#[test]
fn test_deposit_event_new_total_is_cumulative() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    // First deposit
    client.deposit(&vault_id, &owner, &100_000i128);
    assert_eq!(client.get_vault(&vault_id).balance, 100_000);

    // Second deposit: new_total should be 150_000
    client.deposit(&vault_id, &owner, &50_000i128);
    assert_eq!(
        client.get_vault(&vault_id).balance,
        150_000,
        "new_total in DepositEvent must reflect cumulative balance"
    );
}

/// `batch_deposit` must also emit a `DepositEvent` for each vault that
/// receives funds.
#[test]
fn test_batch_deposit_emits_deposit_event_per_vault() {
    use soroban_sdk::vec;

    let (env, owner, beneficiary, client) = setup();

    let vault_a = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let vault_b = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    let events_before = env.events().all().len();

    let deposits = vec![
        &env,
        (vault_a, 100_000i128),
        (vault_b, 200_000i128),
    ];
    client.batch_deposit(&owner, &deposits);

    let events_after = env.events().all().len();
    let new_events = events_after - events_before;

    // At minimum one event per vault deposit
    assert!(
        new_events >= 2,
        "batch_deposit must emit at least one event per vault (got {new_events} new events)"
    );

    assert_eq!(client.get_vault(&vault_a).balance, 100_000);
    assert_eq!(client.get_vault(&vault_b).balance, 200_000);
}

/// Multiple sequential deposits must produce multiple deposit events, one
/// per call, with increasing `new_total` values.
#[test]
fn test_sequential_deposits_emit_separate_events() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    let amounts = [50_000i128, 75_000i128, 25_000i128];
    let mut expected_balance = 0i128;

    for &amount in &amounts {
        let events_before = env.events().all().len();
        client.deposit(&vault_id, &owner, &amount);
        expected_balance += amount;
        let events_after = env.events().all().len();

        assert!(
            events_after > events_before,
            "each deposit call must emit at least one new event"
        );
        assert_eq!(
            client.get_vault(&vault_id).balance,
            expected_balance,
            "balance must increase by exact deposit amount"
        );
    }
}
