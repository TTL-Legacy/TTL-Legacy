/// Tests for minimum check-in interval enforcement (Issue #1121)
/// Ensures vaults cannot be created with intervals shorter than MIN_CHECK_IN_INTERVAL (1 hour / 3600 seconds)

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
        let client = TtlVaultContractClient::new(&env, &contract_id);

        // Setup: initialize contract
        let xlm_token = Address::generate(&env);
        client.initialize(&xlm_token, &admin);

        (env, owner, beneficiary, client)
    }

    /// Test creating vault with exactly the minimum interval (3600 seconds / 1 hour)
    /// Should succeed
    #[test]
    fn test_create_vault_with_exactly_minimum_interval() {
        let (env, owner, beneficiary, client) = setup();
        let result = client.try_create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
        assert!(
            result.is_ok(),
            "Creating vault with exactly minimum interval should succeed"
        );
    }

    /// Test creating vault with interval just below minimum (3599 seconds)
    /// Should fail with CheckInIntervalTooShort error
    #[test]
    fn test_create_vault_with_interval_below_minimum() {
        let (env, owner, beneficiary, client) = setup();
        let below_min = MIN_CHECK_IN_INTERVAL - 1;
        let result = client.try_create_vault(&owner, &beneficiary, &below_min, &None);
        assert!(
            result.is_err(),
            "Creating vault with interval below minimum should fail"
        );
        match result.unwrap_err().unwrap() {
            ContractError::CheckInIntervalTooShort => {}
            e => panic!("Expected CheckInIntervalTooShort, got {:?}", e),
        }
    }

    /// Test creating vault with very small interval (1 second)
    /// Should fail with CheckInIntervalTooShort error
    #[test]
    fn test_create_vault_with_one_second_interval() {
        let (env, owner, beneficiary, client) = setup();
        let result = client.try_create_vault(&owner, &beneficiary, &1, &None);
        assert!(
            result.is_err(),
            "Creating vault with 1 second interval should fail"
        );
        match result.unwrap_err().unwrap() {
            ContractError::CheckInIntervalTooShort => {}
            e => panic!("Expected CheckInIntervalTooShort, got {:?}", e),
        }
    }

    /// Test creating vault with zero interval
    /// Should fail with InvalidInterval error (this is the first check)
    #[test]
    fn test_create_vault_with_zero_interval() {
        let (env, owner, beneficiary, client) = setup();
        let result = client.try_create_vault(&owner, &beneficiary, &0, &None);
        assert!(
            result.is_err(),
            "Creating vault with 0 interval should fail"
        );
        match result.unwrap_err().unwrap() {
            ContractError::InvalidInterval => {}
            e => panic!("Expected InvalidInterval, got {:?}", e),
        }
    }

    /// Test creating vault with well-above minimum interval (30 days / 2592000 seconds)
    /// Should succeed
    #[test]
    fn test_create_vault_with_well_above_minimum_interval() {
        let (env, owner, beneficiary, client) = setup();
        let thirty_days = 30u64 * 24u64 * 3600u64; // 2,592,000 seconds
        let result = client.try_create_vault(&owner, &beneficiary, &thirty_days, &None);
        assert!(
            result.is_ok(),
            "Creating vault with 30-day interval should succeed"
        );
    }

    /// Test creating vault with 2-hour interval (7200 seconds)
    /// Should succeed
    #[test]
    fn test_create_vault_with_two_hour_interval() {
        let (env, owner, beneficiary, client) = setup();
        let two_hours = 2u64 * 3600u64;
        let result = client.try_create_vault(&owner, &beneficiary, &two_hours, &None);
        assert!(
            result.is_ok(),
            "Creating vault with 2-hour interval should succeed"
        );
    }

    /// Test MIN_CHECK_IN_INTERVAL constant value
    /// Ensures it's set to 1 hour as expected
    #[test]
    fn test_min_checkin_interval_is_one_hour() {
        assert_eq!(
            MIN_CHECK_IN_INTERVAL, 3600,
            "MIN_CHECK_IN_INTERVAL should be 3600 seconds (1 hour)"
        );
    }

    /// Test that multiple vaults cannot be created with short intervals
    /// Each attempt should fail consistently
    #[test]
    fn test_multiple_short_interval_attempts_all_fail() {
        let (env, owner, beneficiary, client) = setup();

        // Try creating vaults with various short intervals
        let short_intervals = vec![1u64, 100, 500, 1000, 3599];

        for interval in short_intervals {
            let gen_beneficiary = Address::generate(&env);
            let result = client.try_create_vault(&owner, &gen_beneficiary, &interval, &None);
            assert!(
                result.is_err(),
                "Creating vault with interval {} should fail",
                interval
            );
        }
    }

    /// Test that update_check_in_interval also rejects intervals below minimum
    #[test]
    fn test_update_check_in_interval_rejects_below_minimum() {
        let (env, owner, beneficiary, client) = setup();
        // First create a valid vault
        let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
        // Try to update to an interval below minimum
        let result =
            client.try_update_check_in_interval(&vault_id, &owner, &(MIN_CHECK_IN_INTERVAL - 1));
        assert!(
            result.is_err(),
            "update_check_in_interval should reject interval below minimum"
        );
    }

    /// Test that update_check_in_interval accepts intervals at or above minimum
    #[test]
    fn test_update_check_in_interval_accepts_valid_interval() {
        let (env, owner, beneficiary, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
        let two_days = 2u64 * 86400u64;
        let result = client.try_update_check_in_interval(&vault_id, &owner, &two_days);
        assert!(
            result.is_ok(),
            "update_check_in_interval should accept 2-day interval"
        );
    }
}
