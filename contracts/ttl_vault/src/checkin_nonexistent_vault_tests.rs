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

/// Issue #2041 / #1263: check_in on a non-existent vault should return
/// ContractError::VaultNotFound (HTTP 404) instead of panicking with a raw
/// string from an unwrap inside `load_vault`.
///
/// `load_vault` now calls `panic_with_error!(env, ContractError::VaultNotFound)`
/// so the structured error code propagates back to the caller and the backend
/// can map it to a 404 response.
#[test]
fn test_check_in_nonexistent_vault_returns_vault_not_found() {
    let (env, _admin, client) = setup_checkin_nonexistent();

    let caller = Address::generate(&env);
    let passkey_hash = BytesN::from_array(&env, &[1u8; 32]);
    let nonexistent_vault_id: u64 = 999_999;

    // check_in signature: (vault_id, caller, passkey_hash, nonce, signature, message)
    let result = client.try_check_in(
        &nonexistent_vault_id,
        &caller,
        &passkey_hash,
        &0u64,
        &None::<soroban_sdk::BytesN<64>>,
        &None::<soroban_sdk::Bytes>,
    );

    assert!(
        result.is_err(),
        "check_in on non-existent vault should return an error"
    );

    let err = result.unwrap_err().unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from_contract_error(ContractError::VaultNotFound as u32),
        "error must be VaultNotFound so the backend can map it to HTTP 404"
    );
}

/// Positive counterpart: check_in on an existing vault must NOT return
/// VaultNotFound.  It may fail for other reasons (rate-limit, bad passkey,
/// etc.) but the vault was found.
#[test]
fn test_check_in_existing_vault_does_not_return_vault_not_found() {
    let (env, _admin, client) = setup_checkin_nonexistent();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let passkey_hash = BytesN::from_array(&env, &[1u8; 32]);

    // Create a real vault
    let vault_id = client.create_vault(&owner, &beneficiary, &86_400u64, &None);

    // Register a passkey so check_in does not fail on InvalidPasskey
    client.add_passkey(&vault_id, &owner, &passkey_hash);

    // check_in signature: (vault_id, caller, passkey_hash, nonce, signature, message)
    let result = client.try_check_in(
        &vault_id,
        &owner,
        &passkey_hash,
        &0u64,
        &None::<soroban_sdk::BytesN<64>>,
        &None::<soroban_sdk::Bytes>,
    );

    // The result must not be VaultNotFound; any other error or success is fine.
    if let Err(Ok(ContractError::VaultNotFound)) = result {
        panic!("check_in must not return VaultNotFound for an existing vault");
    }
}
