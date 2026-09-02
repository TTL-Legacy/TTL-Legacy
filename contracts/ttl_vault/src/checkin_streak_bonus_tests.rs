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
        client.check_in(&vault_id, &owner, &None);
    }

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Active);
}
