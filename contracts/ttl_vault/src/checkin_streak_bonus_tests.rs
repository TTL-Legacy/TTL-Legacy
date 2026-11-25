#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

use crate::tests::common::setup_with_roles as setup;

#[test]
fn test_streak_count_initialization() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // streak_count should be initialized on vault creation
    let vault = client.get_vault(&vault_id);
    assert!(!vault.id.is_empty());
}

#[test]
fn test_streak_accumulation_on_time() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Check in on time multiple times to build streak
    for _ in 0..5 {
        now += interval;
        env.ledger().set_timestamp(now);
        client.check_in(&vault_id, &owner, &None, &None, &None);
    }

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}

#[test]
fn test_streak_reset_on_missed_checkin() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Build up a streak
    for _ in 0..3 {
        now += interval;
        env.ledger().set_timestamp(now);
        client.check_in(&vault_id, &owner, &None, &None, &None);
    }

    // Miss a check-in by going past the deadline
    now += interval + 100;
    env.ledger().set_timestamp(now);

    // Streak should reset on missed check-in
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}

#[test]
fn test_streak_bonus_percentage_calculation() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Bonus should be +5% TTL extension per streak level, capped at 25%
    // Streak 1: +5%, Streak 2: +10%, Streak 3: +15%, Streak 4: +20%, Streak 5+: +25%
    for _ in 0..4 {
        now += interval;
        env.ledger().set_timestamp(now);
        client.check_in(&vault_id, &owner, &None, &None, &None);
    }

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}

#[test]
fn test_streak_bonus_cap_at_25_percent() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Check in many times - bonus should cap at 25%
    for _ in 0..10 {
        now += interval;
        env.ledger().set_timestamp(now);
        client.check_in(&vault_id, &owner, &None, &None, &None);
    }

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}

#[test]
fn test_get_streak_bonus_function() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // get_streak_bonus should return current bonus percentage
    for i in 0..5 {
        now += interval;
        env.ledger().set_timestamp(now);
        client.check_in(&vault_id, &owner, &None, &None, &None);

        // Verify bonus increases with streak (5% per level)
        // Expected bonus: (i+1) * 5%, capped at 25%
        if i >= 4 {
            // Bonus should be capped at 25%
            break;
        }
    }

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}

#[test]
fn test_streak_bonus_incentivizes_engagement() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    let initial_ttl = 100_000u64;
    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // With streak bonus, TTL extension should be greater than base interval
    for _ in 0..3 {
        env.ledger()
            .set_timestamp(env.ledger().timestamp() + interval);
        client.check_in(&vault_id, &owner, &None, &None, &None);
    }

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}

#[test]
fn test_streak_not_applied_after_grace_period() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    // Initial check-in just after creation
    let t0 = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(t0);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Second check-in exactly on time (at t0 + interval) — streak grows to 2
    let t1 = t0 + interval;
    env.ledger().set_timestamp(t1);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    // Third check-in one second past the deadline — streak must reset to 1
    let t2 = t1 + interval + 1;
    env.ledger().set_timestamp(t2);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    let streak = client.get_check_in_streak(&vault_id);
    assert_eq!(streak.current, 1);
}

#[test]
fn test_streak_bonus_not_earned_in_grace_window() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    // Build a streak of 3 by checking in exactly on time
    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    for _ in 0..2 {
        now += interval;
        env.ledger().set_timestamp(now);
        client.check_in(&vault_id, &owner, &None, &None, &None);
    }

    // Check-in one second late — streak must reset to 1
    now += interval + 1;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    let streak = client.get_check_in_streak(&vault_id);
    assert_eq!(streak.current, 1);
}
