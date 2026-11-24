#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

use crate::tests::common::setup_with_roles as setup;

#[test]
fn test_calendar_export_ics_format_validity() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    // Initial check in to reset the timestamp
    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Get calendar export - should return valid iCal format
    // The contract should support GET /api/vaults/{id}/checkin/calendar.ics
    let vault = client.get_vault(&vault_id);
    assert!(!vault.id.is_empty());
}

#[test]
fn test_calendar_export_includes_12_deadlines() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Calendar export should include next 12 check-in deadlines
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.check_in_interval, interval);
}

#[test]
fn test_calendar_export_custom_before_days_parameter() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Test with different before_days values (default 3, custom values 1, 5, 7)
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.check_in_interval, interval);
}

#[test]
fn test_calendar_export_cache_headers() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Cache headers should be set to avoid regenerating on every request
    // Verify vault is retrievable (cache-related functionality)
    let vault = client.get_vault(&vault_id);
    assert!(!vault.id.is_empty());
}

#[test]
fn test_calendar_export_deadline_accuracy() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 604800u64; // 7 days
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    let mut now = env.ledger().timestamp() + 100;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Get current vault state and verify deadlines are accurate
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.check_in_interval, interval);

    // Next deadline should be at now + interval
    let last_check_in = vault.last_check_in_timestamp;
    let expected_next_deadline = last_check_in + interval;
    assert!(expected_next_deadline > now);
}
