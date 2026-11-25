#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    vec, Address, Bytes, BytesN, Env,
};

fn setup() -> (
    Env,
    Address,
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
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    soroban_sdk::token::StellarAssetClient::new(&env, &token_address)
        .mint(&owner, &1_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    (env, owner, beneficiary, admin, token_address, client)
}

#[test]
fn test_create_pending_multisig_op() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    // Create a vault
    let vault_id = client.create(
        &owner,
        &beneficiary,
        &7_200u64, // 2-hour check-in interval
        &token_address,
        &None::<Address>,
    );

    // Enable multi-sig with 2-of-2 requirement for withdrawals
    let signer = Address::generate(&env);
    client.configure_multisig(&vault_id, &owner, &vec![&env, signer.clone()], &2u32);

    // Configure vault to require multi-sig for withdrawals
    let mut vault = client.get_vault(&vault_id).unwrap();
    assert_eq!(vault.multisig_required_ops.len(), 0); // No ops configured yet

    // Deposit funds
    client.deposit(&vault_id, &owner, &1000);

    // Create a pending multi-sig operation for withdrawal
    let withdrawal_amount = 500i128;
    let payload = Bytes::from_array(&env, &withdrawal_amount.to_le_bytes());

    let op_nonce = client.create_pending_multisig_op(
        &vault_id,
        &owner,
        &MultiSigOperation::Withdraw,
        &payload,
        &None::<Address>,
    );

    assert_eq!(op_nonce, 1u64);

    // Verify the pending operation exists
    let pending_op = client
        .get_pending_multisig_op(&vault_id, &op_nonce)
        .unwrap();
    assert_eq!(pending_op.nonce, 1u64);
    assert_eq!(pending_op.vault_id, vault_id);
    assert_eq!(pending_op.operation, MultiSigOperation::Withdraw);
    assert_eq!(pending_op.threshold, 2u32);
    assert_eq!(pending_op.signers.len(), 1); // Only owner has signed
}

#[test]
fn test_cosign_pending_multisig_op() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let vault_id = client.create(
        &owner,
        &beneficiary,
        &7_200u64,
        &token_address,
        &None::<Address>,
    );

    let signer = Address::generate(&env);
    client.configure_multisig(&vault_id, &owner, &vec![&env, signer.clone()], &2u32);

    client.deposit(&vault_id, &owner, &1000);

    let withdrawal_amount = 500i128;
    let payload = Bytes::from_array(&env, &withdrawal_amount.to_le_bytes());

    let op_nonce = client.create_pending_multisig_op(
        &vault_id,
        &owner,
        &MultiSigOperation::Withdraw,
        &payload,
        &None::<Address>,
    );

    // Co-sign with the second signer
    client.cosign_pending_multisig_op(&vault_id, &op_nonce, &signer, &payload);

    // Verify the operation now has 2 signers
    let pending_op = client
        .get_pending_multisig_op(&vault_id, &op_nonce)
        .unwrap();
    assert_eq!(pending_op.signers.len(), 2);
}

#[test]
fn test_execute_pending_multisig_op_2of2() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let vault_id = client.create(
        &owner,
        &beneficiary,
        &7_200u64,
        &token_address,
        &None::<Address>,
    );

    let signer = Address::generate(&env);
    client.configure_multisig(&vault_id, &owner, &vec![&env, signer.clone()], &2u32);

    client.deposit(&vault_id, &owner, &1000);

    let withdrawal_amount = 500i128;
    let payload = Bytes::from_array(&env, &withdrawal_amount.to_le_bytes());

    let op_nonce = client.create_pending_multisig_op(
        &vault_id,
        &owner,
        &MultiSigOperation::Withdraw,
        &payload,
        &None::<Address>,
    );

    // Co-sign
    client.cosign_pending_multisig_op(&vault_id, &op_nonce, &signer, &payload);

    // Execute withdrawal
    client.execute_pending_multisig_op(&vault_id, &op_nonce, &owner);

    // Verify withdrawal succeeded
    let vault = client.get_vault(&vault_id).unwrap();
    assert_eq!(vault.balance, 500); // 1000 - 500

    // Verify pending operation is gone
    assert!(client.get_pending_multisig_op(&vault_id, &op_nonce).is_none());
}

#[test]
fn test_pending_multisig_op_expiry() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let vault_id = client.create(
        &owner,
        &beneficiary,
        &7_200u64,
        &token_address,
        &None::<Address>,
    );

    let signer = Address::generate(&env);
    client.configure_multisig(&vault_id, &owner, &vec![&env, signer.clone()], &2u32);

    client.deposit(&vault_id, &owner, &1000);

    let withdrawal_amount = 500i128;
    let payload = Bytes::from_array(&env, &withdrawal_amount.to_le_bytes());

    let op_nonce = client.create_pending_multisig_op(
        &vault_id,
        &owner,
        &MultiSigOperation::Withdraw,
        &payload,
        &None::<Address>,
    );

    // Advance time past expiry (15 minutes)
    env.ledger().with_mut(|ledger| {
        ledger.set_timestamp(ledger.timestamp() + 950); // > 900s expiry
    });

    // Try to execute - should fail with expiry error
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.execute_pending_multisig_op(&vault_id, &op_nonce, &owner);
    }));
    assert!(result.is_err()); // Should panic or return error
}

#[test]
fn test_duplicate_signer_rejection() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let vault_id = client.create(
        &owner,
        &beneficiary,
        &7_200u64,
        &token_address,
        &None::<Address>,
    );

    let signer = Address::generate(&env);
    client.configure_multisig(&vault_id, &owner, &vec![&env, signer.clone()], &2u32);

    client.deposit(&vault_id, &owner, &1000);

    let withdrawal_amount = 500i128;
    let payload = Bytes::from_array(&env, &withdrawal_amount.to_le_bytes());

    let op_nonce = client.create_pending_multisig_op(
        &vault_id,
        &owner,
        &MultiSigOperation::Withdraw,
        &payload,
        &None::<Address>,
    );

    // Try to co-sign twice with same signer
    client.cosign_pending_multisig_op(&vault_id, &op_nonce, &signer, &payload);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.cosign_pending_multisig_op(&vault_id, &op_nonce, &signer, &payload);
    }));
    assert!(result.is_err()); // Should reject duplicate signature
}
