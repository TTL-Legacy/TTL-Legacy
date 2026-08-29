/// Tests for the lightweight get_last_check_in query (Issue #1166)

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

    /// get_last_check_in reflects the vault's creation timestamp immediately after creation.
    #[test]
    fn test_get_last_check_in_matches_creation_time() {
        let (env, owner, beneficiary, client) = setup();
        env.ledger().with_mut(|l| l.timestamp = 1_000);

        let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

        assert_eq!(client.get_last_check_in(&vault_id), 1_000);
    }

    /// get_last_check_in advances after an explicit check_in call.
    #[test]
    fn test_get_last_check_in_updates_after_check_in() {
        let (env, owner, beneficiary, client) = setup();
        env.ledger().with_mut(|l| l.timestamp = 1_000);
        let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

        env.ledger().with_mut(|l| l.timestamp = 5_000);
        client
            .check_in(&vault_id, &owner, &BytesN::from_array(&env, &[1u8; 32]), &0u64)
            .unwrap();

        assert_eq!(client.get_last_check_in(&vault_id), 5_000);
    }

    /// get_last_check_in agrees with the equivalent full-record field, since it's
    /// meant purely as a lighter-weight alias, not a different source of truth.
    #[test]
    fn test_get_last_check_in_matches_get_vault_last_check_in() {
        let (env, owner, beneficiary, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

        assert_eq!(
            client.get_last_check_in(&vault_id),
            client.get_vault_last_check_in(&vault_id)
        );
    }
}
