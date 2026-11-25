/// Tests for vault archiving functionality (Issue #1123)
/// Ensures vaults in terminal states (Released/Cancelled) can be archived
/// and moved to cheaper persistent storage while remaining queryable

#[cfg(test)]
mod tests {
    use super::super::*;

    fn setup() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let contract_id = env.register_contract(None, TtlVaultContract);
        let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

        // Setup: initialize contract
        let xlm_token = Address::generate(&env);
        client.initialize(&admin, &xlm_token);

        (env, owner, beneficiary, client)
    }

    /// Test archiving a released vault
    /// Should succeed and move vault to persistent storage
    #[test]
    fn test_archive_released_vault() {
        let (env, owner, beneficiary, client) = setup();

        // Create and release a vault
        let vault_id = client.create_vault(&owner, &beneficiary, &86400, &None);

        // Manually mark vault as released for testing
        let mut vault = client.get_vault(&vault_id);
        vault.status = ReleaseStatus::Released;
        // Note: In real scenario, trigger_release would do this

        // Archive the vault
        let result = client.try_archive_vault(&vault_id);
        assert!(result.is_ok(), "Archiving released vault should succeed");
    }

    /// Test archiving a cancelled vault
    /// Should succeed and move vault to persistent storage
    #[test]
    fn test_archive_cancelled_vault() {
        let (env, owner, beneficiary, client) = setup();

        // Create a vault
        let vault_id = client.create_vault(&owner, &beneficiary, &86400, &None);

        // Manually mark vault as cancelled for testing
        let mut vault = client.get_vault(&vault_id);
        vault.status = ReleaseStatus::Cancelled;

        // Archive the vault
        let result = client.try_archive_vault(&vault_id);
        assert!(result.is_ok(), "Archiving cancelled vault should succeed");
    }

    /// Test archiving an active (Locked) vault
    /// Should fail with NotReleased error
    #[test]
    fn test_archive_active_vault_fails() {
        let (env, owner, beneficiary, client) = setup();

        // Create a vault (starts as Locked/Active)
        let vault_id = client.create_vault(&owner, &beneficiary, &86400, &None);

        // Try to archive active vault
        let result = client.try_archive_vault(&vault_id);
        assert!(result.is_err(), "Archiving active vault should fail");
        match result.unwrap_err().unwrap() {
            ContractError::NotReleased => {}
            e => panic!("Expected NotReleased, got {:?}", e),
        }
    }

    /// Test archiving an EmergencyFrozen vault
    /// Should fail with NotReleased error
    #[test]
    fn test_archive_frozen_vault_fails() {
        let (env, owner, beneficiary, client) = setup();

        // Create a vault
        let vault_id = client.create_vault(&owner, &beneficiary, &86400, &None);

        // Manually mark vault as frozen for testing
        let mut vault = client.get_vault(&vault_id);
        vault.status = ReleaseStatus::EmergencyFrozen;

        // Try to archive frozen vault
        let result = client.try_archive_vault(&vault_id);
        assert!(result.is_err(), "Archiving frozen vault should fail");
    }

    /// Test retrieving an archived vault
    /// Should return the vault data from persistent storage
    #[test]
    fn test_get_archived_vault() {
        let (env, owner, beneficiary, client) = setup();

        // Create and archive a vault
        let vault_id = client.create_vault(&owner, &beneficiary, &86400, &None);
        let mut vault = client.get_vault(&vault_id);
        vault.status = ReleaseStatus::Released;

        let result = client.try_archive_vault(&vault_id);
        assert!(result.is_ok());

        // Retrieve archived vault
        let archived_vault = client.get_archived_vault(&vault_id);
        assert_eq!(archived_vault.owner, owner);
        assert_eq!(archived_vault.beneficiary, beneficiary);
        assert_eq!(archived_vault.status, ReleaseStatus::Released);
    }

    /// Test querying vault_is_archived
    /// Should return true for archived vaults, false for others
    #[test]
    fn test_vault_is_archived() {
        let (env, owner, beneficiary, client) = setup();

        // Create vault
        let vault_id = client.create_vault(&owner, &beneficiary, &86400, &None);

        // Initially not archived
        let is_archived = client.vault_is_archived(&vault_id);
        assert!(!is_archived, "New vault should not be archived");

        // Archive the vault
        let mut vault = client.get_vault(&vault_id);
        vault.status = ReleaseStatus::Released;
        client.archive_vault(&vault_id);

        // Now should be archived
        let is_archived = client.vault_is_archived(&vault_id);
        assert!(is_archived, "Archived vault should return true");
    }

    /// Test attempting to archive the same vault twice
    /// Should fail with DuplicateVault error (already archived)
    #[test]
    fn test_double_archive_fails() {
        let (env, owner, beneficiary, client) = setup();

        // Create and archive a vault
        let vault_id = client.create_vault(&owner, &beneficiary, &86400, &None);
        let mut vault = client.get_vault(&vault_id);
        vault.status = ReleaseStatus::Released;

        client.archive_vault(&vault_id);

        // Try to archive again
        let result = client.try_archive_vault(&vault_id);
        assert!(
            result.is_err(),
            "Archiving already-archived vault should fail"
        );
        match result.unwrap_err().unwrap() {
            ContractError::DuplicateVault => {}
            e => panic!("Expected DuplicateVault, got {:?}", e),
        }
    }

    /// Test that multiple different vaults can be archived
    /// Each should be stored independently in persistent storage
    #[test]
    fn test_archive_multiple_vaults() {
        let (env, owner, beneficiary, client) = setup();

        let mut vault_ids: Vec<u64> = Vec::new();
        for i in 0..3 {
            let b = Address::generate(&env);
            let vault_id = client.create_vault(&owner, &b, &(86400 + i * 1000), &None);
            let mut vault = client.get_vault(&vault_id);
            vault.status = ReleaseStatus::Released;

            client.archive_vault(&vault_id);
            vault_ids.push(vault_id);
        }

        // Verify all are archived
        for vault_id in vault_ids {
            assert!(
                client.vault_is_archived(&vault_id),
                "All vaults should be archived"
            );
            let archived = client.get_archived_vault(&vault_id);
            assert_eq!(archived.status, ReleaseStatus::Released);
        }
    }

    /// Test archiving preserves vault data integrity
    /// Data should be unchanged when retrieved from archive
    #[test]
    fn test_archived_vault_data_integrity() {
        let (env, owner, beneficiary, client) = setup();

        // Create vault
        let vault_id = client.create_vault(&owner, &beneficiary, &86400, &None);

        // Get vault before archiving
        let vault_before = client.get_vault(&vault_id);
        let check_in_interval_before = vault_before.check_in_interval;
        let balance_before = vault_before.balance;
        let owner_before = vault_before.owner;

        // Archive vault
        let mut vault = client.get_vault(&vault_id);
        vault.status = ReleaseStatus::Released;
        client.archive_vault(&vault_id);

        // Get archived vault
        let vault_after = client.get_archived_vault(&vault_id);

        // Verify data integrity
        assert_eq!(vault_after.check_in_interval, check_in_interval_before);
        assert_eq!(vault_after.balance, balance_before);
        assert_eq!(vault_after.owner, owner_before);
        assert_eq!(vault_after.status, ReleaseStatus::Released);
    }

    /// Test that anyone can archive a released/cancelled vault
    /// No special permissions required
    #[test]
    fn test_anyone_can_archive_vault() {
        let (env, owner, beneficiary, client) = setup();

        let vault_id = client.create_vault(&owner, &beneficiary, &86400, &None);
        let mut vault = client.get_vault(&vault_id);
        vault.status = ReleaseStatus::Released;

        // Different address archives the vault
        let archiver = Address::generate(&env);
        env.mock_all_auths();

        let result = client.try_archive_vault(&vault_id);
        assert!(
            result.is_ok(),
            "Anyone should be able to archive a released vault"
        );
    }
}
