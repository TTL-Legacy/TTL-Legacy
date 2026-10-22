//! Tests for Issues #1525, #1526, #1527 and #1528.
//!
//! Covers:
//!  - #1525: `remove_passkey` emits a `passkey_revoked` event carrying the
//!    credential ID hash and the ledger timestamp
//!  - #1526: `set_vesting_schedule` rejects cliffs that end after the schedule
//!    end and schedules whose end overflows
//!  - #1527: property tests — vested (claimed) amount is monotonic over time,
//!    never exceeds the schedule total, and is fully released at the end
//!  - #1528: withdrawal whitelist entries can expire at a ledger sequence

#![cfg(test)]

extern crate alloc;

use super::*;
use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    token::{self, StellarAssetClient},
    Address, BytesN, Env, IntoVal, String, Symbol, TryIntoVal, Val,
};

const INTERVAL: u64 = MIN_CHECK_IN_INTERVAL;

fn setup() -> (
    Env,
    Address, // owner
    Address, // beneficiary
    Address, // token_address
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
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, beneficiary, token_address, client)
}

// ---------------------------------------------------------------------------
// #1525: passkey_revoked event
// ---------------------------------------------------------------------------

#[test]
fn test_remove_passkey_emits_passkey_revoked_event() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    let passkey_hash = BytesN::<32>::from_array(&env, &[7u8; 32]);
    client.add_passkey(&vault_id, &owner, &passkey_hash);

    let revoked_at = 12_345u64;
    env.ledger().set_timestamp(revoked_at);
    client.remove_passkey(&vault_id, &owner, &passkey_hash);

    let event = env
        .events()
        .all()
        .iter()
        .find(|e| {
            let topics: soroban_sdk::Vec<Val> = e.1.clone().into_val(&env);
            topics
                .get(0)
                .and_then(|t| t.try_into_val(&env).ok())
                .map(|s: Symbol| s == PASSKEY_REVOKED_TOPIC)
                .unwrap_or(false)
        })
        .expect("passkey_revoked event not emitted");

    let topics: soroban_sdk::Vec<Val> = event.1.clone().into_val(&env);
    let topic_vault_id: u64 = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic_vault_id, vault_id);

    let (hash, timestamp): (BytesN<32>, u64) = event.2.into_val(&env);
    assert_eq!(hash, passkey_hash);
    assert_eq!(timestamp, revoked_at);
}

#[test]
fn test_remove_unknown_passkey_emits_no_revoked_event() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    let unknown = BytesN::<32>::from_array(&env, &[9u8; 32]);

    let res = client.try_remove_passkey(&vault_id, &owner, &unknown);
    assert_eq!(res, Err(Ok(ContractError::PasskeyNotFound)));
}

// ---------------------------------------------------------------------------
// #1526: vesting schedule validation
// ---------------------------------------------------------------------------

