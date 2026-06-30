#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{self, StellarAssetClient},
    vec, Address, BytesN, Env,
};

fn setup_lifecycle() -> (
    Env,
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
    let token_address = env.register_stellar_asset_contract_v2(token_admin).address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, beneficiary, token_address, client)
}

/// Full lifecycle: create → deposit → multi-check-in → TTL expiry → trigger_release → verify balance
#[test]
fn test_full_lifecycle_single_beneficiary() {
    let (env, owner, beneficiary, token_address, client) = setup_lifecycle();
    let interval = 1_000u64;

    // 1. Create vault
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    assert_eq!(client.get_vault(&vault_id).balance, 0);

    // 2. Deposit
    client.deposit(&vault_id, &owner, &500_000i128);
    assert_eq!(client.get_vault(&vault_id).balance, 500_000);

    // 3. Multiple check-ins keep vault alive
    env.ledger().with_mut(|l| l.timestamp = interval - 1);
    client.check_in(&vault_id, &owner, &BytesN::from_array(&env, &[1u8; 32]), &0u64);
    env.ledger().with_mut(|l| l.timestamp += interval - 1);
    client.check_in(&vault_id, &owner, &BytesN::from_array(&env, &[1u8; 32]), &0u64);
    env.ledger().with_mut(|l| l.timestamp += interval - 1);
    client.check_in(&vault_id, &owner, &BytesN::from_array(&env, &[1u8; 32]), &0u64);
    assert!(!client.is_expired(&vault_id));

    // 4. Miss a check-in → vault expires
    env.ledger().with_mut(|l| l.timestamp += interval + 1);
    assert!(client.is_expired(&vault_id));

    // 5. Trigger release
    let token_client = token::Client::new(&env, &token_address);
    let balance_before = token_client.balance(&beneficiary);
    client.trigger_release(&vault_id);

    // 6. Beneficiary receives exact deposit amount
    assert_eq!(token_client.balance(&beneficiary) - balance_before, 500_000);
    assert_eq!(client.get_vault(&vault_id).balance, 0);
}

/// Full lifecycle with multi-beneficiary BPS split: verify exact per-beneficiary amounts
#[test]
fn test_full_lifecycle_multi_beneficiary_bps_split() {
    let (env, owner, b1, token_address, client) = setup_lifecycle();
    let b2 = Address::generate(&env);
    let interval = 500u64;
    let deposit_amount = 10_000i128;

    // 1. Create vault
    let vault_id = client.create_vault(&owner, &b1, &interval, &None);

    // 2. Set 70/30 BPS split
    client.set_beneficiaries(
        &vault_id,
        &owner,
        &vec![
            &env,
            BeneficiaryEntry { address: b1.clone(), bps: 7_000, minimum_threshold: 0 },
            BeneficiaryEntry { address: b2.clone(), bps: 3_000, minimum_threshold: 0 },
        ],
    );

    // 3. Verify BPS invariant
    let stored = client.get_vault(&vault_id).beneficiaries;
    assert_eq!(stored.iter().map(|e| e.bps).sum::<u32>(), 10_000);

    // 4. Deposit
    client.deposit(&vault_id, &owner, &deposit_amount);

    // 5. Check-in once, then expire
    env.ledger().with_mut(|l| l.timestamp = interval - 1);
    client.check_in(&vault_id, &owner, &BytesN::from_array(&env, &[1u8; 32]), &0u64);
    env.ledger().with_mut(|l| l.timestamp += interval + 1);
    assert!(client.is_expired(&vault_id));

    // 6. Release and verify per-beneficiary balances
    let token_client = token::Client::new(&env, &token_address);
    let b1_before = token_client.balance(&b1);
    let b2_before = token_client.balance(&b2);

    client.trigger_release(&vault_id);

    let b1_received = token_client.balance(&b1) - b1_before;
    let b2_received = token_client.balance(&b2) - b2_before;

    assert_eq!(b1_received + b2_received, deposit_amount);
    assert_eq!(b1_received, 7_000);  // 70% of 10_000
    assert_eq!(b2_received, 3_000);  // 30% of 10_000
}

