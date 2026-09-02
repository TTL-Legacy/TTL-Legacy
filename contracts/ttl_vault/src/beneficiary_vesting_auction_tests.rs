#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{storage::Instance as _, Address as _, Events, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
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

    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    (env, owner, beneficiary, admin, token_address, client)
}

fn create_vault_with_deposit(
    env: &Env,
    client: &TtlVaultContractClient,
    owner: &Address,
    beneficiary: &Address,
    amount: i128,
) -> u64 {
    let vault_id = client.create_vault(owner, beneficiary, &100u64, &None);
    client.deposit(&vault_id, owner, &amount);
    vault_id
}

// ========== Issue #525: Beneficiary Vesting Tests ==========

#[test]
fn test_set_beneficiary_vesting_schedule() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 1_000_000);

    let start_time = env.ledger().timestamp() + 100;
    let interval = 7_884_000u64; // ~91 days
    let num_installments = 4u32;
    let cliff_period = 0u64;

    // Set beneficiary-specific vesting schedule
    let result = client.try_set_beneficiary_vesting(
        &vault_id,
        &owner,
        &beneficiary,
        &start_time,
        &interval,
        &num_installments,
        &cliff_period,
    );

    assert!(result.is_ok());

    // Verify schedule was stored
    let schedule = client.get_beneficiary_vesting(&vault_id, &beneficiary);
    assert!(schedule.is_some());
    let sched = schedule.unwrap();
    assert_eq!(sched.beneficiary, beneficiary);
    assert_eq!(sched.start_time, start_time);
    assert_eq!(sched.interval, interval);
    assert_eq!(sched.num_installments, num_installments);
    assert_eq!(sched.claimed_installments, 0u32);
}

#[test]
fn test_beneficiary_vesting_requires_owner() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 1_000_000);
    let non_owner = Address::generate(&env);

    let start_time = env.ledger().timestamp() + 100;

    let result = client.try_set_beneficiary_vesting(
        &vault_id,
        &non_owner,
        &beneficiary,
        &start_time,
        &7_884_000u64,
        &4u32,
        &0u64,
    );

    assert!(result.is_err());
}

#[test]
fn test_beneficiary_vesting_with_cliff() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 4_000_000);

    let start_time = env.ledger().timestamp() + 100;
    let interval = 7_884_000u64;
    let cliff_period = 31_536_000u64; // 1 year

    client.set_beneficiary_vesting(
        &vault_id,
        &owner,
        &beneficiary,
        &start_time,
        &interval,
        &4u32,
        &cliff_period,
    );

    let schedule = client.get_beneficiary_vesting(&vault_id, &beneficiary);
    let sched = schedule.unwrap();
    assert_eq!(sched.cliff_period, cliff_period);
}

#[test]
fn test_claim_beneficiary_vesting_installment() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 4_000_000);

    let start_time = 100u64;
    env.ledger().set_timestamp(start_time);

    client.set_beneficiary_vesting(
        &vault_id,
        &owner,
        &beneficiary,
        &start_time,
        &86_400u64, // 1 day interval
        &4u32,
        &0u64,
    );

    // Trigger release to enable claims
    client.trigger_release(&vault_id);

    // Move time forward past first installment
    env.ledger().set_timestamp(start_time + 86_400 + 10);

    let result = client.try_claim_beneficiary_vesting(&vault_id, &beneficiary);
    assert!(result.is_ok());

    // Verify claimed_installments incremented
    let schedule = client.get_beneficiary_vesting(&vault_id, &beneficiary);
    let sched = schedule.unwrap();
    assert!(sched.claimed_installments > 0);
}

