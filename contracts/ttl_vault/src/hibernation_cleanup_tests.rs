/// Tests for Issue #1170 / #1327:
///
/// #1170 — the hibernation entry must be fully removed from storage after
/// `exit_hibernation`, not merely marked inactive.
///
/// #1327 — when a vault is released (or cancelled) while in hibernation state,
/// `trigger_release` must clear the hibernation entry so that
/// `get_hibernation` / `is_hibernating` can never return stale data and
/// ledger space is not wasted.

#[cfg(test)]
mod tests {
    use super::super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::StellarAssetClient,
        Address, Env,
    };

    fn setup() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let token_admin = Address::generate(&env);
        let token_address = env.register_stellar_asset_contract_v2(token_admin).address();
        // Mint enough for a deposit so trigger_release has a non-zero balance
        StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000i128);

        let contract_id = env.register_contract(None, TtlVaultContract);
        let client = TtlVaultContractClient::new(&env, &contract_id);
        client.initialize(&token_address, &admin);

        let client: TtlVaultContractClient<'static> =
            unsafe { core::mem::transmute(client) };
        (env, owner, beneficiary, client)
    }

    // ── Issue #1170 tests (exit_hibernation cleanup) ─────────────────────────

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

    // ── Issue #1327 tests (trigger_release clears hibernation) ───────────────

    /// After a successful `trigger_release`, the hibernation entry must be gone.
    #[test]
    fn test_hibernation_cleared_after_trigger_release() {
        let (env, owner, beneficiary, client) = setup();

        let interval = MIN_CHECK_IN_INTERVAL;
        let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

        // Deposit so the vault is non-empty (required for release)
        client.deposit(&vault_id, &owner, &1_000_000i128);

        // Enter hibernation before the TTL expires
        let hib_duration = 5_000u64;
        client.enter_hibernation(&vault_id, &owner, &hib_duration).unwrap();
        assert!(
            client.get_hibernation(&vault_id).is_some(),
            "hibernation entry should exist before release"
        );

        // Advance ledger past check-in interval + hibernation duration so TTL expires
        env.ledger().with_mut(|l| {
            l.timestamp = interval + hib_duration + 1;
        });

        // Release the vault
        client.trigger_release(&vault_id);

        // Hibernation entry must be cleared
        assert!(
            client.get_hibernation(&vault_id).is_none(),
            "hibernation entry must be removed from storage after trigger_release (Issue #1327)"
        );
    }

    /// `is_hibernating` must return false after a release even if the entry was
    /// never explicitly exited before the release.
    #[test]
    fn test_is_hibernating_false_after_trigger_release() {
        let (env, owner, beneficiary, client) = setup();

        let interval = MIN_CHECK_IN_INTERVAL;
        let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

        client.deposit(&vault_id, &owner, &1_000_000i128);

        let hib_duration = 5_000u64;
        client.enter_hibernation(&vault_id, &owner, &hib_duration).unwrap();

        // Advance ledger to expire the vault
        env.ledger().with_mut(|l| {
            l.timestamp = interval + hib_duration + 1;
        });

        client.trigger_release(&vault_id);

        // is_hibernating must also be false
        assert!(
            !client.is_hibernating(&vault_id),
            "is_hibernating must be false after trigger_release (Issue #1327)"
        );
    }

    /// `trigger_release` on a non-hibernating vault (no entry) must still succeed
    /// — the storage removal is a no-op and must not panic.
    #[test]
    fn test_trigger_release_without_hibernation_succeeds() {
        let (env, owner, beneficiary, client) = setup();

        let interval = MIN_CHECK_IN_INTERVAL;
        let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

        client.deposit(&vault_id, &owner, &1_000_000i128);

        // Do NOT enter hibernation
        assert!(
            client.get_hibernation(&vault_id).is_none(),
            "pre-condition: no hibernation entry"
        );

        // Advance ledger to expire
        env.ledger().with_mut(|l| {
            l.timestamp = interval + 1;
        });

        // Should succeed without panicking
        client.trigger_release(&vault_id);

        assert!(
            client.get_hibernation(&vault_id).is_none(),
            "no hibernation entry should exist after release of non-hibernating vault"
        );
    }
}
