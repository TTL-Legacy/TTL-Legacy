#![cfg(test)]

//! Tests for Issue #558 — Passkey Audit Trail.

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Bytes, BytesN, Env,
};

use crate::tests::common::setup_funded as setup;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn setup_vault_with_passkey(
    env: &Env,
    owner: &Address,
    beneficiary: &Address,
    client: &TtlVaultContractClient,
) -> (u64, BytesN<32>) {
    let vault_id = client.create_vault(owner, beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let passkey_hash = BytesN::<32>::from_array(env, &[0xABu8; 32]);
    client.add_passkey(&vault_id, owner, &passkey_hash, &false);
    (vault_id, passkey_hash)
}

/// Writes the passkey directly into vault.passkey_hash so rotate_passkey can find it.
/// (rotate_passkey checks the legacy vault.passkey_hash field, not VaultPasskeys.)
fn seed_primary_passkey(
    env: &Env,
    client: &TtlVaultContractClient<'static>,
    vault_id: u64,
    passkey: &BytesN<32>,
) {
    env.as_contract(&client.address, || {
        let key = StorageKey::Vault(vault_id);
        let mut v: Vault = env.storage().persistent().get(&key).unwrap();
        v.passkey_hash = Some(Bytes::from_array(env, &passkey.to_array()));
        env.storage().persistent().set(&key, &v);
    });
}

// ── AC1: add_passkey appends "add" audit entry ────────────────────────────────

#[test]
fn test_add_passkey_appends_audit_entry() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let passkey_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);

    client.add_passkey(&vault_id, &owner, &passkey_hash, &false);

    let log = client.get_passkey_audit_log(&vault_id);
    assert_eq!(log.len(), 1);
    let entry = log.get(0).unwrap();
    assert_eq!(entry.operation, soroban_sdk::String::from_str(&env, "add"));
    assert_eq!(entry.actor, owner);
    assert_eq!(entry.passkey_hash, passkey_hash);
}

// ── AC2: remove_passkey appends "remove" audit entry ─────────────────────────

#[test]
fn test_remove_passkey_appends_audit_entry() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    // Add a second passkey so the vault is not empty after removal.
    let passkey2 = BytesN::<32>::from_array(&env, &[0xCDu8; 32]);
    client.add_passkey(&vault_id, &owner, &passkey2, &false);

    client.remove_passkey(&vault_id, &owner, &passkey_hash);

    let log = client.get_passkey_audit_log(&vault_id);
    // log[0] = add first, log[1] = add second, log[2] = remove first
    let last = log.get(log.len() - 1).unwrap();
    assert_eq!(
        last.operation,
        soroban_sdk::String::from_str(&env, "remove")
    );
    assert_eq!(last.passkey_hash, passkey_hash);
}

// ── AC3: rotate_passkey appends "rotate_remove" + "rotate_add" ────────────────

#[test]
fn test_rotate_passkey_appends_two_audit_entries() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let old_hash = BytesN::<32>::from_array(&env, &[0x11u8; 32]);
    let new_hash = BytesN::<32>::from_array(&env, &[0x22u8; 32]);

    // Seed the primary passkey field so rotate_passkey can find it.
    seed_primary_passkey(&env, &client, vault_id, &old_hash);
    let count_before = client.get_passkey_audit_log(&vault_id).len();

    client.rotate_passkey(&vault_id, &owner, &old_hash, &new_hash);

    let log = client.get_passkey_audit_log(&vault_id);
    assert_eq!(
        log.len(),
        count_before + 2,
        "rotation should append exactly 2 audit entries"
    );
    let second_last = log.get(log.len() - 2).unwrap();
    let last = log.get(log.len() - 1).unwrap();
    assert_eq!(
        second_last.operation,
        soroban_sdk::String::from_str(&env, "rotate_remove")
    );
    assert_eq!(second_last.passkey_hash, old_hash);
    assert_eq!(
        last.operation,
        soroban_sdk::String::from_str(&env, "rotate_add")
    );
    assert_eq!(last.passkey_hash, new_hash);
}

// ── AC4: check_in appends "use" audit entry ───────────────────────────────────

#[test]
fn test_check_in_appends_use_audit_entry() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);

    let now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &passkey_hash, &0u64, &None, &None);

    let log = client.get_passkey_audit_log(&vault_id);
    let last = log.get(log.len() - 1).unwrap();
    assert_eq!(
        last.operation,
        soroban_sdk::String::from_str(&env, "use")
    );
    assert_eq!(last.actor, owner);
    assert_eq!(last.passkey_hash, passkey_hash);
}

// ── AC5: get_passkey_audit_log returns all entries ────────────────────────────

#[test]
fn test_audit_log_accumulates_all_operations() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let pk = BytesN::<32>::from_array(&env, &[0x55u8; 32]);

    client.add_passkey(&vault_id, &owner, &pk, &false);

    let now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &pk, &0u64, &None, &None);

    let log = client.get_passkey_audit_log(&vault_id);
    // 1 add + 1 use = 2 entries
    assert_eq!(log.len(), 2);
}

// ── AC6: VaultNotFound when vault does not exist ─────────────────────────────

#[test]
#[should_panic]
fn test_audit_log_vault_not_found() {
    let (_env, _owner, _beneficiary, client) = setup();
    client.get_passkey_audit_log(&9999u64);
}

// ── AC10: entry count matches total operations ────────────────────────────────

#[test]
fn test_audit_log_count_matches_total_operations() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let pk1 = BytesN::<32>::from_array(&env, &[0x01u8; 32]);
    let pk2 = BytesN::<32>::from_array(&env, &[0x02u8; 32]);

    client.add_passkey(&vault_id, &owner, &pk1, &false); // +1
    client.add_passkey(&vault_id, &owner, &pk2, &false); // +1

    env.ledger()
        .set_timestamp(env.ledger().timestamp() + MIN_CHECK_IN_INTERVAL / 2);
    client.check_in(&vault_id, &owner, &pk1, &0u64, &None, &None); // +1

    let log = client.get_passkey_audit_log(&vault_id);
    assert_eq!(log.len(), 3, "3 operations should produce 3 audit entries");
}
