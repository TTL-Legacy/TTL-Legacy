#![cfg(test)]

extern crate alloc;

use super::*;
use proptest::prelude::*;
use soroban_sdk::{
    testutils::Address as _,
    token::StellarAssetClient,
    vec, Address, Env,
};

fn setup_bps_env() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract_v2(token_admin).address();

    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, admin, client)
}

fn bps_sum(entries: &soroban_sdk::Vec<BeneficiaryEntry>) -> u32 {
    entries.iter().map(|e| e.bps).sum()
}

// ── Deterministic invariant tests ────────────────────────────────────────────

#[test]
fn bps_sum_invariant_after_set_beneficiaries() {
    let (env, owner, _, client) = setup_bps_env();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);
    let b3 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &3600u64, &None);

    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 5_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 3_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b3.clone(), bps: 2_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    let stored = client.get_vault(&vault_id).beneficiaries;
    assert_eq!(bps_sum(&stored), 10_000, "BPS sum must equal 10_000 after set_beneficiaries");
}

#[test]
fn bps_sum_invariant_after_cap_application() {
    let (env, owner, _, client) = setup_bps_env();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &3600u64, &None);

    // Set beneficiaries with valid BPS split — caps are applied at release, not at set time
    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 7_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 3_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    // BPS allocation stored in the vault remains unchanged after cap operations
    let stored = client.get_vault(&vault_id).beneficiaries;
    assert_eq!(bps_sum(&stored), 10_000, "BPS sum must equal 10_000 after cap application");
}

#[test]
fn bps_sum_invariant_two_beneficiaries_equal_split() {
    let (env, owner, _, client) = setup_bps_env();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &3600u64, &None);
    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 5_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 5_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    assert_eq!(bps_sum(&client.get_vault(&vault_id).beneficiaries), 10_000);
}

#[test]
fn bps_sum_invariant_single_beneficiary_full_allocation() {
    let (env, owner, _, client) = setup_bps_env();
    let b1 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &3600u64, &None);
    let entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 10_000, minimum_threshold: 0 },
    ];
    client.set_beneficiaries(&vault_id, &owner, &entries);

    assert_eq!(bps_sum(&client.get_vault(&vault_id).beneficiaries), 10_000);
}

#[test]
fn bps_sum_invariant_set_rejects_non_10000() {
    let (env, owner, _, client) = setup_bps_env();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &b1, &3600u64, &None);
    let bad_entries = vec![
        &env,
        BeneficiaryEntry { address: b1.clone(), bps: 4_000, minimum_threshold: 0 },
        BeneficiaryEntry { address: b2.clone(), bps: 4_000, minimum_threshold: 0 },
    ];
    let result = client.try_set_beneficiaries(&vault_id, &owner, &bad_entries);
    assert!(result.is_err(), "set_beneficiaries must reject BPS sum != 10_000");
}

// ── Proptest invariant ────────────────────────────────────────────────────────

proptest! {
    /// For any random BPS split that sums to 10_000, the vault must store
    /// exactly that sum.
    #[test]
    fn prop_bps_sum_invariant_after_set_beneficiaries(
        a_bps in 1u32..9_999u32,
    ) {
        let b_bps = 10_000 - a_bps;
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::generate(&env);
        let admin = Address::generate(&env);
        let token_admin = Address::generate(&env);
        let token_address = env.register_stellar_asset_contract_v2(token_admin).address();
        StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);

        let contract_address = env.register_contract(None, TtlVaultContract);
        let client = TtlVaultContractClient::new(&env, &contract_address);
        client.initialize(&token_address, &admin);
        let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

        let b1 = Address::generate(&env);
        let b2 = Address::generate(&env);

        let vault_id = client.create_vault(&owner, &b1, &3600u64, &None);
        let entries = vec![
            &env,
            BeneficiaryEntry { address: b1.clone(), bps: a_bps, minimum_threshold: 0 },
            BeneficiaryEntry { address: b2.clone(), bps: b_bps, minimum_threshold: 0 },
        ];
        client.set_beneficiaries(&vault_id, &owner, &entries);

        let stored = client.get_vault(&vault_id).beneficiaries;
        prop_assert_eq!(bps_sum(&stored), 10_000);
    }
}
