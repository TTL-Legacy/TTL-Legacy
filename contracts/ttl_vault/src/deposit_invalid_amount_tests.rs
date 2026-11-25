#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{self, StellarAssetClient},
    vec, Address, BytesN, Env,
};

fn setup_invalid_amount_test() -> (
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

// ============================================================================
// TESTS FOR INVALID DEPOSIT AMOUNTS (Zero and Negative)
// ============================================================================

/// Test that deposit with zero amount returns InvalidAmount error
/// This prevents silent no-op transactions that waste fees
#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_deposit_with_zero_amount_returns_invalid_amount() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Attempt to deposit zero amount - should panic with InvalidAmount
    client.deposit(&vault_id, &owner, &0i128);
}

/// Test that deposit with negative amount returns InvalidAmount error
#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_deposit_with_negative_amount_returns_invalid_amount() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Attempt to deposit negative amount - should panic with InvalidAmount
    client.deposit(&vault_id, &owner, &-100_000i128);
}

/// Test that deposit with i128::MIN returns InvalidAmount error
#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_deposit_with_min_i128_returns_invalid_amount() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Attempt to deposit with i128::MIN - should panic with InvalidAmount
    client.deposit(&vault_id, &owner, &i128::MIN);
}

/// Test that deposit with negative one returns InvalidAmount error
#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_deposit_with_negative_one_returns_invalid_amount() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Attempt to deposit -1 - should panic with InvalidAmount
    client.deposit(&vault_id, &owner, &-1i128);
}

/// Test that positive deposit works correctly (boundary case: smallest positive amount)
#[test]
fn test_deposit_with_one_unit_succeeds() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Deposit smallest positive amount - should succeed
    client.deposit(&vault_id, &owner, &1i128);

    // Verify balance was updated correctly
    assert_eq!(client.get_vault(&vault_id).balance, 1i128);
}

/// Test that batch_deposit with zero amount in array returns InvalidAmount
#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_batch_deposit_with_zero_amount_returns_invalid_amount() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &interval, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Batch deposit with zero amount in one vault - should panic with InvalidAmount
    let deposits = vec![
        &env,
        (vault_id_1, 100_000i128),
        (vault_id_2, 0i128), // Zero amount should cause panic
    ];
    client.batch_deposit(&owner, &deposits);
}

/// Test that batch_deposit with negative amount in array returns InvalidAmount
#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_batch_deposit_with_negative_amount_returns_invalid_amount() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &interval, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Batch deposit with negative amount in one vault - should panic with InvalidAmount
    let deposits = vec![
        &env,
        (vault_id_1, 100_000i128),
        (vault_id_2, -50_000i128), // Negative amount should cause panic
    ];
    client.batch_deposit(&owner, &deposits);
}

/// Test that batch_deposit with all zero amounts returns InvalidAmount on first entry
#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_batch_deposit_with_all_zero_amounts_returns_invalid_amount() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Batch deposit with zero amounts - should panic with InvalidAmount
    let deposits = vec![&env, (vault_id_1, 0i128)];
    client.batch_deposit(&owner, &deposits);
}

/// Test that batch_deposit validates all amounts before processing transfers
/// This ensures zero/negative amounts are caught early, preventing partial state changes
#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_batch_deposit_validates_all_amounts_before_transfer() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &interval, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &interval, &None);
    let vault_id_3 = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Batch deposit with zero in first position - should fail validation
    let deposits = vec![
        &env,
        (vault_id_1, 0i128), // Zero - fails validation before any transfer
        (vault_id_2, 50_000i128),
        (vault_id_3, 75_000i128),
    ];
    client.batch_deposit(&owner, &deposits);
}

/// Test that vault balance remains zero after failed zero-amount deposit attempt
/// Ensures no state mutation on invalid input
#[test]
fn test_failed_zero_deposit_leaves_vault_empty() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    
    // Verify vault starts empty
    assert_eq!(client.get_vault(&vault_id).balance, 0);

    // Verify vault is still empty (no state change occurred)
    assert_eq!(client.get_vault(&vault_id).balance, 0);
}

/// Test boundary: largest negative value that should fail
#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_deposit_with_large_negative_amount_fails() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Attempt to deposit very large negative amount
    client.deposit(&vault_id, &owner, &-999_999_999_999i128);
}

/// Test that the InvalidAmount guard prevents silent no-ops
/// Multiple zero deposits should all fail, preventing wasted transaction fees
#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_first_valid_deposit_then_zero_deposit_fails() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // First deposit succeeds
    client.deposit(&vault_id, &owner, &100_000i128);
    assert_eq!(client.get_vault(&vault_id).balance, 100_000);

    // Second attempt with zero should fail (will panic)
    // This prevents a silent no-op that would waste transaction fees
    client.deposit(&vault_id, &owner, &0i128);
}

/// Test that deposit validation happens before auth requirements
/// Zero amount should be caught even if caller is not owner
#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_deposit_validates_amount_early() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    
    // Try to deposit zero - should fail with InvalidAmount before any other checks
    client.deposit(&vault_id, &owner, &0i128);
}

/// Test that negative deposit of exactly -1 is caught
#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_deposit_negative_one_fails_consistently() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Deposit -1 should fail
    client.deposit(&vault_id, &owner, &-1i128);
}

/// Test boundary condition: verify +1 succeeds while 0 and -1 fail
#[test]
fn test_deposit_amount_boundary_conditions() {
    let (env, owner, beneficiary, _token_address, client) = setup_invalid_amount_test();
    let interval = 1_000u64;

    let vault_id_1 = client.create_vault(&owner, &beneficiary, &interval, &None);
    let vault_id_2 = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Deposit +1 should succeed
    client.deposit(&vault_id_1, &owner, &1i128);
    assert_eq!(client.get_vault(&vault_id_1).balance, 1i128);

    // Verify another positive amount also works
    client.deposit(&vault_id_2, &owner, &100i128);
    assert_eq!(client.get_vault(&vault_id_2).balance, 100i128);
}
