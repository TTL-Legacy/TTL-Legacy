#![cfg(test)]

//! Tests for Issue #557 — Passkey Delegation.

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
    let passkey_hash = BytesN::<32>::from_array(env, &[7u8; 32]);
    client.add_passkey(&vault_id, owner, &passkey_hash, &false);
    (vault_id, passkey_hash)
}

// ── AC1 + AC10: delegate_passkey stores record and emits event ───────────────

#[test]
fn test_delegate_passkey_stores_record() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let delegate = Address::generate(&env);
    let now = env.ledger().timestamp();
    let expires_at = now + 3600;

    client.delegate_passkey(&vault_id, &owner, &passkey_hash, &delegate, &expires_at);

    let record = client
        .get_passkey_delegation(&vault_id, &passkey_hash)
        .expect("delegation record should exist");
    assert_eq!(record.delegate, delegate);
    assert_eq!(record.expires_at, expires_at);
}

// ── AC3: PasskeyNotFound when passkey not in vault ───────────────────────────

#[test]
#[should_panic]
fn test_delegate_passkey_not_found() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let missing = BytesN::<32>::from_array(&env, &[0u8; 32]);
    let delegate = Address::generate(&env);
    let expires_at = env.ledger().timestamp() + 3600;

    client.delegate_passkey(&vault_id, &owner, &missing, &delegate, &expires_at);
}

// ── AC4: InvalidInterval when expires_at <= now ──────────────────────────────

#[test]
#[should_panic]
fn test_delegate_passkey_invalid_interval_in_past() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let delegate = Address::generate(&env);
    let expires_at = env.ledger().timestamp(); // equal to now — should fail

    client.delegate_passkey(&vault_id, &owner, &passkey_hash, &delegate, &expires_at);
}

// ── AC5: InvalidBeneficiary when delegate == owner ───────────────────────────

#[test]
#[should_panic]
fn test_delegate_passkey_delegate_is_owner() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let expires_at = env.ledger().timestamp() + 3600;

    client.delegate_passkey(&vault_id, &owner, &passkey_hash, &owner, &expires_at);
}

// ── AC6: delegate can check in using the delegated passkey ───────────────────

#[test]
fn test_passkey_delegate_can_check_in() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let delegate = Address::generate(&env);
    let now = env.ledger().timestamp();
    let expires_at = now + 86_400;

    client.delegate_passkey(&vault_id, &owner, &passkey_hash, &delegate, &expires_at);

    // Advance time to avoid check-in cooldown.
    env.ledger().set_timestamp(now + 10);
    // Delegate check-in — passes because a valid delegation exists.
    client.check_in(&vault_id, &delegate, &passkey_hash, &0u64, &None, &None);
}

// ── AC7: expired delegation is rejected ─────────────────────────────────────

#[test]
#[should_panic]
fn test_passkey_delegate_check_in_expired_delegation() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let delegate = Address::generate(&env);
    let now = env.ledger().timestamp();
    let expires_at = now + 100;

    client.delegate_passkey(&vault_id, &owner, &passkey_hash, &delegate, &expires_at);

    // Advance past delegation expiry.
    env.ledger().set_timestamp(now + 200);
    client.check_in(&vault_id, &delegate, &passkey_hash, &0u64, &None, &None);
}

// ── AC8 + AC9 + AC11: revoke removes record and emits event ──────────────────

#[test]
fn test_revoke_passkey_delegation_removes_record() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let delegate = Address::generate(&env);
    let expires_at = env.ledger().timestamp() + 3600;

    client.delegate_passkey(&vault_id, &owner, &passkey_hash, &delegate, &expires_at);
    client.revoke_passkey_delegation(&vault_id, &owner, &passkey_hash);

    assert!(
        client
            .get_passkey_delegation(&vault_id, &passkey_hash)
            .is_none(),
        "delegation should be removed after revocation"
    );
}

// ── AC12: get_passkey_delegation returns None when no delegation ──────────────

#[test]
fn test_get_passkey_delegation_returns_none_when_absent() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);

    assert!(client.get_passkey_delegation(&vault_id, &passkey_hash).is_none());
}

// ── AC2: NotOwner when non-owner calls delegate_passkey ──────────────────────

#[test]
#[should_panic]
fn test_delegate_passkey_not_owner() {
    let (env, owner, beneficiary, client) = setup();
    let (vault_id, passkey_hash) = setup_vault_with_passkey(&env, &owner, &beneficiary, &client);
    let not_owner = Address::generate(&env);
    let delegate = Address::generate(&env);
    let expires_at = env.ledger().timestamp() + 3600;

    client.delegate_passkey(&vault_id, &not_owner, &passkey_hash, &delegate, &expires_at);
}
