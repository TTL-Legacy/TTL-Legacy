/// Tests for Issue #1167: trigger_release must never perform a no-op transfer
/// or emit a release event for a vault with zero funds available to release.

#[cfg(test)]
mod tests {
    use super::super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::{self, StellarAssetClient},
        Address, Env,
    };

    fn setup() -> (Env, Address, Address, Address, TtlVaultContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let admin = Address::generate(&env);

        let token_admin = Address::generate(&env);
        let token_address = env.register_stellar_asset_contract_v2(token_admin).address();
        StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

        let contract_address = env.register_contract(None, TtlVaultContract);
        let client = TtlVaultContractClient::new(&env, &contract_address);
        client.initialize(&token_address, &admin);

        let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
        (env, owner, beneficiary, token_address, client)
    }

    /// A vault that was never funded must reject trigger_release with EmptyVault,
    /// rather than performing a zero-amount transfer and emitting a release event.
    #[test]
    fn test_trigger_release_on_never_funded_vault_fails() {
        let (env, owner, beneficiary, _token_address, client) = setup();
        let interval = 1_000u64;
        let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

        env.ledger().with_mut(|l| l.timestamp += interval + 1);
        assert!(client.is_expired(&vault_id));

        let result = client.try_trigger_release(&vault_id);
        assert!(result.is_err(), "trigger_release on an unfunded vault must fail");
        match result.unwrap_err().unwrap() {
            ContractError::EmptyVault => {}
            e => panic!("Expected EmptyVault, got {:?}", e),
        }

        // Status must remain Locked — no misleading "released" state for a vault
        // that never actually transferred anything.
        assert_eq!(client.get_vault(&vault_id).status, ReleaseStatus::Locked);
    }

    /// A vault fully drained via partial_release before expiry must also reject
    /// trigger_release rather than emitting a second, zero-amount release.
    #[test]
    fn test_trigger_release_on_fully_drained_vault_fails() {
        let (env, owner, beneficiary, token_address, client) = setup();
        let interval = 1_000u64;
        let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
        client.deposit(&vault_id, &owner, &200_000i128);

        client.partial_release(&vault_id, &200_000i128).unwrap();
        assert_eq!(client.get_vault(&vault_id).balance, 0);

        env.ledger().with_mut(|l| l.timestamp += interval + 1);

        let result = client.try_trigger_release(&vault_id);
        assert!(result.is_err(), "trigger_release on a fully-drained vault must fail");

        let token_client = token::Client::new(&env, &token_address);
        let balance_before = token_client.balance(&beneficiary);
        assert_eq!(balance_before, 200_000, "beneficiary already has the partial-release funds");
    }

    /// Sanity/regression: a properly funded, expired vault still releases normally.
    #[test]
    fn test_trigger_release_on_funded_vault_succeeds() {
        let (env, owner, beneficiary, token_address, client) = setup();
        let interval = 1_000u64;
        let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
        client.deposit(&vault_id, &owner, &500_000i128);

        env.ledger().with_mut(|l| l.timestamp += interval + 1);

        let token_client = token::Client::new(&env, &token_address);
        let before = token_client.balance(&beneficiary);
        client.trigger_release(&vault_id);
        assert_eq!(token_client.balance(&beneficiary) - before, 500_000);
    }
}
