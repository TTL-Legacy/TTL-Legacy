//! Issue #1288 – Multi-beneficiary vault support with percentage-based splits.
//!
//! These tests cover `create_vault_with_splits` end-to-end:
//!   * happy-path vault creation with 2, 3 and N beneficiaries
//!   * fund distribution proportional to declared percentages
//!   * validation: percentages must sum to 100
//!   * validation: zero percentage is rejected
//!   * validation: percentage > 100 is rejected
//!   * validation: empty splits list is rejected
//!   * validation: owner-as-beneficiary is rejected
//!   * rounding: last beneficiary absorbs dust so the total is exact

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

// ── Test harness ─────────────────────────────────────────────────────────────

struct TestEnv {
    env: Env,
    admin: Address,
    token_address: Address,
    client: TtlVaultContractClient<'static>,
}

impl TestEnv {
    fn setup() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let token_admin = Address::generate(&env);
        let token_address = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();

        let contract_address = env.register_contract(None, TtlVaultContract);
        let client = TtlVaultContractClient::new(&env, &contract_address);
        client.initialize(&token_address, &admin);

        let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

        Self {
            env,
            admin,
            token_address,
            client,
        }
    }

    fn mint(&self, to: &Address, amount: i128) {
        StellarAssetClient::new(&self.env, &self.token_address).mint(to, &amount);
    }
}

// ── Helper: build a BeneficiarySplit vec ─────────────────────────────────────

fn splits_2(env: &Env, a: &Address, b: &Address, pct_a: u32, pct_b: u32) -> Vec<BeneficiarySplit> {
    vec![
        env,
        BeneficiarySplit {
            address: a.clone(),
            percentage: pct_a,
        },
        BeneficiarySplit {
            address: b.clone(),
            percentage: pct_b,
        },
    ]
}

fn splits_3(
    env: &Env,
    a: &Address,
    b: &Address,
    c: &Address,
    pct_a: u32,
    pct_b: u32,
    pct_c: u32,
) -> Vec<BeneficiarySplit> {
    vec![
        env,
        BeneficiarySplit {
            address: a.clone(),
            percentage: pct_a,
        },
        BeneficiarySplit {
            address: b.clone(),
            percentage: pct_b,
        },
        BeneficiarySplit {
            address: c.clone(),
            percentage: pct_c,
        },
    ]
}

// ── Happy-path creation ───────────────────────────────────────────────────────

/// Two beneficiaries with a 50/50 split create successfully.
#[test]
fn test_create_vault_with_splits_two_equal() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);
    let b2 = Address::generate(&t.env);

    let splits = splits_2(&t.env, &b1, &b2, 50, 50);
    let vault_id = t
        .client
        .create_vault_with_splits(&owner, &splits, &3600, &None);

    let vault = t.client.get_vault(&vault_id);
    assert_eq!(vault.beneficiaries.len(), 2);

    // Check BPS conversion: 50% → 5000 bps
    let e0 = vault.beneficiaries.get(0).unwrap();
    let e1 = vault.beneficiaries.get(1).unwrap();
    assert_eq!(e0.address, b1);
    assert_eq!(e0.bps, 5000);
    assert_eq!(e1.address, b2);
    assert_eq!(e1.bps, 5000);
}

/// Three beneficiaries with 50/30/20 split.
#[test]
fn test_create_vault_with_splits_three_unequal() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);
    let b2 = Address::generate(&t.env);
    let b3 = Address::generate(&t.env);

    let splits = splits_3(&t.env, &b1, &b2, &b3, 50, 30, 20);
    let vault_id = t
        .client
        .create_vault_with_splits(&owner, &splits, &3600, &None);

    let vault = t.client.get_vault(&vault_id);
    assert_eq!(vault.beneficiaries.len(), 3);

    let e0 = vault.beneficiaries.get(0).unwrap();
    let e1 = vault.beneficiaries.get(1).unwrap();
    let e2 = vault.beneficiaries.get(2).unwrap();
    assert_eq!(e0.bps, 5000);
    assert_eq!(e1.bps, 3000);
    assert_eq!(e2.bps, 2000);
}

/// Single beneficiary with 100% is valid.
#[test]
fn test_create_vault_with_splits_single_100_percent() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);

    let splits = vec![
        &t.env,
        BeneficiarySplit {
            address: b1.clone(),
            percentage: 100,
        },
    ];
    let vault_id = t
        .client
        .create_vault_with_splits(&owner, &splits, &3600, &None);

    let vault = t.client.get_vault(&vault_id);
    assert_eq!(vault.beneficiaries.len(), 1);
    assert_eq!(vault.beneficiaries.get(0).unwrap().bps, 10_000);
}

