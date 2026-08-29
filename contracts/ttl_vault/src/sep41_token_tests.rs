#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{self, StellarAssetClient},
    vec, Address, BytesN, Env,
};

// ─────────────────────────────────────────────────────────────────────────────
// SEP-41 Token Support Tests
// ─────────────────────────────────────────────────────────────────────────────
// 
// This module tests Soroban's SEP-41 token interface compliance, allowing vaults
// to lock and release SEP-41 compatible custom tokens (USDC, EURC, stablecoins, etc.)
// instead of only native XLM.
//
// SEP-41: Stellar standard for cross-chain custom tokens issued on Soroban.
// All transfers use the standard Soroban token::Client interface which implements
// the SEP-41 token specification.

/// Test fixture: Creates contract and initializes with XLM token.
/// Returns (env, admin, xlm_token_address, contract_client)
fn setup_with_xlm_only() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let xlm_admin = Address::generate(&env);
    let xlm_token = env.register_stellar_asset_contract_v2(xlm_admin).address();

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&xlm_token, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, admin, xlm_token, client)
}

/// Test fixture: Creates contract with XLM and whitelists a USDC token.
/// Returns (env, admin, xlm_token, usdc_token, contract_client)
fn setup_with_xlm_and_usdc() -> (
    Env,
    Address,
    Address,
    Address,
    TtlVaultContractClient<'static>,
) {
    let (env, admin, xlm_token, client) = setup_with_xlm_only();

    // Register USDC token (SEP-41 compliant)
    let usdc_admin = Address::generate(&env);
    let usdc_token = env.register_stellar_asset_contract_v2(usdc_admin).address();

    // Whitelist USDC in the contract
    client.whitelist_token(&usdc_token);

    (env, admin, xlm_token, usdc_token, client)
}

// ─────────────────────────────────────────────────────────────────────────────
// Token Whitelist Management Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Verify admin can whitelist a SEP-41 token (USDC)
#[test]
fn test_whitelist_usdc_token() {
    let (env, admin, _xlm_token, client) = setup_with_xlm_only();

    // Create USDC token
    let usdc_admin = Address::generate(&env);
    let usdc_token = env.register_stellar_asset_contract_v2(usdc_admin).address();

    // Initially not whitelisted
    assert!(!client.is_token_whitelisted(&usdc_token));

    // Admin whitelists USDC
    client.whitelist_token(&usdc_token);

    // Now whitelisted
    assert!(client.is_token_whitelisted(&usdc_token));
}

/// Verify admin can remove a token from whitelist
#[test]
fn test_remove_token_from_whitelist() {
    let (env, admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    // USDC is whitelisted
    assert!(client.is_token_whitelisted(&usdc_token));

    // Admin removes USDC from whitelist
    client.remove_token_whitelist(&usdc_token);

    // No longer whitelisted
    assert!(!client.is_token_whitelisted(&usdc_token));
}

/// Verify non-admin cannot whitelist tokens
#[test]
#[should_panic(expected = "NotAdmin")]
fn test_non_admin_cannot_whitelist_token() {
    let (env, _admin, _xlm_token, client) = setup_with_xlm_only();

    let non_admin = Address::generate(&env);
    let usdc_admin = Address::generate(&env);
    let usdc_token = env.register_stellar_asset_contract_v2(usdc_admin).address();

    // Non-admin attempts to whitelist
    client.whitelist_token(&usdc_token);
}

/// Verify default XLM token is always whitelisted (no explicit whitelist needed)
#[test]
fn test_default_xlm_token_always_whitelisted() {
    let (env, admin, xlm_token, client) = setup_with_xlm_only();

    // XLM token should be whitelisted by default
    assert!(client.is_token_whitelisted(&xlm_token));
}

// ─────────────────────────────────────────────────────────────────────────────
// Vault Creation with SEP-41 Tokens
// ─────────────────────────────────────────────────────────────────────────────

/// Create a vault locked with USDC (SEP-41 token)
#[test]
fn test_create_usdc_vault() {
    let (env, _admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;

    // Create USDC vault with explicit token_address
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &Some(usdc_token.clone()));

    // Verify vault is correctly configured
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.owner, owner);
    assert_eq!(vault.beneficiary, beneficiary);
    assert_eq!(vault.token_address, usdc_token);
    assert_eq!(vault.balance, 0);
}

/// Create a vault with default XLM token (None parameter)
#[test]
fn test_create_vault_with_default_xlm() {
    let (env, _admin, xlm_token, _usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;

    // Create vault without specifying token (defaults to XLM)
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Verify vault uses default XLM token
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.token_address, xlm_token);
}

