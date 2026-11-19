#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

fn setup_vesting() -> (Env, Address, Address, u64, TtlVaultContractClient<'static>) {
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
    client.deposit(&vault_id, &owner, &4_000_000);

    (env, owner, beneficiary, vault_id, client)
}

// ========== Beneficiary Vesting Edge Cases ==========

#[test]
fn test_vesting_with_multiple_beneficiaries_different_schedules() {
    let (env, owner, _, vault_id, client) = setup_vesting();

    let ben_a = Address::generate(&env);
    let ben_b = Address::generate(&env);

    let start = env.ledger().timestamp() + 100;

    // Different vesting schedules for different beneficiaries
    let result_a = client
        .try_set_beneficiary_vesting(&vault_id, &owner, &ben_a, &start, &86_400u64, &4u32, &0u64);
    let result_b = client.try_set_beneficiary_vesting(
        &vault_id,
        &owner,
        &ben_b,
        &(start + 172_800u64), // 2 days later
        &172_800u64,           // 2-day interval
        &2u32,
        &0u64,
    );

    assert!(result_a.is_ok());
    assert!(result_b.is_ok());

    let sched_a = client.get_beneficiary_vesting(&vault_id, &ben_a).unwrap();
    let sched_b = client.get_beneficiary_vesting(&vault_id, &ben_b).unwrap();

    assert_eq!(sched_a.start_time, start);
    assert_eq!(sched_b.start_time, start + 172_800u64);
    assert_eq!(sched_a.interval, 86_400u64);
    assert_eq!(sched_b.interval, 172_800u64);
}

#[test]
fn test_vesting_schedule_can_be_updated() {
    let (env, owner, beneficiary, vault_id, client) = setup_vesting();

    let start = env.ledger().timestamp() + 100;

    // Set initial schedule
    client.set_beneficiary_vesting(
        &vault_id,
        &owner,
        &beneficiary,
        &start,
        &86_400u64,
        &4u32,
        &0u64,
    );

    let initial = client
        .get_beneficiary_vesting(&vault_id, &beneficiary)
        .unwrap();
    assert_eq!(initial.num_installments, 4u32);

    // Update to new schedule
    client.set_beneficiary_vesting(
        &vault_id,
        &owner,
        &beneficiary,
        &start,
        &86_400u64,
        &6u32,
        &0u64,
    );

    let updated = client
        .get_beneficiary_vesting(&vault_id, &beneficiary)
        .unwrap();
    assert_eq!(updated.num_installments, 6u32);
    assert_eq!(updated.claimed_installments, 0u32); // Reset on update
}

#[test]
fn test_vesting_cliff_prevents_early_claim() {
    let (env, owner, beneficiary, vault_id, client) = setup_vesting();

    let start = 1000u64;
    env.ledger().set_timestamp(start);

    let cliff = 31_536_000u64; // 1 year

    client.set_beneficiary_vesting(
        &vault_id,
        &owner,
        &beneficiary,
        &start,
        &86_400u64,
        &4u32,
        &cliff,
    );

    client.trigger_release(&vault_id);

    // Try to claim before cliff reached
    env.ledger().set_timestamp(start + 86_400);
    let result = client.try_claim_beneficiary_vesting(&vault_id, &beneficiary);
    assert!(result.is_err());

    // Move past cliff
    env.ledger().set_timestamp(start + cliff + 86_400);
    let result = client.try_claim_beneficiary_vesting(&vault_id, &beneficiary);
    assert!(result.is_ok());
}

#[test]
fn test_vesting_with_uneven_amounts() {
    let (env, owner, beneficiary, vault_id, client) = setup_vesting();

    // 1,000,001 stroops / 3 installments = 333,333 + 333,333 + 333_335
    let amount = 1_000_001i128;
    client.deposit(&vault_id, &owner, &(amount - 4_000_000));

    let start = 100u64;
    env.ledger().set_timestamp(start);

    client.set_beneficiary_vesting(&vault_id, &owner, &beneficiary, &start, &1u64, &3u32, &0u64);

    client.trigger_release(&vault_id);

    // First claim
    env.ledger().set_timestamp(start + 2);
    let amount_1 = client
        .claim_beneficiary_vesting(&vault_id, &beneficiary)
        .unwrap();

    // Last installment should include remainder
    env.ledger().set_timestamp(start + 4);
    let amount_2 = client
        .claim_beneficiary_vesting(&vault_id, &beneficiary)
        .unwrap();

    // Last installment (claimed_installments = 3)
    env.ledger().set_timestamp(start + 6);
    let amount_3 = client
        .claim_beneficiary_vesting(&vault_id, &beneficiary)
        .unwrap();

    let total = amount_1 + amount_2 + amount_3;
    assert_eq!(total, amount);
}

