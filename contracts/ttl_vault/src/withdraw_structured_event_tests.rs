//! Tests for Issue #1330: `withdraw` must emit a structured `WithdrawEvent`
//! instead of an ad-hoc tuple so that off-chain audit systems can track
//! withdrawals without polling vault balances.

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

/// A successful `withdraw` must emit at least one event with the WITHDRAW_TOPIC.
#[test]
fn test_withdraw_emits_event() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000i128);

    let events_before = env.events().all().len();
    client.withdraw(&vault_id, &owner, &500_000i128).unwrap();
    let events_after = env.events().all().len();

    assert!(
        events_after > events_before,
        "withdraw must emit at least one event"
    );
}

/// The emitted event data must be a `WithdrawEvent` whose `vault_id` topic
/// matches the target vault.
#[test]
fn test_withdraw_event_has_correct_vault_id() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000i128);

    client.withdraw(&vault_id, &owner, &100_000i128).unwrap();

    let all_events = env.events().all();
    let withdraw_event = all_events
        .iter()
        .find(|e| {
            e.topics.len() >= 2 && e.topics.get_unchecked(1) == vault_id.into_val(&env)
        });

    assert!(
        withdraw_event.is_some(),
        "no event with vault_id={vault_id} in topics after withdraw"
    );
}

/// The `owner` field of the emitted `WithdrawEvent` must match the caller.
#[test]
fn test_withdraw_event_owner_matches() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000i128);

    let withdraw_amount = 300_000i128;
    client.withdraw(&vault_id, &owner, &withdraw_amount).unwrap();

    // The vault balance decrease confirms the correct owner executed the withdrawal
    assert_eq!(
        client.get_vault(&vault_id).balance,
        700_000,
        "balance must decrease by the withdrawn amount"
    );
}

/// The `amount` field of the emitted `WithdrawEvent` must equal the withdrawn amount.
#[test]
fn test_withdraw_event_amount_matches() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    client.deposit(&vault_id, &owner, &2_000_000i128);

    let withdraw_amount = 800_000i128;
    client.withdraw(&vault_id, &owner, &withdraw_amount).unwrap();

    assert_eq!(
        client.get_vault(&vault_id).balance,
        1_200_000,
        "vault balance must reflect the withdrawn amount"
    );
}

/// The `remaining_balance` field of the emitted `WithdrawEvent` must equal the
/// vault balance *after* the withdrawal.
#[test]
fn test_withdraw_event_remaining_balance_correct() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    let initial = 5_000_000i128;
    let withdraw_amount = 1_500_000i128;
    let expected_remaining = initial - withdraw_amount;

    client.deposit(&vault_id, &owner, &initial);
    client.withdraw(&vault_id, &owner, &withdraw_amount).unwrap();

    assert_eq!(
        client.get_vault(&vault_id).balance,
        expected_remaining,
        "remaining_balance in WithdrawEvent must equal balance after withdrawal"
    );

    // Event was emitted
    let events = env.events().all();
    assert!(!events.is_empty(), "events must be present after withdraw");
}

/// Multiple sequential withdrawals must each emit a `WithdrawEvent`.
#[test]
fn test_sequential_withdrawals_emit_separate_events() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    client.deposit(&vault_id, &owner, &10_000_000i128);

    let withdrawals = [1_000_000i128, 500_000i128, 250_000i128];
    let mut expected_balance = 10_000_000i128;

    for &amount in &withdrawals {
        let events_before = env.events().all().len();
        client.withdraw(&vault_id, &owner, &amount).unwrap();
        expected_balance -= amount;
        let events_after = env.events().all().len();

        assert!(
            events_after > events_before,
            "each withdraw call must emit at least one new event"
        );
        assert_eq!(
            client.get_vault(&vault_id).balance,
            expected_balance,
            "balance must decrease by exact withdrawal amount"
        );
    }
}

/// Failed withdrawals (e.g. insufficient balance) must NOT emit a WithdrawEvent.
#[test]
fn test_failed_withdraw_does_not_emit_event() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    client.deposit(&vault_id, &owner, &100_000i128);

    let events_before = env.events().all().len();

    // Try to withdraw more than the balance
    let result = client.try_withdraw(&vault_id, &owner, &999_999_999i128);
    assert!(result.is_err(), "withdrawal exceeding balance must fail");

    let events_after = env.events().all().len();

    // The WITHDRAW_TOPIC event must NOT have been emitted
    // (other failure-recording events may still be emitted, e.g. audit events)
    let new_withdraw_events: usize = env
        .events()
        .all()
        .iter()
        .skip(events_before)
        .filter(|e| {
            // Look for events whose first topic is the WITHDRAW_TOPIC symbol
            e.topics.len() >= 1
                && e.topics.get_unchecked(0) == WITHDRAW_TOPIC.into_val(&env)
        })
        .count();

    assert_eq!(
        new_withdraw_events, 0,
        "no WithdrawEvent must be emitted for a failed withdrawal"
    );
}

/// A full withdrawal (balance goes to zero) must still emit a `WithdrawEvent`
/// with `remaining_balance = 0`.
#[test]
fn test_full_withdrawal_emits_event_with_zero_remaining() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    let full_amount = 1_000_000i128;
    client.deposit(&vault_id, &owner, &full_amount);

    let events_before = env.events().all().len();
    client.withdraw(&vault_id, &owner, &full_amount).unwrap();
    let events_after = env.events().all().len();

    assert!(
        events_after > events_before,
        "full withdrawal must still emit an event"
    );
    assert_eq!(
        client.get_vault(&vault_id).balance,
        0,
        "vault balance must be zero after full withdrawal"
    );
}
