/// Tests for Issue #1263: vault check-in panics with unhelpful message when vault does not exist.
///
/// Verifies that `check_in` (and related check-in variants) return a structured
/// `ContractError::VaultNotFound` instead of panicking with a raw string when the
/// requested vault ID does not exist.
extern crate alloc;

use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};

fn setup_checkin_nonexistent() -> (Env, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_address = Address::generate(&env);

    let contract_id = env.register_contract(None, TtlVaultContract);
    let client: TtlVaultContractClient<'static> =
        unsafe { core::mem::transmute(TtlVaultContractClient::new(&env, &contract_id)) };

    client.initialize(&token_address, &admin);

    (env, admin, client)
}

/// Issue #1263: check_in on a non-existent vault should return ContractError::VaultNotFound.
/// Previously this would panic with `expect("vault not found")` producing an unhelpful raw string.
/// Now `load_vault` uses `panic_with_error!(env, ContractError::VaultNotFound)` which maps to a
/// structured error the caller can inspect.
#[test]
fn test_check_in_nonexistent_vault_returns_vault_not_found() {
    let (env, _admin, client) = setup_checkin_nonexistent();

    let caller = Address::generate(&env);
    let passkey_hash = BytesN::from_array(&env, &[1u8; 32]);
    let nonexistent_vault_id: u64 = 999_999;

    let result = client.try_check_in(&nonexistent_vault_id, &caller, &passkey_hash, &0u64);

    assert!(
        result.is_err(),
        "check_in on non-existent vault should return an error"
    );

    match result.unwrap_err().unwrap() {
        ContractError::VaultNotFound => {}
        other => panic!(
            "Expected ContractError::VaultNotFound, got {:?}",
            other
        ),
    }
}

/// Ensure a vault that has just been created does NOT trigger VaultNotFound.
/// This is the positive counterpart to the above test.
#[test]
fn test_check_in_existing_vault_does_not_return_vault_not_found() {
    let (env, _admin, client) = setup_checkin_nonexistent();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let passkey_hash = BytesN::from_array(&env, &[1u8; 32]);

    // Create a real vault
    let vault_id = client.create_vault(&owner, &beneficiary, &86_400u64, &None);

    // Register a passkey so the check-in does not fail on InvalidPasskey
    client.add_passkey(&vault_id, &owner, &passkey_hash);

    let result = client.try_check_in(&vault_id, &owner, &passkey_hash, &0u64);

    // The error must NOT be VaultNotFound; it may succeed or fail for other reasons
    // (e.g. InvalidInterval / rate-limit), but the vault was found.
    if let Err(ref e) = result {
        if let Ok(ContractError::VaultNotFound) = e {
            panic!("check_in should not return VaultNotFound for an existing vault");
        }
    }
}