#[test]
fn test_vesting_requires_vault_released() {
    let (env, owner, beneficiary, vault_id, client) = setup_vesting();

    let start = env.ledger().timestamp() + 100;
    client.set_beneficiary_vesting(
        &vault_id,
        &owner,
        &beneficiary,
        &start,
        &86_400u64,
        &4u32,
        &0u64,
    );

    // Try to claim before vault is released
    env.ledger().set_timestamp(start + 86_400 + 10);
    let result = client.try_claim_beneficiary_vesting(&vault_id, &beneficiary);

    assert!(result.is_err());
}

#[test]
fn test_vesting_nonexistent_schedule() {
    let (env, _, _, vault_id, client) = setup_vesting();

    let nonexistent = Address::generate(&env);
    client.trigger_release(&vault_id);

    let result = client.try_claim_beneficiary_vesting(&vault_id, &nonexistent);
    assert!(result.is_err());
}

#[test]
fn test_vesting_claim_sequence() {
    let (env, owner, beneficiary, vault_id, client) = setup_vesting();

    let start = 100u64;
    env.ledger().set_timestamp(start);

    client.set_beneficiary_vesting(&vault_id, &owner, &beneficiary, &start, &1u64, &4u32, &0u64);
    client.trigger_release(&vault_id);

    // Per installment = 4_000_000 / 4 = 1_000_000
    let per_install = 1_000_000i128;

    // Claim 1: at timestamp start + 1
    env.ledger().set_timestamp(start + 1);
    let claim_1 = client
        .claim_beneficiary_vesting(&vault_id, &beneficiary)
        .unwrap();
    assert_eq!(claim_1, per_install);

    // Claim 2: at timestamp start + 2
    env.ledger().set_timestamp(start + 2);
    let claim_2 = client
        .claim_beneficiary_vesting(&vault_id, &beneficiary)
        .unwrap();
    assert_eq!(claim_2, per_install);

    // Verify claimed_installments advanced
    let schedule = client
        .get_beneficiary_vesting(&vault_id, &beneficiary)
        .unwrap();
    assert_eq!(schedule.claimed_installments, 2u32);
}

#[test]
fn test_vesting_all_claimed_blocks_further_claims() {
    let (env, owner, beneficiary, vault_id, client) = setup_vesting();

    let start = 100u64;
    env.ledger().set_timestamp(start);

    client.set_beneficiary_vesting(&vault_id, &owner, &beneficiary, &start, &1u64, &2u32, &0u64);
    client.trigger_release(&vault_id);

    // Claim first installment
    env.ledger().set_timestamp(start + 1);
    let _ = client.claim_beneficiary_vesting(&vault_id, &beneficiary);

    // Claim second (final) installment
    env.ledger().set_timestamp(start + 2);
    let _ = client.claim_beneficiary_vesting(&vault_id, &beneficiary);

    // Try to claim again - should fail
    env.ledger().set_timestamp(start + 3);
    let result = client.try_claim_beneficiary_vesting(&vault_id, &beneficiary);
    assert!(result.is_err());
}

#[test]
fn test_vesting_large_amounts() {
    let (env, owner, beneficiary, vault_id, client) = setup_vesting();

    // Deposit a large amount
    let large_amount = 1_000_000_000_000i128; // 10M XLM
    client.deposit(&vault_id, &owner, &(large_amount - 4_000_000));

    let start = 100u64;
    env.ledger().set_timestamp(start);

    client.set_beneficiary_vesting(
        &vault_id,
        &owner,
        &beneficiary,
        &start,
        &86_400u64,
        &100u32,
        &0u64,
    );
    client.trigger_release(&vault_id);

    let per_install = large_amount / 100;

    env.ledger().set_timestamp(start + 86_400);
    let claimed = client
        .claim_beneficiary_vesting(&vault_id, &beneficiary)
        .unwrap();

    assert_eq!(claimed, per_install);
}
