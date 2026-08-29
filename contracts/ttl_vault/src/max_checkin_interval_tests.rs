/// Tests for maximum check-in interval enforcement (Issue #1165)
/// Ensures vaults cannot be created with intervals above MAX_CHECK_IN_INTERVAL (10 years),
/// independent of the optional admin-configured MaxCheckInInterval protocol setting.

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

        let xlm_token = Address::generate(&env);
        client.initialize(&xlm_token, &admin);

        (env, owner, beneficiary, client)
    }

    /// Test creating vault with exactly the maximum interval (10 years)
    /// Should succeed
    #[test]
    fn test_create_vault_with_exactly_maximum_interval() {
        let (env, owner, beneficiary, client) = setup();
        let result = client.try_create_vault(&owner, &beneficiary, &MAX_CHECK_IN_INTERVAL, &None);
        assert!(result.is_ok(), "Creating vault with exactly maximum interval should succeed");
    }

    /// Test creating vault with interval just above maximum
    /// Should fail with IntervalTooHigh error
    #[test]
    fn test_create_vault_with_interval_above_maximum() {
        let (env, owner, beneficiary, client) = setup();
        let above_max = MAX_CHECK_IN_INTERVAL + 1;
        let result = client.try_create_vault(&owner, &beneficiary, &above_max, &None);
        assert!(result.is_err(), "Creating vault with interval above maximum should fail");
        match result.unwrap_err().unwrap() {
            ContractError::IntervalTooHigh => {}
            e => panic!("Expected IntervalTooHigh, got {:?}", e),
        }
    }

    /// Test creating vault with an astronomically large interval (u64::MAX)
    /// Should fail with IntervalTooHigh rather than being accepted and later
    /// overflowing TTL/expiry arithmetic.
    #[test]
    fn test_create_vault_with_u64_max_interval_rejected() {
        let (env, owner, beneficiary, client) = setup();
        let result = client.try_create_vault(&owner, &beneficiary, &u64::MAX, &None);
        assert!(result.is_err(), "Creating vault with u64::MAX interval should fail");
        match result.unwrap_err().unwrap() {
            ContractError::IntervalTooHigh => {}
            e => panic!("Expected IntervalTooHigh, got {:?}", e),
        }
    }

    /// Test MAX_CHECK_IN_INTERVAL constant value (10 years in seconds)
    #[test]
    fn test_max_checkin_interval_is_ten_years() {
        assert_eq!(
            MAX_CHECK_IN_INTERVAL, 315_360_000,
            "MAX_CHECK_IN_INTERVAL should be 315,360,000 seconds (10 years)"
        );
    }

    /// Test that update_check_in_interval also rejects intervals above maximum
    #[test]
    fn test_update_check_in_interval_rejects_above_maximum() {
        let (env, owner, beneficiary, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
        let above_max = MAX_CHECK_IN_INTERVAL + 1;
        let result = client.try_update_check_in_interval(&vault_id, &owner, &above_max);
        assert!(result.is_err(), "update_check_in_interval should reject interval above maximum");
    }

    /// Test that update_check_in_interval accepts intervals at or below maximum
    #[test]
    fn test_update_check_in_interval_accepts_maximum_interval() {
        let (env, owner, beneficiary, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
        let result = client.try_update_check_in_interval(&vault_id, &owner, &MAX_CHECK_IN_INTERVAL);
        assert!(result.is_ok(), "update_check_in_interval should accept the maximum interval");
    }

    /// The hard-coded maximum applies even when no admin protocol config has ever
    /// been set (MaxCheckInInterval storage key absent) — this is the actual gap
    /// Issue #1165 closes, since assert_interval_in_bounds alone is a no-op until
    /// an admin explicitly proposes/applies a max via propose_protocol_config.
    #[test]
    fn test_maximum_enforced_without_any_admin_protocol_config() {
        let (env, owner, beneficiary, client) = setup();
        assert!(client.get_protocol_config().max_check_in_interval.is_none());

        let result = client.try_create_vault(&owner, &beneficiary, &(MAX_CHECK_IN_INTERVAL + 1), &None);
        assert!(
            result.is_err(),
            "Interval above the hard-coded maximum must be rejected even with no admin config set"
        );
    }
}
