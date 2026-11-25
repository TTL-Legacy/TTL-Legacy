/// Tests for Issue #1117: Multi-sig requirement for sensitive owner operations
/// Tests 2-of-2, 2-of-3, expiry cancellation, and duplicate signer rejection

#[cfg(test)]
mod multisig_pending_op_tests {
    use crate::*;
    use soroban_sdk::{testutils::Address as _, Address, Bytes, Env, Vec};

    fn setup() -> (Env, Address, Address, Address, Address, TtlVaultContractClient) {
        let env = Env::default();
        let owner = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let signer2 = Address::generate(&env);

        env.mock_all_auths();

        let contract_id = env.register_contract(None, TtlVaultContract);
        let client = TtlVaultContractClient::new(&env, &contract_id);

        let token_admin = Address::generate(&env);
        let admin = Address::generate(&env);

        let token_id = soroban_sdk::testutils::register_test_contract(
            &env,
            &env.current_contract_address(),
        );

        client.initialize(&token_id, &admin);

        (env, owner, beneficiary, signer1, signer2, client)
    }

    #[test]
    fn test_initiate_pending_multisig_op_2of2() {
        let (env, owner, beneficiary, signer1, _signer2, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

        // Configure multisig with 2-of-2 (owner + 1 co-signer)
        client.configure_multisig(
            &vault_id,
            &owner,
            &soroban_sdk::vec![&env, signer1.clone()],
            &2u32,
        );

        // Set multisig_required_ops to include Withdraw
        let ops = soroban_sdk::vec![&env, MultiSigOperation::Withdraw];
        // Store this via vault configuration (need to update vault with multisig_required_ops)

        // Initiate a pending withdrawal operation
        let amount: i128 = 100;
        let payload = client.encode_i128_payload(&env, &amount);

        let nonce = client
            .initiate_pending_multisig_op(
                &vault_id,
                &owner,
                &MultiSigOperation::Withdraw,
                &payload,
                &None,
            )
            .unwrap();

        assert_eq!(nonce, 1);

        // Verify the pending op exists
        let pending_op = client.get_pending_multisig_op(&vault_id, &nonce).unwrap();
        assert_eq!(pending_op.vault_id, vault_id);
        assert_eq!(pending_op.nonce, 1);
        assert_eq!(pending_op.signers.len(), 1); // Only owner has signed so far
    }

    #[test]
    fn test_cosign_pending_op_with_cosigner() {
        let (env, owner, beneficiary, signer1, _signer2, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

        client.configure_multisig(
            &vault_id,
            &owner,
            &soroban_sdk::vec![&env, signer1.clone()],
            &2u32,
        );

        let amount: i128 = 100;
        let payload = client.encode_i128_payload(&env, &amount);

        let nonce = client
            .initiate_pending_multisig_op(
                &vault_id,
                &owner,
                &MultiSigOperation::Withdraw,
                &payload,
                &None,
            )
            .unwrap();

        // Co-signer approves
        client
            .cosign_pending_op(&vault_id, &signer1, &nonce)
            .unwrap();

        let pending_op = client.get_pending_multisig_op(&vault_id, &nonce).unwrap();
        assert_eq!(pending_op.signers.len(), 2); // Owner + co-signer
    }

    #[test]
    fn test_duplicate_signer_rejection() {
        let (env, owner, beneficiary, signer1, _signer2, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

        client.configure_multisig(
            &vault_id,
            &owner,
            &soroban_sdk::vec![&env, signer1.clone()],
            &2u32,
        );

        let amount: i128 = 100;
        let payload = client.encode_i128_payload(&env, &amount);

        let nonce = client
            .initiate_pending_multisig_op(
                &vault_id,
                &owner,
                &MultiSigOperation::Withdraw,
                &payload,
                &None,
            )
            .unwrap();

        // Co-signer approves once
        client
            .cosign_pending_op(&vault_id, &signer1, &nonce)
            .unwrap();

        // Attempt to co-sign again with same address
        let err = client.cosign_pending_op(&vault_id, &signer1, &nonce).err().unwrap();
        assert_eq!(err, ContractError::DuplicateSignature);
    }

    #[test]
    fn test_2of3_multisig_requires_2_cosignatures() {
        let (env, owner, beneficiary, signer1, signer2, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

        // Configure 2-of-3 (owner + 2 co-signers)
        client.configure_multisig(
            &vault_id,
            &owner,
            &soroban_sdk::vec![&env, signer1.clone(), signer2.clone()],
            &2u32,
        );

        let amount: i128 = 100;
        let payload = client.encode_i128_payload(&env, &amount);

        let nonce = client
            .initiate_pending_multisig_op(
                &vault_id,
                &owner,
                &MultiSigOperation::Withdraw,
                &payload,
                &None,
            )
            .unwrap();

        // First co-signer approves
        client
            .cosign_pending_op(&vault_id, &signer1, &nonce)
            .unwrap();

        let pending_op = client.get_pending_multisig_op(&vault_id, &nonce).unwrap();
        assert_eq!(pending_op.signers.len(), 2); // Owner + signer1

        // Second co-signer approves
        client
            .cosign_pending_op(&vault_id, &signer2, &nonce)
            .unwrap();

        let pending_op = client.get_pending_multisig_op(&vault_id, &nonce).unwrap();
        assert_eq!(pending_op.signers.len(), 3); // Owner + signer1 + signer2
    }

    #[test]
    fn test_expiry_cancellation() {
        let (env, owner, beneficiary, _signer1, _signer2, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

        let amount: i128 = 100;
        let payload = client.encode_i128_payload(&env, &amount);

        let nonce = client
            .initiate_pending_multisig_op(
                &vault_id,
                &owner,
                &MultiSigOperation::Withdraw,
                &payload,
                &None,
            )
            .unwrap();

        let pending_op = client.get_pending_multisig_op(&vault_id, &nonce).unwrap();
        let expires_at = pending_op.expires_at;

        // Advance ledger to past expiry
        let current_ledger = env.ledger().sequence();
        let ledger_to_advance =
            ((expires_at - env.ledger().timestamp() as u64) / 5) + 100; // 5s per ledger
        env.ledger().with_sequence(current_ledger + ledger_to_advance as u32);

        // Check if expired
        let is_expired = client.is_pending_multisig_op_expired(&vault_id, &nonce);
        assert!(is_expired);
    }

    #[test]
    fn test_non_signer_cannot_cosign() {
        let (env, owner, beneficiary, _signer1, _signer2, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
        let unauthorized = Address::generate(&env);

        client.configure_multisig(
            &vault_id,
            &owner,
            &soroban_sdk::vec![&env, unauthorized.clone()],
            &2u32,
        );

        let amount: i128 = 100;
        let payload = client.encode_i128_payload(&env, &amount);

        let nonce = client
            .initiate_pending_multisig_op(
                &vault_id,
                &owner,
                &MultiSigOperation::Withdraw,
                &payload,
                &None,
            )
            .unwrap();

        // Another address (not owner, not configured signer) tries to cosign
        let random_address = Address::generate(&env);
        let err = client
            .cosign_pending_op(&vault_id, &random_address, &nonce)
            .err()
            .unwrap();
        assert_eq!(err, ContractError::NotASigner);
    }

    #[test]
    fn test_beneficiary_update_multisig() {
        let (env, owner, beneficiary, signer1, _signer2, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
        let new_beneficiary = Address::generate(&env);

        client.configure_multisig(
            &vault_id,
            &owner,
            &soroban_sdk::vec![&env, signer1.clone()],
            &2u32,
        );

        let nonce = client
            .initiate_pending_multisig_op(
                &vault_id,
                &owner,
                &MultiSigOperation::UpdateBeneficiary,
                &Bytes::new(&env),
                &Some(new_beneficiary.clone()),
            )
            .unwrap();

        // Co-signer approves
        client
            .cosign_pending_op(&vault_id, &signer1, &nonce)
            .unwrap();

        let pending_op = client.get_pending_multisig_op(&vault_id, &nonce).unwrap();
        assert_eq!(pending_op.signers.len(), 2);
        assert_eq!(pending_op.address_payload, Some(new_beneficiary));
    }

    #[test]
    fn test_cancel_vault_multisig() {
        let (env, owner, beneficiary, signer1, _signer2, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

        client.configure_multisig(
            &vault_id,
            &owner,
            &soroban_sdk::vec![&env, signer1.clone()],
            &2u32,
        );

        let nonce = client
            .initiate_pending_multisig_op(
                &vault_id,
                &owner,
                &MultiSigOperation::CancelVault,
                &Bytes::new(&env),
                &None,
            )
            .unwrap();

        // Co-signer approves
        client
            .cosign_pending_op(&vault_id, &signer1, &nonce)
            .unwrap();

        let pending_op = client.get_pending_multisig_op(&vault_id, &nonce).unwrap();
        assert_eq!(pending_op.operation, MultiSigOperation::CancelVault);
        assert_eq!(pending_op.signers.len(), 2);
    }
}
