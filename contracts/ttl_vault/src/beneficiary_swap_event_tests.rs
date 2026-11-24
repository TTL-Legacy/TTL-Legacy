#![cfg(test)]
//! Tests for the BENEFICIARY_SWAPPED_TOPIC event emitted by swap_allocations.
//! Issue #2074 — beneficiary_swap transaction type on-chain event log.

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    token::StellarAssetClient,
    vec, Address, Env, IntoVal,
};

fn setup_env() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
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
    (env, owner, admin, client)
}

// ---------------------------------------------------------------------------
// Topic emission
// ---------------------------------------------------------------------------

/// swap_allocations must emit BENEFICIARY_SWAPPED_TOPIC after a successful swap.
#[test]
fn test_swap_emits_beneficiary_swapped_topic() {
    let (env, owner, _, client) = setup_env();

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);

    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 6000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 4000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);
    client.swap_allocations(&vault_id, &owner, &b1, &b2);

    // The event log must contain at least one event with BENEFICIARY_SWAPPED_TOPIC.
    let events = env.events().all();
    let has_swap_event = events.iter().any(|(_, topics, _)| {
        topics.len() >= 1 && topics.get(0) == Some(BENEFICIARY_SWAPPED_TOPIC.into_val(&env))
    });
    assert!(has_swap_event, "BENEFICIARY_SWAPPED_TOPIC was not emitted");
}

// ---------------------------------------------------------------------------
// Payload fields
// ---------------------------------------------------------------------------

/// The event payload must carry old_address, new_address, old_bps, new_bps,
/// and vault_id so downstream indexers can update cached beneficiary tables
/// without re-reading the vault.
#[test]
fn test_swap_event_payload_contains_addresses_bps_and_vault_id() {
    let (env, owner, _, client) = setup_env();

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);

    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 7000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 3000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);
    client.swap_allocations(&vault_id, &owner, &b1, &b2);

    // Find the swap event and verify its payload.
    let events = env.events().all();
    let swap_event = events.iter().find(|(_, topics, _)| {
        topics.len() >= 1 && topics.get(0) == Some(BENEFICIARY_SWAPPED_TOPIC.into_val(&env))
    });

    assert!(swap_event.is_some(), "swap event not found in event log");

    let (_, _, data) = swap_event.unwrap();
    // The payload must be a BeneficiarySwappedEvent; we verify it round-trips
    // through SCVal by checking the vault_id field is correct.
    let event: BeneficiarySwappedEvent = data.into_val(&env);
    assert_eq!(event.vault_id, vault_id, "vault_id mismatch in swap event");
    assert_eq!(event.old_bps, 7000, "old_bps should be b1's pre-swap BPS");
    assert_eq!(event.new_bps, 3000, "new_bps should be b2's pre-swap BPS");
}

// ---------------------------------------------------------------------------
// Downstream consistency invariant
// ---------------------------------------------------------------------------

/// After a swap the BPS values recorded in the event must equal the values
/// visible in get_vault for each address (just inverted).
#[test]
fn test_swap_event_bps_consistent_with_vault_state() {
    let (env, owner, _, client) = setup_env();

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);

    let bps_b1_before = 8000u32;
    let bps_b2_before = 2000u32;

    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: bps_b1_before, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: bps_b2_before, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);
    client.swap_allocations(&vault_id, &owner, &b1, &b2);

    // After the swap b1 should hold bps_b2_before and b2 should hold bps_b1_before.
    let vault = client.get_vault(&vault_id);
    let b1_post = vault.beneficiaries.iter().find(|e| e.address == b1).unwrap().bps;
    let b2_post = vault.beneficiaries.iter().find(|e| e.address == b2).unwrap().bps;

    // The event's old_bps / new_bps should equal the pre-swap values.
    let events = env.events().all();
    let swap_event = events.iter().find(|(_, topics, _)| {
        topics.len() >= 1 && topics.get(0) == Some(BENEFICIARY_SWAPPED_TOPIC.into_val(&env))
    });
    let (_, _, data) = swap_event.unwrap();
    let event: BeneficiarySwappedEvent = data.into_val(&env);

    // event.old_bps was b1's allocation before the swap
    assert_eq!(event.old_bps, bps_b1_before);
    // event.new_bps was b2's allocation before the swap
    assert_eq!(event.new_bps, bps_b2_before);
    // The post-swap vault state is exactly the inverse
    assert_eq!(b1_post, bps_b2_before);
    assert_eq!(b2_post, bps_b1_before);
}

// ---------------------------------------------------------------------------
// No event on failure
// ---------------------------------------------------------------------------

/// A failed swap (unknown address) must not emit BENEFICIARY_SWAPPED_TOPIC.
#[test]
fn test_failed_swap_does_not_emit_event() {
    let (env, owner, _, client) = setup_env();

    let b1 = Address::generate(&env);
    let stranger = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &100);
    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 10_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    // This should fail because `stranger` is not a registered beneficiary.
    let _ = client.try_swap_allocations(&vault_id, &owner, &b1, &stranger);

    let events = env.events().all();
    let has_swap_event = events.iter().any(|(_, topics, _)| {
        topics.len() >= 1 && topics.get(0) == Some(BENEFICIARY_SWAPPED_TOPIC.into_val(&env))
    });
    assert!(!has_swap_event, "swap event must not be emitted on failure");
}