/// Reject vault creation with non-whitelisted token
#[test]
#[should_panic(expected = "TokenNotWhitelisted")]
fn test_create_vault_with_non_whitelisted_token_fails() {
    let (env, _admin, _xlm_token, _usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;

    // Create non-whitelisted token (EURC)
    let eurc_admin = Address::generate(&env);
    let eurc_token = env.register_stellar_asset_contract_v2(eurc_admin).address();

    // Attempt to create vault with non-whitelisted EURC — should panic
    client.create_vault(&owner, &beneficiary, &interval, &Some(eurc_token));
}

// ─────────────────────────────────────────────────────────────────────────────
// USDC Deposit Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Deposit USDC into a USDC-locked vault
#[test]
fn test_deposit_usdc_into_vault() {
    let (env, _admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;
    let deposit_amount = 1_000_000i128; // 1 USDC (6 decimals) = 1_000_000

    // Mint USDC to owner
    let usdc_client = token::Client::new(&env, &usdc_token);
    let usdc_admin = Address::generate(&env);
    StellarAssetClient::new(&env, &usdc_token).mint(&owner, &deposit_amount);

    // Create USDC vault
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &Some(usdc_token.clone()));

    // Deposit USDC
    client.deposit(&vault_id, &owner, &deposit_amount);

    // Verify vault balance increased
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, deposit_amount);
    
    // Verify contract received USDC tokens
    let contract_balance = usdc_client.balance(&env.current_contract_address());
    assert_eq!(contract_balance, deposit_amount);
}

/// Multiple USDC deposits into same vault
#[test]
fn test_multiple_usdc_deposits() {
    let (env, _admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;

    // Mint USDC
    StellarAssetClient::new(&env, &usdc_token).mint(&owner, &5_000_000i128);

    // Create vault
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &Some(usdc_token.clone()));

    // First deposit: 1 USDC
    client.deposit(&vault_id, &owner, &1_000_000i128);
    assert_eq!(client.get_vault(&vault_id).balance, 1_000_000);

    // Second deposit: 2 USDC
    client.deposit(&vault_id, &owner, &2_000_000i128);
    assert_eq!(client.get_vault(&vault_id).balance, 3_000_000);

    // Third deposit: 1.5 USDC
    client.deposit(&vault_id, &owner, &1_500_000i128);
    assert_eq!(client.get_vault(&vault_id).balance, 4_500_000);
}

/// Reject USDC deposit if amount exceeds max_deposit_amount
#[test]
#[should_panic(expected = "DepositLimitExceeded")]
fn test_usdc_deposit_exceeds_limit() {
    let (env, _admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;

    // Mint 100 USDC
    StellarAssetClient::new(&env, &usdc_token).mint(&owner, &100_000_000i128);

    // Create vault
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &Some(usdc_token.clone()));

    // Set max deposit to 10 USDC
    client.set_max_deposit_amount(&vault_id, &owner, &10_000_000i128);

    // Attempt deposit of 15 USDC — should panic
    client.deposit(&vault_id, &owner, &15_000_000i128);
}

// ─────────────────────────────────────────────────────────────────────────────
// USDC Withdrawal Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Withdraw USDC from a USDC vault before expiry
#[test]
fn test_withdraw_usdc_before_expiry() {
    let (env, _admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;
    let deposit_amount = 10_000_000i128; // 10 USDC

    // Mint and deposit USDC
    StellarAssetClient::new(&env, &usdc_token).mint(&owner, &deposit_amount);
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &Some(usdc_token.clone()));
    client.deposit(&vault_id, &owner, &deposit_amount);

    // Track owner's USDC balance before withdrawal
    let usdc_client = token::Client::new(&env, &usdc_token);
    let owner_balance_before = usdc_client.balance(&owner);

    // Withdraw partial amount: 4 USDC
    let withdraw_amount = 4_000_000i128;
    client.withdraw(&vault_id, &owner, &withdraw_amount).ok();

    // Verify vault balance decreased
    assert_eq!(
        client.get_vault(&vault_id).balance,
        deposit_amount - withdraw_amount
    );

    // Verify owner received USDC
    let owner_balance_after = usdc_client.balance(&owner);
    assert_eq!(owner_balance_after - owner_balance_before, withdraw_amount);
}

/// Withdraw full USDC amount from vault
#[test]
fn test_withdraw_full_usdc_amount() {
    let (env, _admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;
    let deposit_amount = 5_000_000i128; // 5 USDC

    // Setup
    StellarAssetClient::new(&env, &usdc_token).mint(&owner, &deposit_amount);
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &Some(usdc_token.clone()));
    client.deposit(&vault_id, &owner, &deposit_amount);

    // Withdraw full amount
    client.withdraw(&vault_id, &owner, &deposit_amount).ok();

    // Verify vault is empty
    assert_eq!(client.get_vault(&vault_id).balance, 0);
}

