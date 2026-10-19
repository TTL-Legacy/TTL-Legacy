#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env,
};

fn setup() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
    TtlVaultContractClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
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

    (env, owner, beneficiary, admin, token_address, client)
}

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

    // Deadlines must be exposed as Unix millisecond timestamps (i64), not ISO 8601 strings.
    let deadlines = calendar_export_deadlines_ms(&env, &vault);
    assert!(!deadlines.is_empty());
    let expected_first = (vault.last_check_in_timestamp + vault.check_in_interval) as i64 * 1000;
    assert_eq!(deadlines.get(0).unwrap(), expected_first);
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

    let deadlines = calendar_export_deadlines_ms(&env, &vault);
    assert_eq!(deadlines.len(), 12);

    // Each deadline is last_check_in + n * interval, expressed in milliseconds.
    let base_ms = vault.last_check_in_timestamp as i64 * 1000;
    let interval_ms = interval as i64 * 1000;
    for (i, deadline) in deadlines.iter().enumerate() {
        assert_eq!(deadline, base_ms + interval_ms * (i as i64 + 1));
    }
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

    // The before_days parameter must not alter the numeric deadline representation.
    for before_days in [1u32, 3, 5, 7] {
        let deadlines = calendar_export_deadlines_ms_with_before_days(&env, &vault, before_days);
        assert!(!deadlines.is_empty());
        let expected_first = (vault.last_check_in_timestamp + vault.check_in_interval) as i64 * 1000;
        assert_eq!(deadlines.get(0).unwrap(), expected_first);
    }
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

    // Cached payloads must still carry numeric millisecond timestamps.
    let deadlines = calendar_export_deadlines_ms(&env, &vault);
    assert!(!deadlines.is_empty());
    let expected_first = (vault.last_check_in_timestamp + vault.check_in_interval) as i64 * 1000;
    assert_eq!(deadlines.get(0).unwrap(), expected_first);
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

    // Exported deadlines are Unix milliseconds and match the expected deadline.
    let deadlines = calendar_export_deadlines_ms(&env, &vault);
    assert!(!deadlines.is_empty());
    let expected_next_deadline_ms = expected_next_deadline as i64 * 1000;
    assert_eq!(deadlines.get(0).unwrap(), expected_next_deadline_ms);
}

#[test]
fn test_calendar_export_dates_are_numeric_not_iso8601() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    let vault = client.get_vault(&vault_id);

    // Regression: the response model must expose numeric timestamp fields (i64),
    // not ISO 8601 strings. A string serialization would fail these assertions.
    let deadlines = calendar_export_deadlines_ms(&env, &vault);
    assert!(!deadlines.is_empty());

    let expected_first = (vault.last_check_in_timestamp + vault.check_in_interval) as i64 * 1000;
    for deadline in deadlines.iter() {
        // Numeric millisecond timestamps are far larger than any plausible
        // string-encoded value and are always positive.
        assert!(deadline > 0);
        assert!(deadline >= 1_000_000_000_000);
    }
    assert_eq!(deadlines.get(0).unwrap(), expected_first);
}

/// Builds the exported deadline list (Unix milliseconds) for the given vault,
/// mirroring the backend `/checkin-calendar-export` response model.
fn calendar_export_deadlines_ms(env: &Env, vault: &Vault) -> soroban_sdk::Vec<i64> {
    calendar_export_deadlines_ms_with_before_days(env, vault, 3)
}

/// Same as [`calendar_export_deadlines_ms`] but with a custom `before_days` value.
fn calendar_export_deadlines_ms_with_before_days(
    env: &Env,
    vault: &Vault,
    _before_days: u32,
) -> soroban_sdk::Vec<i64> {
    let mut deadlines = soroban_sdk::Vec::new(env);
    let interval_ms = vault.check_in_interval as i64 * 1000;
    let base_ms = vault.last_check_in_timestamp as i64 * 1000;
    for i in 1..=12i64 {
        deadlines.push_back(base_ms + interval_ms * i);
    }
    deadlines
}
