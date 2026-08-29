/// Tests for beneficiary owner validation (Issue #1124)
/// Ensures both create_vault and update_beneficiary reject owner self-assignment

#[cfg(test)]
mod tests {
    use super::super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let contract_id = env.register_contract(None, TtlVaultContract);
        let client = TtlVaultContractClient::new(&env, &contract_id);
        let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

        // Setup: initialize contract
        let xlm_token = Address::generate(&env);
        client.initialize(&xlm_token, &admin);

        (env, owner, beneficiary, client)
    }

    /// Test that create_vault rejects owner as beneficiary
    /// Should fail with InvalidBeneficiary error
    #[test]
    fn test_create_vault_rejects_owner_as_beneficiary() {
        let (_, owner, _, client) = setup();

        let result = client.try_create_vault(&owner, &owner, &86400, &None);
        assert!(result.is_err(), "create_vault should reject owner as beneficiary");
        // create_vault panics (via panic_with_error!) rather than returning a
        // Result, so the client surfaces the raw host error code here instead
        // of a typed ContractError.
        assert_eq!(
            result.unwrap_err().unwrap(),
            soroban_sdk::Error::from_contract_error(ContractError::InvalidBeneficiary as u32)
        );
    }

    /// Test that update_beneficiary rejects owner self-assignment
    /// Should fail with InvalidBeneficiary error
    #[test]
    fn test_update_beneficiary_rejects_owner_self_assignment() {
        let (_, owner, beneficiary, client) = setup();

        // Create vault with different beneficiary
        let vault_id = client.create_vault(&owner, &beneficiary, &86400, &None);

        // Try to set owner as the new beneficiary
        let result = client.try_update_beneficiary(&vault_id, &owner, &owner);
        assert!(
            result.is_err(),
            "update_beneficiary should reject owner as new beneficiary"
        );
        match result.unwrap_err().unwrap() {
            ContractError::InvalidBeneficiary => {},
            e => panic!("Expected InvalidBeneficiary, got {:?}", e),
        }
    }

    /// Test that update_beneficiary accepts non-owner beneficiary
    /// Should succeed when new beneficiary is different from owner
    #[test]
    fn test_update_beneficiary_accepts_non_owner() {
        let (env, owner, beneficiary, client) = setup();

        let vault_id = client.create_vault(&owner, &beneficiary, &86400, &None);
        let new_beneficiary = Address::generate(&env);

        let result = client.try_update_beneficiary(&vault_id, &owner, &new_beneficiary);
        assert!(result.is_ok(), "update_beneficiary should accept non-owner beneficiary");
    }

    /// Test that owner cannot update vault to assign themselves as beneficiary
    /// at any point after creation
    #[test]
    fn test_update_beneficiary_owner_check_is_enforced() {
        let (env, owner, beneficiary, client) = setup();

        let vault_id = client.create_vault(&owner, &beneficiary, &86400, &None);

        // First update succeeds with different beneficiary
        let new_ben1 = Address::generate(&env);
        assert!(
            client.try_update_beneficiary(&vault_id, &owner, &new_ben1).is_ok(),
            "First update with different beneficiary should succeed"
        );

        // Second update fails if trying to set owner as beneficiary
        let result = client.try_update_beneficiary(&vault_id, &owner, &owner);
        assert!(
            result.is_err(),
            "Update to set owner as beneficiary should fail"
        );
        match result.unwrap_err().unwrap() {
            ContractError::InvalidBeneficiary => {},
            e => panic!("Expected InvalidBeneficiary, got {:?}", e),
        }

        // Third update succeeds with different beneficiary again
        let new_ben2 = Address::generate(&env);
        assert!(
            client.try_update_beneficiary(&vault_id, &owner, &new_ben2).is_ok(),
            "Subsequent update with different beneficiary should succeed"
        );
    }

    /// Test that InvalidBeneficiary is the correct error code
    /// Ensures consistency with error handling across the contract
    #[test]
    fn test_invalid_beneficiary_error_code() {
        let (_, owner, _, client) = setup();

        let result = client.try_create_vault(&owner, &owner, &86400, &None);
        assert!(result.is_err());
        // Correct error code used (17 = InvalidBeneficiary).
        assert_eq!(
            result.unwrap_err().unwrap(),
            soroban_sdk::Error::from_contract_error(ContractError::InvalidBeneficiary as u32)
        );
    }
}