#[test]
fn test_multiple_beneficiary_vesting_schedules() {
    let (env, owner, _, _, _, client) = setup();

    let ben_a = Address::generate(&env);
    let ben_b = Address::generate(&env);

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &ben_a, 8_000_000);

    let start_time = env.ledger().timestamp() + 100;

    client.set_beneficiary_vesting(
        &vault_id,
        &owner,
        &ben_a,
        &start_time,
        &86_400u64,
        &4u32,
        &0u64,
    );
    client.set_beneficiary_vesting(
        &vault_id,
        &owner,
        &ben_b,
        &start_time,
        &86_400u64,
        &2u32,
        &0u64,
    );

    let sched_a = client.get_beneficiary_vesting(&vault_id, &ben_a);
    let sched_b = client.get_beneficiary_vesting(&vault_id, &ben_b);

    assert!(sched_a.is_some());
    assert!(sched_b.is_some());
    assert_eq!(sched_a.unwrap().num_installments, 4u32);
    assert_eq!(sched_b.unwrap().num_installments, 2u32);
}

#[test]
fn test_beneficiary_vesting_rejects_zero_interval() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 1_000_000);

    let result = client.try_set_beneficiary_vesting(
        &vault_id,
        &owner,
        &beneficiary,
        &100u64,
        &0u64, // Invalid: zero interval
        &4u32,
        &0u64,
    );

    assert!(result.is_err());
}

#[test]
fn test_beneficiary_vesting_rejects_zero_installments() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 1_000_000);

    let result = client.try_set_beneficiary_vesting(
        &vault_id,
        &owner,
        &beneficiary,
        &100u64,
        &86_400u64,
        &0u32, // Invalid: zero installments
        &0u64,
    );

    assert!(result.is_err());
}

// ========== Issue #527: Beneficiary Auction Tests ==========

#[test]
fn test_create_beneficiary_auction() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 10_000_000);

    let start_time = env.ledger().timestamp() + 100;
    let end_time = start_time + 604_800u64; // 1 week
    let total_allocation_bps = 10_000u32; // 100%
    let minimum_bid = 100_000i128;

    let result = client.try_create_beneficiary_auction(
        &vault_id,
        &owner,
        &start_time,
        &end_time,
        &total_allocation_bps,
        &minimum_bid,
    );

    assert!(result.is_ok());

    let auction = client.get_beneficiary_auction(&vault_id);
    assert!(auction.is_some());
    let auc = auction.unwrap();
    assert_eq!(auc.vault_id, vault_id);
    assert_eq!(auc.total_allocation_bps, total_allocation_bps);
    assert_eq!(auc.minimum_bid, minimum_bid);
    assert!(!auc.finalized);
}

#[test]
fn test_auction_requires_owner() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 10_000_000);
    let non_owner = Address::generate(&env);

    let result = client.try_create_beneficiary_auction(
        &vault_id,
        &non_owner,
        &100u64,
        &200u64,
        &10_000u32,
        &100_000i128,
    );

    assert!(result.is_err());
}

#[test]
fn test_place_auction_bid() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 10_000_000);

    let start_time = env.ledger().timestamp() + 100;
    let end_time = start_time + 604_800u64;

    client.create_beneficiary_auction(
        &vault_id,
        &owner,
        &start_time,
        &end_time,
        &10_000u32,
        &100_000i128,
    );

    // Move time into auction window
    env.ledger().set_timestamp(start_time + 10);

    let bidder_a = Address::generate(&env);
    let bid_amount = 500_000i128;
    let desired_allocation = 5_000u32; // 50%

    let result =
        client.try_place_auction_bid(&vault_id, &bidder_a, &bid_amount, &desired_allocation);

    assert!(result.is_ok());
}

#[test]
fn test_auction_bid_requires_minimum() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 10_000_000);

    let start_time = env.ledger().timestamp() + 100;
    let end_time = start_time + 604_800u64;
    let minimum_bid = 500_000i128;

    client.create_beneficiary_auction(
        &vault_id,
        &owner,
        &start_time,
        &end_time,
        &10_000u32,
        &minimum_bid,
    );

    env.ledger().set_timestamp(start_time + 10);

    let bidder = Address::generate(&env);
    let insufficient_bid = minimum_bid - 1;

    let result = client.try_place_auction_bid(&vault_id, &bidder, &insufficient_bid, &5_000u32);

    assert!(result.is_err());
}

