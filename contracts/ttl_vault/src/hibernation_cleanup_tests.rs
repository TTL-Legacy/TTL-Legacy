/// Tests for Issue #1170: the hibernation entry must be fully removed from
/// storage after exit_hibernation, not merely marked inactive, so a later
/// get_hibernation call can never return stale data.

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

    #[test]
    fn test_get_hibernation_returns_none_after_exit_hibernation() {
        let (env, owner, beneficiary, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

        client.enter_hibernation(&vault_id, &owner, &10_000u64).unwrap();
        assert!(client.get_hibernation(&vault_id).is_some());

        client.exit_hibernation(&vault_id, &owner).unwrap();

        assert!(
            client.get_hibernation(&vault_id).is_none(),
            "hibernation entry must be fully removed after exit_hibernation, not just marked inactive"
        );
    }

    /// A guard that checks hibernation state (e.g. is_hibernating) must also see
    /// the cleared state immediately after exit, not stale data from a
    /// merely-deactivated entry.
    #[test]
    fn test_vault_can_re_enter_hibernation_after_exiting() {
        let (env, owner, beneficiary, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

        client.enter_hibernation(&vault_id, &owner, &10_000u64).unwrap();
        client.exit_hibernation(&vault_id, &owner).unwrap();

        // If the old entry were merely deactivated rather than removed, this
        // would risk colliding with or being masked by stale state.
        let result = client.try_enter_hibernation(&vault_id, &owner, &5_000u64);
        assert!(result.is_ok(), "should be able to re-enter hibernation after a clean exit");
        assert!(client.get_hibernation(&vault_id).is_some());
    }
}
