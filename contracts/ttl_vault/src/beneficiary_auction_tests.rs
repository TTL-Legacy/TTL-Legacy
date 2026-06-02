#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

fn setup_auction() -> (
    Env,
    Address,
    Address,
    u64,
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

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &10_000_000);

    (env, owner, beneficiary, vault_id, client)
}

// ========== Beneficiary Auction Edge Cases ==========

#[test]
fn test_auction_timing_validation() {
    let (env, owner, _, vault_id, client) = setup_auction();

    let now = env.ledger().timestamp() as u64;
    let start = now + 100;
    let end = start + 604_800u64;

    client.create_beneficiary_auction(&vault_id, &owner, &start, &end, &10_000u32, &100_000i128);

    // Before auction starts
    env.ledger().set_timestamp((start - 1) as u64);
    let bidder = Address::generate(&env);
    let result = client.try_place_auction_bid(&vault_id, &bidder, &500_000i128, &5_000u32);
    assert!(result.is_err()); // Should fail - auction hasn't started

    // During auction
    env.ledger().set_timestamp((start + 10) as u64);
    let result = client.try_place_auction_bid(&vault_id, &bidder, &500_000i128, &5_000u32);
    assert!(result.is_ok());

    // After auction ends
    env.ledger().set_timestamp((end + 1) as u64);
    let bidder2 = Address::generate(&env);
    let result = client.try_place_auction_bid(&vault_id, &bidder2, &500_000i128, &5_000u32);
    assert!(result.is_err()); // Should fail - auction ended
}

#[test]
fn test_auction_minimum_bid_enforcement() {
    let (env, owner, _, vault_id, client) = setup_auction();

    let now = env.ledger().timestamp() as u64;
    let start = now + 100;
    let end = start + 604_800u64;
    let minimum_bid = 1_000_000i128;

    client.create_beneficiary_auction(&vault_id, &owner, &start, &end, &10_000u32, &minimum_bid);

    env.ledger().set_timestamp((start + 10) as u64);

    let bidder = Address::generate(&env);

    // Below minimum
    let result = client.try_place_auction_bid(&vault_id, &bidder, &(minimum_bid - 1), &5_000u32);
    assert!(result.is_err());

    // At minimum
    let result = client.try_place_auction_bid(&vault_id, &bidder, &minimum_bid, &5_000u32);
    assert!(result.is_ok());

    // Above minimum
    let bidder2 = Address::generate(&env);
    let result = client.try_place_auction_bid(&vault_id, &bidder2, &(minimum_bid + 100_000), &5_000u32);
    assert!(result.is_ok());
}

#[test]
fn test_auction_winner_selection() {
    let (env, owner, _, vault_id, client) = setup_auction();

    let now = env.ledger().timestamp() as u64;
    let start = now + 100;
    let end = start + 604_800u64;

    client.create_beneficiary_auction(&vault_id, &owner, &start, &end, &10_000u32, &100_000i128);

    env.ledger().set_timestamp((start + 10) as u64);

    let bidder1 = Address::generate(&env);
    let bidder2 = Address::generate(&env);
    let bidder3 = Address::generate(&env);

    client.place_auction_bid(&vault_id, &bidder1, &200_000i128, &3_000u32);
    client.place_auction_bid(&vault_id, &bidder2, &900_000i128, &7_000u32); // Highest
    client.place_auction_bid(&vault_id, &bidder3, &400_000i128, &4_000u32);

    env.ledger().set_timestamp((end + 1) as u64);
    client.finalize_beneficiary_auction(&vault_id).unwrap();

    let auction = client.get_beneficiary_auction(&vault_id).unwrap();
    assert!(auction.finalized);
    assert_eq!(auction.winner, Some(bidder2));
}

#[test]
fn test_auction_with_no_bids() {
    let (env, owner, _, vault_id, client) = setup_auction();

    let now = env.ledger().timestamp() as u64;
    let start = now + 100;
    let end = start + 604_800u64;

    client.create_beneficiary_auction(&vault_id, &owner, &start, &end, &10_000u32, &100_000i128);

    env.ledger().set_timestamp((end + 1) as u64);
    let result = client.try_finalize_beneficiary_auction(&vault_id);

    assert!(result.is_ok());
    let auction = client.get_beneficiary_auction(&vault_id).unwrap();
    assert!(auction.finalized);
    assert_eq!(auction.winner, None);
}

#[test]
fn test_auction_allocation_tracking() {
    let (env, owner, _, vault_id, client) = setup_auction();

    let now = env.ledger().timestamp() as u64;
    let start = now + 100;
    let end = start + 604_800u64;
    let allocation = 5_000u32; // 50%

    client.create_beneficiary_auction(&vault_id, &owner, &start, &end, &allocation, &100_000i128);

    env.ledger().set_timestamp((start + 10) as u64);

    let bidder = Address::generate(&env);
    client.place_auction_bid(&vault_id, &bidder, &500_000i128, &allocation);

    let auction = client.get_beneficiary_auction(&vault_id).unwrap();
    assert_eq!(auction.total_allocation_bps, allocation);

    let bid = auction.bids.get(0).unwrap();
    assert_eq!(bid.desired_allocation_bps, allocation);
}

#[test]
fn test_auction_multiple_rounds() {
    let (env, owner, _, vault_id, client) = setup_auction();

    let now = env.ledger().timestamp() as u64;

    // First auction
    let start1 = now + 100;
    let end1 = start1 + 604_800u64;

    client.create_beneficiary_auction(&vault_id, &owner, &start1, &end1, &10_000u32, &100_000i128);

    // Can't create second auction for same vault
    let result = client.try_create_beneficiary_auction(
        &vault_id,
        &owner,
        &(end1 + 100),
        &(end1 + 700_800u64),
        &10_000u32,
        &100_000i128,
    );

    assert!(result.is_err());
}

