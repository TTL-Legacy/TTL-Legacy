/// Tests for Issue #559 – Passkey Escrow.
#[cfg(test)]
use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
};

use crate::tests::common::setup_unfunded as setup;

fn make_pk(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::<32>::from_array(env, &[byte; 32])
}

fn setup_vault_with_pk(byte: u8) -> (Env, Address, Address, TtlVaultContractClient<'static>, u64, BytesN<32>) {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &300_u64, &None);
    let pk = make_pk(&env, byte);
    client.add_passkey(&vault_id, &owner, &pk, &false);
    (env, owner, beneficiary, client, vault_id, pk)
}

#[test]
fn test_escrow_passkey_creates_record() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_pk(1);
    let recovery = Address::generate(&env);

    client.escrow_passkey(&vault_id, &owner, &pk, &recovery);

    let record = client.get_passkey_escrow(&vault_id, &pk).unwrap();
    assert_eq!(record.recovery_contact, recovery);
}

#[test]
#[should_panic]
fn test_escrow_passkey_requires_owner() {
    let (env, _owner, _beneficiary, client, vault_id, pk) = setup_vault_with_pk(2);
    let attacker = Address::generate(&env);
    let recovery = Address::generate(&env);
    client.escrow_passkey(&vault_id, &attacker, &pk, &recovery);
}

#[test]
#[should_panic(expected = "PasskeyNotFound")]
fn test_escrow_passkey_unknown_hash() {
    let (env, owner, _beneficiary, client, vault_id, _pk) = setup_vault_with_pk(3);
    let unknown_pk = make_pk(&env, 99);
    let recovery = Address::generate(&env);
    client.escrow_passkey(&vault_id, &owner, &unknown_pk, &recovery);
}

#[test]
#[should_panic(expected = "InvalidBeneficiary")]
fn test_escrow_passkey_recovery_equals_owner() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_pk(4);
    // recovery_contact == owner → InvalidBeneficiary
    client.escrow_passkey(&vault_id, &owner, &pk, &owner);
}

#[test]
#[should_panic(expected = "AlreadyInEscrow")]
fn test_escrow_passkey_already_in_escrow() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_pk(5);
    let recovery = Address::generate(&env);
    client.escrow_passkey(&vault_id, &owner, &pk, &recovery);
    // Second call should fail.
    client.escrow_passkey(&vault_id, &owner, &pk, &recovery);
}

#[test]
fn test_escrowed_passkey_blocks_owner_check_in() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_pk(6);
    let recovery = Address::generate(&env);
    client.escrow_passkey(&vault_id, &owner, &pk, &recovery);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    // Owner should not be able to check in with the escrowed passkey.
    let result = client.try_check_in(&vault_id, &owner, &pk, &0u64, &None, &None);
    assert!(result.is_err());
}

#[test]
fn test_release_escrow_passkey_by_recovery_contact() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_pk(7);
    let recovery = Address::generate(&env);
    client.escrow_passkey(&vault_id, &owner, &pk, &recovery);

    client.release_escrow_passkey(&vault_id, &recovery, &pk);

    // Record should be removed.
    assert!(client.get_passkey_escrow(&vault_id, &pk).is_none());
}

#[test]
fn test_owner_can_check_in_after_escrow_release() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_pk(8);
    let recovery = Address::generate(&env);
    client.escrow_passkey(&vault_id, &owner, &pk, &recovery);
    client.release_escrow_passkey(&vault_id, &recovery, &pk);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    // Should succeed now.
    client.check_in(&vault_id, &owner, &pk, &0u64, &None, &None);
}

#[test]
#[should_panic(expected = "NotRecoveryContact")]
fn test_release_escrow_wrong_caller() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_pk(9);
    let recovery = Address::generate(&env);
    let stranger = Address::generate(&env);
    client.escrow_passkey(&vault_id, &owner, &pk, &recovery);

    // stranger is not the recovery contact.
    client.release_escrow_passkey(&vault_id, &stranger, &pk);
}

#[test]
#[should_panic(expected = "PasskeyNotFound")]
fn test_release_escrow_no_record() {
    let (env, _owner, _beneficiary, client, vault_id, pk) = setup_vault_with_pk(10);
    let recovery = Address::generate(&env);
    client.release_escrow_passkey(&vault_id, &recovery, &pk);
}

#[test]
fn test_cancel_passkey_escrow_by_owner() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_pk(11);
    let recovery = Address::generate(&env);
    client.escrow_passkey(&vault_id, &owner, &pk, &recovery);

    client.cancel_passkey_escrow(&vault_id, &owner, &pk);

    assert!(client.get_passkey_escrow(&vault_id, &pk).is_none());
}

#[test]
#[should_panic]
fn test_cancel_passkey_escrow_requires_owner() {
    let (env, _owner, _beneficiary, client, vault_id, pk) = setup_vault_with_pk(12);
    let recovery = Address::generate(&env);
    let stranger = Address::generate(&env);
    // Need to escrow first (use owner, mock_all_auths handles it).
    let (env2, owner2, _, client2, vault_id2, pk2) = setup_vault_with_pk(12);
    client2.escrow_passkey(&vault_id2, &owner2, &pk2, &recovery);
    // Stranger tries to cancel.
    client2.cancel_passkey_escrow(&vault_id2, &stranger, &pk2);
}

#[test]
fn test_owner_can_check_in_after_escrow_cancel() {
    let (env, owner, _beneficiary, client, vault_id, pk) = setup_vault_with_pk(13);
    let recovery = Address::generate(&env);
    client.escrow_passkey(&vault_id, &owner, &pk, &recovery);
    client.cancel_passkey_escrow(&vault_id, &owner, &pk);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &pk, &0u64, &None, &None);
}

#[test]
fn test_get_passkey_escrow_none_when_absent() {
    let (env, _owner, _beneficiary, client, vault_id, pk) = setup_vault_with_pk(14);
    assert!(client.get_passkey_escrow(&vault_id, &pk).is_none());
}