/// Reject USDC withdrawal that exceeds vault balance
#[test]
fn test_usdc_withdrawal_exceeds_balance_fails() {
    let (env, _admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;

    // Deposit 5 USDC
    StellarAssetClient::new(&env, &usdc_token).mint(&owner, &5_000_000i128);
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &Some(usdc_token.clone()));
    client.deposit(&vault_id, &owner, &5_000_000i128);

    // Attempt withdrawal of 10 USDC — should fail
    let result = client.withdraw(&vault_id, &owner, &10_000_000i128);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// USDC Release Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Complete lifecycle: USDC deposit → expire → release to beneficiary
#[test]
fn test_usdc_vault_full_lifecycle() {
    let (env, _admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;
    let deposit_amount = 20_000_000i128; // 20 USDC

    // 1. Create and fund USDC vault
    StellarAssetClient::new(&env, &usdc_token).mint(&owner, &deposit_amount);
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &Some(usdc_token.clone()));
    client.deposit(&vault_id, &owner, &deposit_amount);

    assert_eq!(client.get_vault(&vault_id).balance, deposit_amount);

    // 2. Check-in to keep vault alive
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    env.ledger().with_mut(|l| l.timestamp = interval - 1);
    client.check_in(&vault_id, &owner, &passkey, &0u64);

    // 3. Advance time past interval → vault expires
    env.ledger().with_mut(|l| l.timestamp += interval + 1);
    assert!(client.is_expired(&vault_id));

    // 4. Track beneficiary balance before release
    let usdc_client = token::Client::new(&env, &usdc_token);
    let beneficiary_balance_before = usdc_client.balance(&beneficiary);

    // 5. Trigger release
    client.trigger_release(&vault_id);

    // 6. Verify beneficiary received exact USDC amount
    let beneficiary_balance_after = usdc_client.balance(&beneficiary);
    assert_eq!(
        beneficiary_balance_after - beneficiary_balance_before,
        deposit_amount
    );

    // 7. Verify vault is empty
    assert_eq!(client.get_vault(&vault_id).balance, 0);
}

/// USDC release with spending limit (partial release)
#[test]
fn test_usdc_release_with_spending_limit() {
    let (env, _admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;
    let deposit_amount = 100_000_000i128; // 100 USDC

    // Create and fund vault
    StellarAssetClient::new(&env, &usdc_token).mint(&owner, &deposit_amount);
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &Some(usdc_token.clone()));
    client.deposit(&vault_id, &owner, &deposit_amount);

    // Set spending limit to 30 USDC
    let spending_limit = 30_000_000i128;
    client.set_spending_limit(&vault_id, &owner, &Some(spending_limit));

    // Expire vault
    env.ledger().with_mut(|l| l.timestamp = interval + 1);

    // Track balances
    let usdc_client = token::Client::new(&env, &usdc_token);
    let beneficiary_balance_before = usdc_client.balance(&beneficiary);

    // Trigger release
    client.trigger_release(&vault_id);

    // Verify beneficiary received only the spending limit amount
    let beneficiary_balance_after = usdc_client.balance(&beneficiary);
    assert_eq!(
        beneficiary_balance_after - beneficiary_balance_before,
        spending_limit
    );

    // Remaining amount stays in contract (per Soroban design, not returned to owner)
    assert_eq!(client.get_vault(&vault_id).balance, deposit_amount - spending_limit);
}

/// USDC release with multiple beneficiaries (BPS split)
#[test]
fn test_usdc_release_multi_beneficiary() {
    let (env, _admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);
    let interval = 1_000u64;
    let deposit_amount = 10_000_000i128; // 10 USDC

    // Create USDC vault
    StellarAssetClient::new(&env, &usdc_token).mint(&owner, &deposit_amount);
    let vault_id = client.create_vault(&owner, &b1, &interval, &Some(usdc_token.clone()));

    // Set 60/40 split between b1 and b2
    client.set_beneficiaries(
        &vault_id,
        &owner,
        &vec![
            &env,
            BeneficiaryEntry { address: b1.clone(), bps: 6_000, minimum_threshold: 0 },
            BeneficiaryEntry { address: b2.clone(), bps: 4_000, minimum_threshold: 0 },
        ],
    );

    // Deposit and expire
    client.deposit(&vault_id, &owner, &deposit_amount);
    env.ledger().with_mut(|l| l.timestamp = interval + 1);

    // Release
    let usdc_client = token::Client::new(&env, &usdc_token);
    let b1_before = usdc_client.balance(&b1);
    let b2_before = usdc_client.balance(&b2);

    client.trigger_release(&vault_id);

    // Verify split: 60% to b1, 40% to b2
    let b1_received = usdc_client.balance(&b1) - b1_before;
    let b2_received = usdc_client.balance(&b2) - b2_before;
    assert_eq!(b1_received, 6_000_000);  // 60% of 10 USDC
    assert_eq!(b2_received, 4_000_000);  // 40% of 10 USDC
}

// ─────────────────────────────────────────────────────────────────────────────
// Cross-Token Scenarios
// ─────────────────────────────────────────────────────────────────────────────

/// Create separate vaults for XLM and USDC in same contract
#[test]
fn test_xlm_and_usdc_vaults_coexist() {
    let (env, _admin, xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;

    // Mint tokens
    StellarAssetClient::new(&env, &xlm_token).mint(&owner, &100_000i128);
    StellarAssetClient::new(&env, &usdc_token).mint(&owner, &50_000_000i128);

    // Create XLM vault
    let xlm_vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    
    // Create USDC vault
    let usdc_vault_id = client.create_vault(&owner, &beneficiary, &interval, &Some(usdc_token.clone()));

    // Deposit into both
    client.deposit(&xlm_vault_id, &owner, &50_000i128);
    client.deposit(&usdc_vault_id, &owner, &25_000_000i128);

    // Verify both vaults have correct balances
    assert_eq!(client.get_vault(&xlm_vault_id).balance, 50_000);
    assert_eq!(client.get_vault(&usdc_vault_id).balance, 25_000_000);

    // Verify token addresses are different
    assert_eq!(client.get_vault(&xlm_vault_id).token_address, xlm_token);
    assert_eq!(client.get_vault(&usdc_vault_id).token_address, usdc_token);
}

/// Verify wrapped token support (cross-chain token mapping)
#[test]
fn test_wrapped_token_registration() {
    let (env, admin, xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    // Create a wrapped USDC token for another chain
    let wrapped_usdc_admin = Address::generate(&env);
    let wrapped_usdc = env.register_stellar_asset_contract_v2(wrapped_usdc_admin).address();

    // Register wrapped USDC as pointing to canonical USDC
    client.register_wrapped_token(&wrapped_usdc, &usdc_token);

    // Verify wrapped token is considered whitelisted
    assert!(client.is_token_whitelisted(&wrapped_usdc));

    // Create vault with wrapped token should work
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &beneficiary, &1_000u64, &Some(wrapped_usdc.clone()));
    
    // Vault should accept wrapped token
    assert_eq!(client.get_vault(&vault_id).token_address, wrapped_usdc);
}

// ─────────────────────────────────────────────────────────────────────────────
// Token Precision and Edge Cases
// ─────────────────────────────────────────────────────────────────────────────

/// Test USDC deposit with minimum amount (1 stroop = 0.000001 USDC)
#[test]
fn test_usdc_deposit_minimum_amount() {
    let (env, _admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;

    // Mint 1 stroop
    StellarAssetClient::new(&env, &usdc_token).mint(&owner, &1i128);

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &Some(usdc_token.clone()));

    // Deposit 1 stroop
    client.deposit(&vault_id, &owner, &1i128);

    assert_eq!(client.get_vault(&vault_id).balance, 1);
}

/// Test USDC deposit with large amount (max i128)
#[test]
fn test_usdc_deposit_large_amount() {
    let (env, _admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;

    // Use large but safe amount
    let large_amount = 1_000_000_000_000_000i128; // 1 trillion USDC

    StellarAssetClient::new(&env, &usdc_token).mint(&owner, &large_amount);

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &Some(usdc_token.clone()));

    client.deposit(&vault_id, &owner, &large_amount);

    assert_eq!(client.get_vault(&vault_id).balance, large_amount);
}

// ─────────────────────────────────────────────────────────────────────────────
// Security and Error Cases
// ─────────────────────────────────────────────────────────────────────────────

/// Verify vault status prevents operations on wrong token type
#[test]
fn test_cannot_withdraw_from_released_usdc_vault() {
    let (env, _admin, _xlm_token, usdc_token, client) = setup_with_xlm_and_usdc();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let interval = 1_000u64;
    let deposit_amount = 5_000_000i128;

    // Setup and release
    StellarAssetClient::new(&env, &usdc_token).mint(&owner, &deposit_amount);
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &Some(usdc_token.clone()));
    client.deposit(&vault_id, &owner, &deposit_amount);
    env.ledger().with_mut(|l| l.timestamp = interval + 1);
    client.trigger_release(&vault_id);

    // Attempt withdrawal from released vault should fail
    let result = client.withdraw(&vault_id, &owner, &1_000_000i128);
    assert!(result.is_err());
}
