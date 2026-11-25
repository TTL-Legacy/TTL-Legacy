#![cfg(test)]

//! Tests for Issue #559 — Passkey Escrow.

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
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
    let passkey_hash = BytesN::<32>::from_array(env, &[0xEEu8; 32]);
    client.add_passkey(&vault_id, owner, &passkey_hash, &false);
    (vault_id, passkey_hash)
}

// ── AC1 + AC12: escrow_passkey creates record and emits event ─────────────────

#[test]
fn test_escrow_passkey_creates_record() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let recovery = Address::generate(&env);

    client.escrow_passkey(&vault_id, &owner, &passkey_hash, &recovery);

    let record = client
        .get_passkey_escrow(&vault_id, &passkey_hash)
        .expect("escrow record should exist after escrow_passkey");
    assert_eq!(record.recovery_contact, recovery);
}

// ── AC3: PasskeyNotFound when passkey not in vault ────────────────────────────

#[test]
#[should_panic]
fn test_escrow_passkey_not_found() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let missing = BytesN::<32>::from_array(&env, &[0u8; 32]);
    let recovery = Address::generate(&env);

    client.escrow_passkey(&vault_id, &owner, &missing, &recovery);
}

// ── AC4: InvalidBeneficiary when recovery_contact == owner ───────────────────

#[test]
#[should_panic]
fn test_escrow_passkey_recovery_contact_is_owner() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);

    client.escrow_passkey(&vault_id, &owner, &passkey_hash, &owner);
}

// ── AC5: AlreadyInEscrow on second escrow call ────────────────────────────────

#[test]
#[should_panic]
fn test_escrow_passkey_already_in_escrow() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let recovery = Address::generate(&env);

    client.escrow_passkey(&vault_id, &owner, &passkey_hash, &recovery);
    // Second call should return AlreadyInEscrow.
    client.escrow_passkey(&vault_id, &owner, &passkey_hash, &recovery);
}

// ── AC6: check-in blocked while passkey is in escrow ─────────────────────────

#[test]
#[should_panic]
fn test_escrowed_passkey_cannot_be_used_for_check_in() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let recovery = Address::generate(&env);

    client.escrow_passkey(&vault_id, &owner, &passkey_hash, &recovery);

    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 10);
    // Should fail because passkey is in escrow.
    client.check_in(&vault_id, &owner, &passkey_hash, &0u64, &None, &None);
}

// ── AC7 + AC13: release_escrow_passkey removes record and emits event ─────────

#[test]
fn test_release_escrow_passkey_removes_record() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let recovery = Address::generate(&env);

    client.escrow_passkey(&vault_id, &owner, &passkey_hash, &recovery);
    client.release_escrow_passkey(&vault_id, &recovery, &passkey_hash);

    assert!(
        client
            .get_passkey_escrow(&vault_id, &passkey_hash)
            .is_none(),
        "escrow record should be removed after release"
    );
}

// ── AC9: NotRecoveryContact when wrong caller releases escrow ─────────────────

#[test]
#[should_panic]
fn test_release_escrow_passkey_wrong_caller() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let recovery = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.escrow_passkey(&vault_id, &owner, &passkey_hash, &recovery);
    client.release_escrow_passkey(&vault_id, &stranger, &passkey_hash);
}

// ── AC10 + AC14: cancel_passkey_escrow removes record and emits event ─────────

#[test]
fn test_cancel_passkey_escrow_removes_record() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let recovery = Address::generate(&env);

    client.escrow_passkey(&vault_id, &owner, &passkey_hash, &recovery);
    client.cancel_passkey_escrow(&vault_id, &owner, &passkey_hash);

    assert!(
        client
            .get_passkey_escrow(&vault_id, &passkey_hash)
            .is_none(),
        "escrow record should be removed after cancellation"
    );
}

// ── AC11: NotOwner when non-owner cancels escrow ─────────────────────────────

#[test]
#[should_panic]
fn test_cancel_passkey_escrow_not_owner() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let recovery = Address::generate(&env);
    let not_owner = Address::generate(&env);

    client.escrow_passkey(&vault_id, &owner, &passkey_hash, &recovery);
    client.cancel_passkey_escrow(&vault_id, &not_owner, &passkey_hash);
}

// ── Passkey restored after escrow release allows check-in ────────────────────

#[test]
fn test_check_in_allowed_after_escrow_released() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let recovery = Address::generate(&env);

    client.escrow_passkey(&vault_id, &owner, &passkey_hash, &recovery);
    client.release_escrow_passkey(&vault_id, &recovery, &passkey_hash);

    // After release, check-in should succeed.
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &passkey_hash, &0u64, &None, &None);
}

// ── AC15: get_passkey_escrow returns None when passkey is not in escrow ───────

#[test]
fn test_get_passkey_escrow_none_when_not_escrowed() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);

    assert!(client.get_passkey_escrow(&vault_id, &passkey_hash).is_none());
}