#[test]
fn test_auction_bid_outside_window() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 10_000_000);

    let start_time = env.ledger().timestamp() + 100;
    let end_time = start_time + 604_800u64;

    client.create_beneficiary_auction(
        &vault_id,
        &owner,
        &start_time,
        &end_time,
        &10_000u32,
        &100_000i128,
    );

    // Try to bid before auction starts
    let bidder = Address::generate(&env);
    let result = client.try_place_auction_bid(&vault_id, &bidder, &500_000i128, &5_000u32);

    assert!(result.is_err());
}

#[test]
fn test_finalize_auction_selects_winner() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 10_000_000);

    let start_time = env.ledger().timestamp() + 100;
    let end_time = start_time + 604_800u64;

    client.create_beneficiary_auction(
        &vault_id,
        &owner,
        &start_time,
        &end_time,
        &10_000u32,
        &100_000i128,
    );

    env.ledger().set_timestamp(start_time + 10);

    let bidder_a = Address::generate(&env);
    let bidder_b = Address::generate(&env);

    // Bidder A: lower bid
    client.place_auction_bid(&vault_id, &bidder_a, &200_000i128, &3_000u32);

    // Bidder B: higher bid
    client.place_auction_bid(&vault_id, &bidder_b, &800_000i128, &7_000u32);

    // Move past auction end time
    env.ledger().set_timestamp(end_time + 1);

    let result = client.try_finalize_beneficiary_auction(&vault_id);
    assert!(result.is_ok());

    let auction = client.get_beneficiary_auction(&vault_id);
    assert!(auction.unwrap().finalized);
    // Highest bidder should win
    assert_eq!(auction.unwrap().winner, Some(bidder_b));
}

#[test]
fn test_auction_cannot_be_created_twice() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 10_000_000);

    let start_time = env.ledger().timestamp() + 100;
    let end_time = start_time + 604_800u64;

    client.create_beneficiary_auction(
        &vault_id,
        &owner,
        &start_time,
        &end_time,
        &10_000u32,
        &100_000i128,
    );

    let result = client.try_create_beneficiary_auction(
        &vault_id,
        &owner,
        &start_time,
        &end_time,
        &5_000u32,
        &100_000i128,
    );

    assert!(result.is_err());
}

#[test]
fn test_multiple_bids_in_auction() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 10_000_000);

    let start_time = env.ledger().timestamp() + 100;
    let end_time = start_time + 604_800u64;

    client.create_beneficiary_auction(
        &vault_id,
        &owner,
        &start_time,
        &end_time,
        &10_000u32,
        &100_000i128,
    );

    env.ledger().set_timestamp(start_time + 10);

    // Place 3 bids
    for i in 0..3 {
        let bidder = Address::generate(&env);
        let bid_amount = 300_000i128 + (i as i128 * 100_000i128);
        client.place_auction_bid(&vault_id, &bidder, &bid_amount, &3_000u32);
    }

    let auction = client.get_beneficiary_auction(&vault_id);
    assert_eq!(auction.unwrap().bids.len(), 3);
}

#[test]
fn test_get_auction_bids() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let vault_id = create_vault_with_deposit(&env, &client, &owner, &beneficiary, 10_000_000);

    let start_time = env.ledger().timestamp() + 100;
    let end_time = start_time + 604_800u64;

    client.create_beneficiary_auction(
        &vault_id,
        &owner,
        &start_time,
        &end_time,
        &10_000u32,
        &100_000i128,
    );

    env.ledger().set_timestamp(start_time + 10);

    let bidder = Address::generate(&env);
    client.place_auction_bid(&vault_id, &bidder, &500_000i128, &5_000u32);

    let bids = client.get_beneficiary_auction_bids(&vault_id);
    assert_eq!(bids.len(), 1);
    assert_eq!(bids.get(0).unwrap().bidder, bidder);
}