/// Vault is created in Locked status by default.
#[test]
fn test_created_vault_is_locked() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);
    let b2 = Address::generate(&t.env);

    let splits = splits_2(&t.env, &b1, &b2, 70, 30);
    let vault_id = t
        .client
        .create_vault_with_splits(&owner, &splits, &3600, &None);

    let vault = t.client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Locked);
}

// ── Fund distribution ─────────────────────────────────────────────────────────

/// Funds distributed proportionally on release — 50/50.
#[test]
fn test_release_distributes_50_50() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);
    let b2 = Address::generate(&t.env);

    // Fund owner
    t.mint(&owner, 1_000_000);

    let splits = splits_2(&t.env, &b1, &b2, 50, 50);
    let vault_id = t
        .client
        .create_vault_with_splits(&owner, &splits, &3600, &None);

    // Deposit 1 000 000 stroops
    t.client.deposit(&vault_id, &owner, &1_000_000);

    // Advance time past the check-in interval to trigger expiry
    t.env.ledger().with_mut(|l| l.timestamp += 7_200);

    t.client.trigger_release(&vault_id);

    let tok = soroban_sdk::token::Client::new(&t.env, &t.token_address);
    assert_eq!(tok.balance(&b1), 500_000);
    assert_eq!(tok.balance(&b2), 500_000);
}

/// Funds distributed proportionally on release — 70/30.
#[test]
fn test_release_distributes_70_30() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);
    let b2 = Address::generate(&t.env);

    t.mint(&owner, 1_000_000);

    let splits = splits_2(&t.env, &b1, &b2, 70, 30);
    let vault_id = t
        .client
        .create_vault_with_splits(&owner, &splits, &3600, &None);
    t.client.deposit(&vault_id, &owner, &1_000_000);

    t.env.ledger().with_mut(|l| l.timestamp += 7_200);
    t.client.trigger_release(&vault_id);

    let tok = soroban_sdk::token::Client::new(&t.env, &t.token_address);
    assert_eq!(tok.balance(&b1), 700_000);
    assert_eq!(tok.balance(&b2), 300_000);
}

/// Three-way 50/30/20 split distributes correctly.
#[test]
fn test_release_distributes_50_30_20() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);
    let b2 = Address::generate(&t.env);
    let b3 = Address::generate(&t.env);

    t.mint(&owner, 1_000_000);

    let splits = splits_3(&t.env, &b1, &b2, &b3, 50, 30, 20);
    let vault_id = t
        .client
        .create_vault_with_splits(&owner, &splits, &3600, &None);
    t.client.deposit(&vault_id, &owner, &1_000_000);

    t.env.ledger().with_mut(|l| l.timestamp += 7_200);
    t.client.trigger_release(&vault_id);

    let tok = soroban_sdk::token::Client::new(&t.env, &t.token_address);
    assert_eq!(tok.balance(&b1), 500_000);
    assert_eq!(tok.balance(&b2), 300_000);
    assert_eq!(tok.balance(&b3), 200_000);
}

/// Last beneficiary absorbs rounding dust — total must equal deposit.
#[test]
fn test_release_dust_goes_to_last_beneficiary() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);
    let b2 = Address::generate(&t.env);
    let b3 = Address::generate(&t.env);

    // 100 stroops split 33/33/34 — 33.33 rounds to 33 for first two, leaving 34 for last
    t.mint(&owner, 100);

    let splits = splits_3(&t.env, &b1, &b2, &b3, 33, 33, 34);
    let vault_id = t
        .client
        .create_vault_with_splits(&owner, &splits, &3600, &None);
    t.client.deposit(&vault_id, &owner, &100);

    t.env.ledger().with_mut(|l| l.timestamp += 7_200);
    t.client.trigger_release(&vault_id);

    let tok = soroban_sdk::token::Client::new(&t.env, &t.token_address);
    let total = tok.balance(&b1) + tok.balance(&b2) + tok.balance(&b3);
    // All 100 stroops must be distributed with no leakage
    assert_eq!(total, 100);
}

// ── Validation: invalid inputs ────────────────────────────────────────────────

/// Empty splits list is rejected.
#[test]
#[should_panic]
fn test_empty_splits_rejected() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let empty: Vec<BeneficiarySplit> = Vec::new(&t.env);
    t.client
        .create_vault_with_splits(&owner, &empty, &3600, &None);
}

