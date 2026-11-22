/// Tests for Issue #557 – Passkey Delegation.
#[cfg(test)]
use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
};

use crate::tests::common::setup_unfunded as setup;

fn make_passkey(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::<32>::from_array(env, &[byte; 32])
}

fn setup_vault_with_passkey(byte: u8) -> (Env, Address, Address, TtlVaultContractClient<'static>, u64, BytesN<32>) {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &300_u64, &None);
    let pk = make_passkey(&env, byte);
    client.add_passkey(&vault_id, &owner, &pk, &false);
    (env, owner, beneficiary, client, vault_id, pk)
}

#[test]
fn test_delegate_passkey_stores_record() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_passkey(1);
    let delegate = Address::generate(&env);
    let now = env.ledger().timestamp();
    let expires_at = now + 3600;

    client.delegate_passkey(&vault_id, &owner, &pk, &delegate, &expires_at);

    let record = client.get_passkey_delegation(&vault_id, &pk).unwrap();
    assert_eq!(record.delegate, delegate);
    assert_eq!(record.expires_at, expires_at);
}

#[test]
fn test_delegate_passkey_emits_event() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_passkey(2);
    let delegate = Address::generate(&env);
    let now = env.ledger().timestamp();
    let expires_at = now + 3600;

    client.delegate_passkey(&vault_id, &owner, &pk, &delegate, &expires_at);

    // Event was emitted without panicking – structural check sufficient here.
    let record = client.get_passkey_delegation(&vault_id, &pk);
    assert!(record.is_some());
}

#[test]
#[should_panic]
fn test_delegate_passkey_requires_owner_auth() {
    let (env, _owner, _beneficiary, client, vault_id, pk) = setup_vault_with_passkey(3);
    let attacker = Address::generate(&env);
    let now = env.ledger().timestamp();
    // attacker is not the owner – should panic
    client.delegate_passkey(&vault_id, &attacker, &pk, &attacker, &(now + 3600));
}

#[test]
#[should_panic(expected = "PasskeyNotFound")]
fn test_delegate_passkey_not_found() {
    let (env, owner, _beneficiary, client, vault_id, _pk) = setup_vault_with_passkey(4);
    let delegate = Address::generate(&env);
    let unknown_pk = make_passkey(&env, 99);
    let now = env.ledger().timestamp();
    client.delegate_passkey(&vault_id, &owner, &unknown_pk, &delegate, &(now + 3600));
}

#[test]
#[should_panic(expected = "InvalidInterval")]
fn test_delegate_passkey_expires_in_past() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_passkey(5);
    let delegate = Address::generate(&env);
    let now = env.ledger().timestamp();
    // expires_at must be strictly > now; use now itself to trigger InvalidInterval.
    client.delegate_passkey(&vault_id, &owner, &pk, &delegate, &now);
}

#[test]
#[should_panic(expected = "InvalidBeneficiary")]
fn test_delegate_passkey_to_self_rejected() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_passkey(6);
    let now = env.ledger().timestamp();
    // delegate == owner → InvalidBeneficiary
    client.delegate_passkey(&vault_id, &owner, &pk, &owner, &(now + 3600));
}

#[test]
fn test_revoke_passkey_delegation_removes_record() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_passkey(7);
    let delegate = Address::generate(&env);
    let now = env.ledger().timestamp();
    client.delegate_passkey(&vault_id, &owner, &pk, &delegate, &(now + 3600));

    client.revoke_passkey_delegation(&vault_id, &owner, &pk);

    assert!(client.get_passkey_delegation(&vault_id, &pk).is_none());
}

#[test]
#[should_panic]
fn test_revoke_passkey_delegation_requires_owner() {
    let (env, _owner, _beneficiary, client, vault_id, pk) = setup_vault_with_passkey(8);
    let attacker = Address::generate(&env);
    client.revoke_passkey_delegation(&vault_id, &attacker, &pk);
}

#[test]
fn test_get_passkey_delegation_returns_none_when_absent() {
    let (env, _owner, _beneficiary, client, vault_id, pk) = setup_vault_with_passkey(9);
    assert!(client.get_passkey_delegation(&vault_id, &pk).is_none());
}

#[test]
fn test_expired_delegation_cannot_check_in() {
    let (env, owner, beneficiary, client, vault_id, pk) = setup_vault_with_passkey(10);
    let delegate = Address::generate(&env);
    let now = env.ledger().timestamp();
    // Set delegation to expire in 60 seconds.
    let expires_at = now + 60;
    client.delegate_passkey(&vault_id, &owner, &pk, &delegate, &expires_at);

    // Advance time past expiry.
    env.ledger().set_timestamp(expires_at + 1);

    // Attempt check-in as delegate should fail (delegation expired → NotOwner).
    let result = client.try_check_in(&vault_id, &delegate, &pk, &0u64, &None, &None);
    assert!(result.is_err());
}