/// Full lifecycle with hibernation: vault survives interval during hibernation, expires after
#[test]
fn test_full_lifecycle_with_hibernation() {
    let (env, owner, beneficiary, token_address, client) = setup_lifecycle();
    let interval = 1_000u64;
    let hibernation = 5_000u64;

    // 1. Create vault and deposit
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&vault_id, &owner, &200_000i128);

    // 2. Enter hibernation before interval expires
    env.ledger().with_mut(|l| l.timestamp = interval / 2);
    client.enter_hibernation(&vault_id, &owner, &hibernation).unwrap();

    // 3. Advance past normal interval — must NOT expire during hibernation
    env.ledger().with_mut(|l| l.timestamp += interval + 1);
    assert!(!client.is_expired(&vault_id), "vault must not expire while hibernating");

    // 4. Exit hibernation
    client.exit_hibernation(&vault_id, &owner).unwrap();

    // 5. Advance well past all intervals → vault now expires
    env.ledger().with_mut(|l| l.timestamp += hibernation + interval + 1);
    assert!(client.is_expired(&vault_id));

    // 6. Release and verify beneficiary receives funds
    let token_client = token::Client::new(&env, &token_address);
    let before = token_client.balance(&beneficiary);
    client.trigger_release(&vault_id);
    assert_eq!(token_client.balance(&beneficiary) - before, 200_000);
}

// --- Vault Partial Liquidation Before Release ---

/// Owner can partially liquidate up to the percentage cap, balance is debited correctly
#[test]
fn test_partial_liquidate_within_limit_succeeds() {
    let (env, owner, beneficiary, _token_address, client) = setup_lifecycle();
    let interval = 10_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&vault_id, &owner, &1_000_000i128);

    // Liquidate 25% = 250_000
    client.partial_liquidate(&vault_id, &250_000i128, &25u32).unwrap();
    assert_eq!(client.get_vault(&vault_id).balance, 750_000);

    // Second liquidation at 10% bound against new balance: 75_000
    client.partial_liquidate(&vault_id, &75_000i128, &10u32).unwrap();
    assert_eq!(client.get_vault(&vault_id).balance, 675_000);
}

/// Owner requesting more than the percentage cap is rejected
#[test]
fn test_partial_liquidate_exceeds_limit_rejected() {
    let (env, owner, beneficiary, _token_address, client) = setup_lifecycle();
    let interval = 10_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&vault_id, &owner, &1_000_000i128);

    // 11% of 1_000_000 = 110_000; asking 120_000 should be rejected
    let err = client
        .try_partial_liquidate(&vault_id, &120_000i128, &11u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::LiquidationExceedsLimit);

    // Balance and TTL untouched on rejected call
    assert_eq!(client.get_vault(&vault_id).balance, 1_000_000);
}

/// TTL countdown is NOT extended by partial liquidation
#[test]
fn test_partial_liquidate_preserves_ttl() {
    let (env, owner, beneficiary, _token_address, client) = setup_lifecycle();
    let interval = 10_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&vault_id, &owner, &1_000_000i128);

    // Move time forward to be partway through the interval.
    env.ledger().with_mut(|l| l.timestamp = 8_000);

    let ttl_before = client.get_ttl_remaining(&vault_id).unwrap();
    assert_eq!(ttl_before, interval - 8_000);

    // Liquidate well within the bounds (1%).
    client.partial_liquidate(&vault_id, &5_000i128, &1u32).unwrap();

    // TTL remaining must be identical after liquidation — last_check_in unchanged.
    let ttl_after = client.get_ttl_remaining(&vault_id).unwrap();
    assert_eq!(
        ttl_after, ttl_before,
        "partial_liquidate must not extend TTL"
    );

    // Balance was correctly debited.
    assert_eq!(client.get_vault(&vault_id).balance, 995_000);
}

/// A regular `check_in` still resets TTL after partial liquidation (no interaction)
#[test]
fn test_partial_liquidate_then_check_in_resets_ttl() {
    let (env, owner, beneficiary, _token_address, client) = setup_lifecycle();
    let interval = 10_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&vault_id, &owner, &1_000_000i128);

    env.ledger().with_mut(|l| l.timestamp = 9_000);
    client.partial_liquidate(&vault_id, &50_000i128, &5u32).unwrap();

    // Now check-in as normal and confirm TTL extends.
    let passkey = BytesN::from_array(&env, &[0u8; 32]);
    client.check_in(&vault_id, &owner, &passkey).unwrap();
    let ttl_after_checkin = client.get_ttl_remaining(&vault_id).unwrap();
    assert_eq!(ttl_after_checkin, interval);
}

/// percentage_allowed = 0 is rejected; percentage_allowed > 100 is rejected
#[test]
fn test_partial_liquidate_invalid_percentage_rejected() {
    let (env, owner, beneficiary, _token_address, client) = setup_lifecycle();
    let interval = 10_000u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&vault_id, &owner, &1_000_000i128);

    let err_zero = client
        .try_partial_liquidate(&vault_id, &1i128, &0u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err_zero, ContractError::InvalidPercentage);

    let err_too_large = client
        .try_partial_liquidate(&vault_id, &1i128, &101u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err_too_large, ContractError::InvalidPercentage);

    // Balance unchanged.
    assert_eq!(client.get_vault(&vault_id).balance, 1_000_000);
}
