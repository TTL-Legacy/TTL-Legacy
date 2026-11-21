#![cfg(test)]

extern crate alloc;

use super::*;
use alloc::vec::Vec as StdVec;
use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env,
};

const INTERVAL: u64 = 3_600;

/// Scales relative weights to basis points summing to exactly 10_000
/// (remainder goes to the first entry).
fn weights_to_bps(weights: &[u32]) -> StdVec<u32> {
    let total: u32 = weights.iter().sum();
    let mut bps: StdVec<u32> = weights.iter().map(|w| w * 10_000 / total).collect();
    let assigned: u32 = bps.iter().sum();
    bps[0] += 10_000 - assigned;
    bps
}

/// Mirrors `distribute_release_amount`: declined beneficiaries are skipped and
/// the remaining beneficiaries split the amount pro rata, with the first
/// qualifying beneficiary absorbing rounding dust.
fn expected_payouts(amount: i128, bps: &[u32], declined: &[bool]) -> StdVec<i128> {
    let qualifying: StdVec<usize> = (0..bps.len()).filter(|&i| !declined[i]).collect();
    let mut payouts = alloc::vec![0i128; bps.len()];
    if qualifying.is_empty() {
        return payouts;
    }
    let total_bps: i128 = qualifying.iter().map(|&i| bps[i] as i128).sum();
    let mut distributed = 0i128;
    for &i in qualifying.iter().skip(1) {
        let share = (amount / total_bps) * bps[i] as i128
            + ((amount % total_bps) * bps[i] as i128) / total_bps;
        payouts[i] = share;
        distributed += share;
    }
    payouts[qualifying[0]] = amount - distributed;
    payouts
}

fn run_waterfall(
    amount: i128,
    weights: &[u32],
    declined: &[bool],
) -> (StdVec<i128>, StdVec<u32>, i128) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &amount);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let bps = weights_to_bps(weights);
    let addresses: StdVec<Address> = bps.iter().map(|_| Address::generate(&env)).collect();
    let mut entries = soroban_sdk::Vec::new(&env);
    for (address, b) in addresses.iter().zip(bps.iter()) {
        entries.push_back(BeneficiaryEntry {
            address: address.clone(),
            bps: *b,
            minimum_threshold: 0,
        });
    }

    let vault_id = client.create_vault(&owner, &addresses[0], &INTERVAL, &None);
    client.set_beneficiaries(&vault_id, &owner, &entries);
    client.deposit(&vault_id, &owner, &amount);

    for (address, is_declined) in addresses.iter().zip(declined.iter()) {
        if *is_declined {
            client.decline_beneficiary_role(&vault_id, address);
        }
    }

    env.ledger().with_mut(|l| l.timestamp += INTERVAL * 2);
    client.trigger_release(&vault_id);

    let token = TokenClient::new(&env, &token_address);
    let balances = addresses.iter().map(|a| token.balance(a)).collect();
    let owner_balance = token.balance(&owner);
    (balances, bps, owner_balance)
}

fn arb_scenario() -> impl Strategy<Value = (i128, StdVec<u32>, StdVec<bool>)> {
    (2usize..=20).prop_flat_map(|n| {
        (
            1i128..1_000_000_000,
            prop::collection::vec(1u32..=100, n),
            prop::collection::vec(any::<bool>(), n),
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Funds are fully conserved: every stroop goes either to a non-declined
    /// beneficiary or, when all decline, back to the owner.
    #[test]
    fn prop_waterfall_conserves_funds((amount, weights, declined) in arb_scenario()) {
        let (balances, _, owner_balance) = run_waterfall(amount, &weights, &declined);
        let paid: i128 = balances.iter().sum();
        prop_assert_eq!(paid + owner_balance, amount);
    }

    /// Declined beneficiaries never receive funds, and the waterfall pays
    /// lower-priority beneficiaries the rebalanced pro-rata share.
    #[test]
    fn prop_waterfall_redistributes_to_non_declined((amount, weights, declined) in arb_scenario()) {
        let (balances, bps, owner_balance) = run_waterfall(amount, &weights, &declined);
        let expected = expected_payouts(amount, &bps, &declined);

        for i in 0..balances.len() {
            if declined[i] {
                prop_assert_eq!(balances[i], 0, "declined beneficiary {} was paid", i);
            }
            prop_assert_eq!(balances[i], expected[i], "payout mismatch at index {}", i);
        }

        if declined.iter().all(|d| *d) {
            prop_assert_eq!(owner_balance, amount);
        } else {
            prop_assert_eq!(owner_balance, 0);
        }
    }
}