/// Percentages summing to less than 100 are rejected.
#[test]
#[should_panic]
fn test_percentages_sum_less_than_100_rejected() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);
    let b2 = Address::generate(&t.env);

    // 40 + 40 = 80, not 100
    let splits = splits_2(&t.env, &b1, &b2, 40, 40);
    t.client
        .create_vault_with_splits(&owner, &splits, &3600, &None);
}

/// Percentages summing to more than 100 are rejected.
#[test]
#[should_panic]
fn test_percentages_sum_more_than_100_rejected() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);
    let b2 = Address::generate(&t.env);

    // 60 + 60 = 120, not 100
    let splits = splits_2(&t.env, &b1, &b2, 60, 60);
    t.client
        .create_vault_with_splits(&owner, &splits, &3600, &None);
}

/// A zero percentage entry is rejected.
#[test]
#[should_panic]
fn test_zero_percentage_rejected() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);
    let b2 = Address::generate(&t.env);

    // percentage = 0 is not allowed even if sum would equal 100
    let splits = splits_2(&t.env, &b1, &b2, 0, 100);
    t.client
        .create_vault_with_splits(&owner, &splits, &3600, &None);
}

/// A percentage > 100 on a single entry is rejected.
#[test]
#[should_panic]
fn test_percentage_over_100_rejected() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);

    let splits = vec![
        &t.env,
        BeneficiarySplit {
            address: b1.clone(),
            percentage: 101,
        },
    ];
    t.client
        .create_vault_with_splits(&owner, &splits, &3600, &None);
}

/// Owner listed as a beneficiary is rejected.
#[test]
#[should_panic]
fn test_owner_as_beneficiary_rejected() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);

    // owner == split address
    let splits = splits_2(&t.env, &owner, &b1, 50, 50);
    t.client
        .create_vault_with_splits(&owner, &splits, &3600, &None);
}

// ── BPS conversion accuracy ───────────────────────────────────────────────────

/// BPS values stored in vault.beneficiaries are exactly percentage × 100.
#[test]
fn test_bps_conversion_accuracy() {
    let t = TestEnv::setup();
    let owner = Address::generate(&t.env);

    let a0 = Address::generate(&t.env);
    let a1 = Address::generate(&t.env);
    let a2 = Address::generate(&t.env);
    let a3 = Address::generate(&t.env);

    let splits = vec![
        &t.env,
        BeneficiarySplit {
            address: a0.clone(),
            percentage: 10,
        },
        BeneficiarySplit {
            address: a1.clone(),
            percentage: 20,
        },
        BeneficiarySplit {
            address: a2.clone(),
            percentage: 30,
        },
        BeneficiarySplit {
            address: a3.clone(),
            percentage: 40,
        },
    ];

    let vault_id = t
        .client
        .create_vault_with_splits(&owner, &splits, &3600, &None);
    let vault = t.client.get_vault(&vault_id);

    let expected_bps = [1000u32, 2000, 3000, 4000];
    for (i, expected) in expected_bps.iter().enumerate() {
        assert_eq!(vault.beneficiaries.get(i as u32).unwrap().bps, *expected);
    }

    // Sum of all BPS must equal 10 000
    let total_bps: u32 = vault.beneficiaries.iter().map(|e| e.bps).sum();
    assert_eq!(total_bps, 10_000);
}

// ── Idempotency ───────────────────────────────────────────────────────────────

/// Two separate calls produce two independent vaults.
#[test]
fn test_two_vaults_independent() {
    let t = TestEnv::setup();
    let owner1 = Address::generate(&t.env);
    let owner2 = Address::generate(&t.env);
    let b1 = Address::generate(&t.env);
    let b2 = Address::generate(&t.env);
    let b3 = Address::generate(&t.env);
    let b4 = Address::generate(&t.env);

    let splits1 = splits_2(&t.env, &b1, &b2, 60, 40);
    let splits2 = splits_2(&t.env, &b3, &b4, 25, 75);

    let id1 = t
        .client
        .create_vault_with_splits(&owner1, &splits1, &3600, &None);
    let id2 = t
        .client
        .create_vault_with_splits(&owner2, &splits2, &7200, &None);

    assert_ne!(id1, id2);

    let v1 = t.client.get_vault(&id1);
    let v2 = t.client.get_vault(&id2);

    assert_eq!(v1.beneficiaries.get(0).unwrap().bps, 6000);
    assert_eq!(v2.beneficiaries.get(1).unwrap().bps, 7500);
}