#[test]
fn test_auction_bid_ordering() {
    let (env, owner, _, vault_id, client) = setup_auction();

    let now = env.ledger().timestamp() as u64;
    let start = now + 100;
    let end = start + 604_800u64;

    client.create_beneficiary_auction(&vault_id, &owner, &start, &end, &10_000u32, &100_000i128);

    env.ledger().set_timestamp((start + 10) as u64);

    let mut bidders = vec![];
    let mut bids = vec![];

    // Place 5 bids in random order
    for i in 0..5 {
        let bidder = Address::generate(&env);
        let amount = 300_000i128 + (i as i128 * 100_000i128);
        client.place_auction_bid(&vault_id, &bidder, &amount, &2_000u32);
        bidders.push(bidder);
        bids.push(amount);
    }

    let auction = client.get_beneficiary_auction(&vault_id).unwrap();
    assert_eq!(auction.bids.len(), 5);

    // Verify all bids recorded
    for i in 0..5 {
        let bid = auction.bids.get(i).unwrap();
        assert_eq!(bid.bidder, bidders[i]);
        assert_eq!(bid.bid_amount, bids[i]);
    }
}

#[test]
fn test_auction_bid_timestamp_recorded() {
    let (env, owner, _, vault_id, client) = setup_auction();

    let now = env.ledger().timestamp() as u64;
    let start = now + 100;
    let end = start + 604_800u64;

    client.create_beneficiary_auction(&vault_id, &owner, &start, &end, &10_000u32, &100_000i128);

    env.ledger().set_timestamp((start + 10) as u64);

    let bidder = Address::generate(&env);
    let bid_timestamp = (start + 10) as u64;

    client.place_auction_bid(&vault_id, &bidder, &500_000i128, &5_000u32);

    let auction = client.get_beneficiary_auction(&vault_id).unwrap();
    let bid = auction.bids.get(0).unwrap();

    assert_eq!(bid.bid_timestamp, bid_timestamp);
}

#[test]
fn test_auction_same_bidder_multiple_bids() {
    let (env, owner, _, vault_id, client) = setup_auction();

    let now = env.ledger().timestamp() as u64;
    let start = now + 100;
    let end = start + 604_800u64;

    client.create_beneficiary_auction(&vault_id, &owner, &start, &end, &10_000u32, &100_000i128);

    env.ledger().set_timestamp((start + 10) as u64);

    let bidder = Address::generate(&env);

    // Same bidder places multiple bids
    client.place_auction_bid(&vault_id, &bidder, &300_000i128, &3_000u32);
    client.place_auction_bid(&vault_id, &bidder, &700_000i128, &7_000u32); // Increase bid

    let auction = client.get_beneficiary_auction(&vault_id).unwrap();
    assert_eq!(auction.bids.len(), 2);

    let bid1 = auction.bids.get(0).unwrap();
    let bid2 = auction.bids.get(1).unwrap();

    assert_eq!(bid1.bidder, bidder);
    assert_eq!(bid2.bidder, bidder);
    assert_eq!(bid1.bid_amount, 300_000i128);
    assert_eq!(bid2.bid_amount, 700_000i128);
}

#[test]
fn test_auction_large_allocation_percentages() {
    let (env, owner, _, vault_id, client) = setup_auction();

    let now = env.ledger().timestamp() as u64;
    let start = now + 100;
    let end = start + 604_800u64;

    // 100% allocation
    client.create_beneficiary_auction(&vault_id, &owner, &start, &end, &10_000u32, &100_000i128);

    env.ledger().set_timestamp((start + 10) as u64);

    let bidder = Address::generate(&env);
    let result = client.try_place_auction_bid(&vault_id, &bidder, &500_000i128, &10_000u32);

    assert!(result.is_ok());

    let auction = client.get_beneficiary_auction(&vault_id).unwrap();
    let bid = auction.bids.get(0).unwrap();
    assert_eq!(bid.desired_allocation_bps, 10_000u32);
}

#[test]
fn test_auction_nonexistent_vault() {
    let (env, owner, _, _, client) = setup_auction();

    let now = env.ledger().timestamp() as u64;
    let start = now + 100;
    let end = start + 604_800u64;

    let nonexistent_vault = 9999u64;
    let result = client.try_create_beneficiary_auction(
        &nonexistent_vault,
        &owner,
        &start,
        &end,
        &10_000u32,
        &100_000i128,
    );

    assert!(result.is_err());
}

#[test]
fn test_auction_finalization_timing() {
    let (env, owner, _, vault_id, client) = setup_auction();

    let now = env.ledger().timestamp() as u64;
    let start = now + 100;
    let end = start + 604_800u64;

    client.create_beneficiary_auction(&vault_id, &owner, &start, &end, &10_000u32, &100_000i128);

    // Try to finalize before end time
    env.ledger().set_timestamp((end - 1) as u64);
    let result = client.try_finalize_beneficiary_auction(&vault_id);
    assert!(result.is_err());

    // At end time boundary
    env.ledger().set_timestamp(end as u64);
    let result = client.try_finalize_beneficiary_auction(&vault_id);
    assert!(result.is_err()); // Still within auction (exclusive end)

    // After end time
    env.ledger().set_timestamp((end + 1) as u64);
    let result = client.try_finalize_beneficiary_auction(&vault_id);
    assert!(result.is_ok());
}
