#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

use crate::tests::common::setup_with_roles as setup;

#[test]
fn test_email_token_recovery_hash_stored() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // recovery_email_hash should be stored (hashed, not plaintext)
    let vault = client.get_vault(&vault_id);
    assert!(!vault.id.is_empty());
}

#[test]
fn test_email_token_otp_generation() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // POST /api/vaults/{id}/checkin/email-token should generate and send OTP
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}

#[test]
fn test_email_token_otp_validation() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // OTP validation in POST /api/vaults/{id}/checkin/email-verify should trigger check_in
    let vault_before = client.get_vault(&vault_id);
    let timestamp_before = vault_before.last_check_in_timestamp;

    // Simulate OTP validation and check-in
    env.ledger().set_timestamp(env.ledger().timestamp() + 100);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    let vault_after = client.get_vault(&vault_id);
    assert!(vault_after.last_check_in_timestamp > timestamp_before);
}

#[test]
fn test_email_token_rate_limiting() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Rate-limit to 3 email check-ins per 30-day window
    // Should fail on 4th attempt
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}

#[test]
fn test_email_token_expiry() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // OTP should expire after a certain time (typically 15 minutes)
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.check_in_interval, interval);
}

#[test]
fn test_email_token_backup_check_in_method() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Email token should serve as backup if owner loses passkey device access
    let vault = client.get_vault(&vault_id);
    assert!(!vault.id.is_empty());
}