#[test]
fn test_vesting_rejects_cliff_after_end() {
    let (_env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    // duration = 100 * 4 = 400, cliff = 401 > duration
    let res = client.try_set_vesting_schedule(
        &vault_id, &owner, &10_000u64, &100u64, &4u32, &1_000_000i128, &401u64,
    );
    assert_eq!(res, Err(Ok(ContractError::VestingCliffAfterEnd)));
}

#[test]
fn test_vesting_accepts_cliff_equal_to_end() {
    let (_env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let res = client.try_set_vesting_schedule(
        &vault_id, &owner, &10_000u64, &100u64, &4u32, &1_000_000i128, &400u64,
    );
    assert!(res.is_ok());
}

#[test]
fn test_vesting_rejects_zero_interval() {
    let (_env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let res = client.try_set_vesting_schedule(
        &vault_id, &owner, &10_000u64, &0u64, &4u32, &1_000_000i128, &0u64,
    );
    assert_eq!(res, Err(Ok(ContractError::InvalidInterval)));
}

#[test]
fn test_vesting_rejects_overflowing_end() {
    let (_env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let res = client.try_set_vesting_schedule(
        &vault_id, &owner, &(u64::MAX - 10), &100u64, &4u32, &1_000_000i128, &0u64,
    );
    assert_eq!(res, Err(Ok(ContractError::InvalidVestingSchedule)));
}

#[test]
fn test_vesting_rejects_overflowing_cliff() {
    let (_env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let res = client.try_set_vesting_schedule(
        &vault_id, &owner, &10_000u64, &100u64, &4u32, &1_000_000i128, &u64::MAX,
    );
    assert_eq!(res, Err(Ok(ContractError::VestingCliffAfterEnd)));
}

// ---------------------------------------------------------------------------
// #1527: property tests for vesting release amounts
// ---------------------------------------------------------------------------

/// Creates a vault with a single vesting schedule, releases it, and returns
/// the handles needed to claim against it.
fn setup_released_schedule(
    start_offset: u64,
    interval: u64,
    num_installments: u32,
    total: i128,
    cliff_period: u64,
) -> (Env, Address, Address, u64, u64, TtlVaultContractClient<'static>) {
    let (env, owner, beneficiary, token_address, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &total);

    // Vesting starts after the vault has been released.
    let start_time = INTERVAL + 1 + start_offset;
    client.set_vesting_schedule(
        &vault_id,
        &owner,
        &start_time,
        &interval,
        &num_installments,
        &total,
        &cliff_period,
    );

    env.ledger().set_timestamp(INTERVAL + 1);
    client.trigger_release(&vault_id);

    (env, beneficiary, token_address, vault_id, start_time, client)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn prop_vested_amount_monotonic_capped_and_fully_released(
        start_offset in 0u64..10_000,
        interval in 1u64..10_000,
        num_installments in 1u32..=12,
        total in 1i128..=5_000_000,
        cliff_frac in 0u64..=100,
        mut sample_offsets in proptest::collection::vec(0u64..200_000, 1..8),
    ) {
        let duration = interval * num_installments as u64;
        let cliff_period = duration * cliff_frac / 100;

        let (env, beneficiary, token_address, vault_id, start_time, client) =
            setup_released_schedule(start_offset, interval, num_installments, total, cliff_period);
        let token_client = token::Client::new(&env, &token_address);

        sample_offsets.sort_unstable();
        let mut previous = 0i128;
        for offset in sample_offsets.iter() {
            env.ledger().set_timestamp(start_time + offset);
            let _ = client.try_claim_vested(&vault_id, &0u32, &beneficiary);

            let vested = token_client.balance(&beneficiary);
            prop_assert!(vested >= previous, "vested amount decreased: {} -> {}", previous, vested);
            prop_assert!(vested <= total, "vested {} exceeds total {}", vested, total);
            previous = vested;
        }

        // At (or after) the schedule end the full amount must be released.
        let end_time = start_time + duration;
        let now = env.ledger().timestamp();
        env.ledger().set_timestamp(now.max(end_time));
        let _ = client.try_claim_vested(&vault_id, &0u32, &beneficiary);

        prop_assert_eq!(token_client.balance(&beneficiary), total);
        let schedule = client.get_vesting_schedule_by_id(&vault_id, &0u32).unwrap();
        prop_assert_eq!(schedule.claimed_installments, num_installments);
    }
}

// ---------------------------------------------------------------------------
// #1528: withdrawal whitelist expiry
// ---------------------------------------------------------------------------

#[test]
fn test_whitelist_entry_without_expiry_never_expires() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    client.add_whitelist_address(&vault_id, &owner, &owner, &String::from_str(&env, "me"));
    let entry = client.get_whitelist(&vault_id).unwrap().get(0).unwrap();
    assert_eq!(entry.expires_at_ledger, 0);

    env.ledger().with_mut(|l| l.sequence_number += 1_000_000);
    client.withdraw(&vault_id, &owner, &100, &None, &None, &None);
}

#[test]
fn test_whitelist_entry_valid_until_expiry_ledger() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let expiry = env.ledger().sequence() + 100;
    client.add_whitelist_with_expiry(
        &vault_id,
        &owner,
        &owner,
        &String::from_str(&env, "me"),
        &expiry,
    );
    assert_eq!(
        client.get_whitelist(&vault_id).unwrap().get(0).unwrap().expires_at_ledger,
        expiry
    );

    // Exactly at the expiry ledger the entry is still valid.
    env.ledger().with_mut(|l| l.sequence_number = expiry);
    client.withdraw(&vault_id, &owner, &100, &None, &None, &None);
}

#[test]
fn test_expired_whitelist_entry_is_rejected() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let expiry = env.ledger().sequence() + 100;
    client.add_whitelist_with_expiry(
        &vault_id,
        &owner,
        &owner,
        &String::from_str(&env, "me"),
        &expiry,
    );

    env.ledger().with_mut(|l| l.sequence_number = expiry + 1);
    let res = client.try_withdraw(&vault_id, &owner, &100, &None, &None, &None);
    assert_eq!(res, Err(Ok(ContractError::WhitelistEntryExpired)));
}

#[test]
fn test_whitelist_expiry_must_be_in_future() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);

    let current = env.ledger().sequence();
    let res = client.try_add_whitelist_with_expiry(
        &vault_id,
        &owner,
        &owner,
        &String::from_str(&env, "me"),
        &current,
    );
    assert_eq!(res, Err(Ok(ContractError::InvalidWhitelistExpiry)));
}

#[test]
fn test_readding_whitelist_address_replaces_expired_entry() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let expiry = env.ledger().sequence() + 10;
    client.add_whitelist_with_expiry(
        &vault_id,
        &owner,
        &owner,
        &String::from_str(&env, "me"),
        &expiry,
    );
    env.ledger().with_mut(|l| l.sequence_number = expiry + 1);

    client.add_whitelist_address(&vault_id, &owner, &owner, &String::from_str(&env, "me"));
    assert_eq!(client.get_whitelist(&vault_id).unwrap().len(), 1);
    client.withdraw(&vault_id, &owner, &100, &None, &None, &None);
}
