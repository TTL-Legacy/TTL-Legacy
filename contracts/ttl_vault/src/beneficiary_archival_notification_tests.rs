//! Tests for Issue #1337: beneficiary archival notification on TTL expiry.
//!
//! These tests cover:
//!   1. A beneficiary can register encrypted contact information.
//!   2. A non-beneficiary cannot set contact information.
//!   3. Contact information is correctly retrieved.
//!   4. Opt-in / opt-out flag is correctly toggled.
//!   5. A non-beneficiary cannot change the opt-in flag.
//!   6. Multi-beneficiary vault: any listed beneficiary can register contact.
//!   7. Overwriting contact info updates the record.
//!   8. Missing contact returns None from get_beneficiary_contact.

#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Bytes, Env,
};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn setup() -> (
    Env,
    Address, // owner
    Address, // beneficiary
    Address, // admin
    TtlVaultContractClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_id = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_id);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, beneficiary, admin, client)
}

fn encrypted_contact(env: &Env) -> Bytes {
    // Simulate a client-side encrypted payload (opaque bytes in tests)
    Bytes::from_slice(env, b"ENCRYPTED:email:beneficiary@example.com|sms:+15551234567")
}

// ─── set_beneficiary_contact ──────────────────────────────────────────────────

/// Beneficiary can register contact info; it is retrievable and opted_in = true.
#[test]
fn test_set_and_get_beneficiary_contact() {
    let (env, owner, beneficiary, _admin, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    env.ledger().with_mut(|l| l.timestamp = 100);
    let contact_blob = encrypted_contact(&env);

    client.set_beneficiary_contact(&vault_id, &beneficiary, &contact_blob);

    let stored = client
        .get_beneficiary_contact(&vault_id, &beneficiary)
        .expect("contact should be stored");

    assert_eq!(stored.encrypted_contact, contact_blob);
    assert!(stored.opted_in, "should default to opted_in = true");
    assert_eq!(stored.updated_at, 100);
}

/// Non-beneficiary cannot set contact info.
#[test]
#[should_panic]
fn test_non_beneficiary_cannot_set_contact() {
    let (env, owner, beneficiary, _admin, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let attacker = Address::generate(&env);
    let contact_blob = encrypted_contact(&env);
    client.set_beneficiary_contact(&vault_id, &attacker, &contact_blob);
}

/// Owner cannot set beneficiary contact (only the beneficiary can).
#[test]
#[should_panic]
fn test_owner_cannot_set_beneficiary_contact() {
    let (env, owner, beneficiary, _admin, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let contact_blob = encrypted_contact(&env);
    client.set_beneficiary_contact(&vault_id, &owner, &contact_blob);
}

// ─── get_beneficiary_contact ──────────────────────────────────────────────────

/// Returns None when no contact has been registered.
#[test]
fn test_get_contact_returns_none_when_unset() {
    let (_env, owner, beneficiary, _admin, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let result = client.get_beneficiary_contact(&vault_id, &beneficiary);
    assert!(result.is_none(), "should be None before any contact is set");
}

// ─── opt-in / opt-out ─────────────────────────────────────────────────────────

/// Beneficiary can opt out and then opt back in.
#[test]
fn test_opt_out_and_opt_in() {
    let (env, owner, beneficiary, _admin, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Register contact first
    let contact_blob = encrypted_contact(&env);
    client.set_beneficiary_contact(&vault_id, &beneficiary, &contact_blob);

    // Opt out
    client.set_beneficiary_notification_opt_in(&vault_id, &beneficiary, &false);
    let stored = client
        .get_beneficiary_contact(&vault_id, &beneficiary)
        .unwrap();
    assert!(!stored.opted_in, "should be opted out");

    // Opt back in
    client.set_beneficiary_notification_opt_in(&vault_id, &beneficiary, &true);
    let stored = client
        .get_beneficiary_contact(&vault_id, &beneficiary)
        .unwrap();
    assert!(stored.opted_in, "should be opted in again");
}

/// Non-beneficiary cannot change the opt-in flag.
#[test]
#[should_panic]
fn test_non_beneficiary_cannot_opt_in() {
    let (env, owner, beneficiary, _admin, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let attacker = Address::generate(&env);
    client.set_beneficiary_notification_opt_in(&vault_id, &attacker, &false);
}

/// `set_beneficiary_notification_opt_in` creates a minimal record if no
/// contact has been set yet, rather than panicking.
#[test]
fn test_opt_out_without_prior_contact_creates_record() {
    let (_env, owner, beneficiary, _admin, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // No contact set yet — opt out should create a minimal record
    client.set_beneficiary_notification_opt_in(&vault_id, &beneficiary, &false);

    let stored = client
        .get_beneficiary_contact(&vault_id, &beneficiary)
        .expect("record should be created");
    assert!(!stored.opted_in);
}

// ─── multi-beneficiary vault ──────────────────────────────────────────────────

/// In a multi-beneficiary vault, each listed beneficiary can register contact.
#[test]
fn test_multi_beneficiary_can_each_set_contact() {
    let (env, owner, b1, _admin, client) = setup();
    let b2 = Address::generate(&env);
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &b1, &interval, &None);

    // Set multi-beneficiary split
    client.set_beneficiaries(
        &vault_id,
        &owner,
        &vec![
            &env,
            BeneficiaryEntry {
                address: b1.clone(),
                bps: 5_000,
                minimum_threshold: 0,
            },
            BeneficiaryEntry {
                address: b2.clone(),
                bps: 5_000,
                minimum_threshold: 0,
            },
        ],
    );

    let blob1 = Bytes::from_slice(&env, b"ENCRYPTED:b1@example.com");
    let blob2 = Bytes::from_slice(&env, b"ENCRYPTED:b2@example.com");

    client.set_beneficiary_contact(&vault_id, &b1, &blob1);
    client.set_beneficiary_contact(&vault_id, &b2, &blob2);

    let stored1 = client.get_beneficiary_contact(&vault_id, &b1).unwrap();
    let stored2 = client.get_beneficiary_contact(&vault_id, &b2).unwrap();

    assert_eq!(stored1.encrypted_contact, blob1);
    assert_eq!(stored2.encrypted_contact, blob2);
}

/// A third-party address that is NOT in the beneficiaries list cannot set contact.
#[test]
#[should_panic]
fn test_non_listed_address_cannot_set_contact_in_multi() {
    let (env, owner, b1, _admin, client) = setup();
    let b2 = Address::generate(&env);
    let attacker = Address::generate(&env);
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &b1, &interval, &None);

    client.set_beneficiaries(
        &vault_id,
        &owner,
        &vec![
            &env,
            BeneficiaryEntry {
                address: b1.clone(),
                bps: 5_000,
                minimum_threshold: 0,
            },
            BeneficiaryEntry {
                address: b2.clone(),
                bps: 5_000,
                minimum_threshold: 0,
            },
        ],
    );

    let blob = Bytes::from_slice(&env, b"ENCRYPTED:attacker@example.com");
    client.set_beneficiary_contact(&vault_id, &attacker, &blob);
}

// ─── overwrite ────────────────────────────────────────────────────────────────

/// Calling set_beneficiary_contact again overwrites the previous entry.
#[test]
fn test_overwrite_contact_info() {
    let (env, owner, beneficiary, _admin, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let old_blob = Bytes::from_slice(&env, b"ENCRYPTED:old@example.com");
    let new_blob = Bytes::from_slice(&env, b"ENCRYPTED:new@example.com");

    env.ledger().with_mut(|l| l.timestamp = 100);
    client.set_beneficiary_contact(&vault_id, &beneficiary, &old_blob);

    env.ledger().with_mut(|l| l.timestamp = 200);
    client.set_beneficiary_contact(&vault_id, &beneficiary, &new_blob);

    let stored = client.get_beneficiary_contact(&vault_id, &beneficiary).unwrap();
    assert_eq!(stored.encrypted_contact, new_blob, "contact should be updated");
    assert_eq!(stored.updated_at, 200, "timestamp should be updated");
}
