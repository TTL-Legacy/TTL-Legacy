/// Dedicated tests for bug fixes in issues #1264, #1265, #1266, #1267.
///
/// #1264 — create_vault must reject a zero check-in interval.
/// #1265 — deposit must reject a zero (or negative) amount.
/// #1266 — withdraw must reject an amount exceeding the vault balance.
/// #1267 — trigger_release must not execute while the vault TTL has not expired.
///
/// All tests use a check-in interval of MIN_CHECK_IN_INTERVAL (3600 s) or above
/// so that the minimum-interval guard does not interfere with the scenarios under
/// test.
///
/// Notes on trigger_release flow (#1267):
/// trigger_release_internal first checks release conditions stored in persistent
/// storage (`get_release_conditions`).  For a freshly created vault no conditions
/// are stored, so the vector is empty and `ConditionsNotApproved` fires before
/// the explicit `NotExpired` guard.  To reach the `NotExpired` guard a
/// `TTLExpiry` condition must be registered via `set_release_conditions`.
/// Both code-paths confirm that a non-expired vault can never be released.
#[cfg(test)]
mod tests {
    use super::super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::StellarAssetClient,
        vec, Address, Env,
    };

    // ──────────────────────────────────────────────────────────────────────────
    // Shared test helpers
    // ──────────────────────────────────────────────────────────────────────────

    /// Minimum valid check-in interval — mirrors the constant defined in lib.rs.
    const INTERVAL: u64 = MIN_CHECK_IN_INTERVAL; // 3 600 s

    fn setup() -> (
        Env,
        Address,
        Address,
        Address,
        TtlVaultContractClient<'static>,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let token_admin = Address::generate(&env);
        let token_address = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();

        // Mint enough tokens for all deposit scenarios.
        StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000i128);

        let contract_address = env.register_contract(None, TtlVaultContract);
        let client = TtlVaultContractClient::new(&env, &contract_address);
        client.initialize(&token_address, &admin);

        // SAFETY: lifetime extension is the standard pattern used throughout this
        // test suite.  The client is never accessed after `env` is dropped.
        let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

        (env, owner, beneficiary, token_address, client)
    }

    // ──────────────────────────────────────────────────────────────────────────
    // #1264 — create_vault: zero check-in interval must be rejected
    // ──────────────────────────────────────────────────────────────────────────

    /// Passing `check_in_interval = 0` must return `ContractError::InvalidInterval`
    /// (error code 2).
    /// A zero interval would cause the vault to expire immediately on creation,
    /// enabling instant (unintended) automatic release of funds.
    #[test]
    fn test_1264_create_vault_zero_interval_rejected() {
        let (_, owner, beneficiary, _, client) = setup();

        let result = client.try_create_vault(&owner, &beneficiary, &0u64, &None);
        assert!(result.is_err(), "create_vault with interval=0 must fail");

        let err = result.unwrap_err().unwrap();
        assert_eq!(
            err,
            soroban_sdk::Error::from_contract_error(2), // InvalidInterval = 2
            "expected ContractError::InvalidInterval (2) for zero interval"
        );
    }

    /// Verify the guard fires on zero but not on a valid positive interval.
    /// Creating a vault with `check_in_interval = INTERVAL` (1 h) must succeed.
    #[test]
    fn test_1264_create_vault_valid_interval_succeeds() {
        let (_, owner, beneficiary, _, client) = setup();

        let result = client.try_create_vault(&owner, &beneficiary, &INTERVAL, &None);
        assert!(
            result.is_ok(),
            "create_vault with interval={INTERVAL} must succeed"
        );
    }

    /// A rejected zero-interval call must not increment the vault counter,
    /// confirming no partial state was written before the guard fired.
    #[test]
    fn test_1264_vault_count_unchanged_after_zero_interval_failure() {
        let (_, owner, beneficiary, _, client) = setup();

        let count_before = client.get_vault_count();
        let _ = client.try_create_vault(&owner, &beneficiary, &0u64, &None);
        let count_after = client.get_vault_count();

        assert_eq!(
            count_before, count_after,
            "vault count must not change when create_vault is rejected"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // #1265 — deposit: zero or negative amount must be rejected
    // ──────────────────────────────────────────────────────────────────────────

    /// Depositing exactly zero must return `ContractError::InvalidAmount` (code 5).
    /// A no-op deposit still emits events and burns transaction fees.
    #[test]
    fn test_1265_deposit_zero_amount_rejected() {
        let (_, owner, beneficiary, _, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);

        let result = client.try_deposit(&vault_id, &owner, &0i128);
        assert!(result.is_err(), "deposit with amount=0 must fail");

        let err = result.unwrap_err().unwrap();
        assert_eq!(
            err,
            soroban_sdk::Error::from_contract_error(5), // InvalidAmount = 5
            "expected ContractError::InvalidAmount (5) for zero deposit"
        );
    }

    /// Depositing a negative amount must also return `ContractError::InvalidAmount`
    /// (code 5).
    #[test]
    fn test_1265_deposit_negative_amount_rejected() {
        let (_, owner, beneficiary, _, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);

        let result = client.try_deposit(&vault_id, &owner, &-1i128);
        assert!(result.is_err(), "deposit with amount=-1 must fail");

        let err = result.unwrap_err().unwrap();
        assert_eq!(
            err,
            soroban_sdk::Error::from_contract_error(5), // InvalidAmount = 5
            "expected ContractError::InvalidAmount (5) for negative deposit"
        );
    }

    /// A zero-amount deposit must not mutate the vault balance.
    #[test]
    fn test_1265_vault_balance_unchanged_after_zero_deposit_failure() {
        let (_, owner, beneficiary, _, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);

        // Seed the vault with a known positive balance first.
        client.deposit(&vault_id, &owner, &1_000i128);
        let balance_before = client.get_vault(&vault_id).balance;

        let _ = client.try_deposit(&vault_id, &owner, &0i128);
        let balance_after = client.get_vault(&vault_id).balance;

        assert_eq!(
            balance_before, balance_after,
            "vault balance must not change when zero-amount deposit is rejected"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // #1266 — withdraw: amount exceeding vault balance must be rejected
    // ──────────────────────────────────────────────────────────────────────────

    /// Attempting to withdraw more than the current vault balance must return
    /// `ContractError::InsufficientBalance` (code 8) instead of underflowing.
    #[test]
    fn test_1266_withdraw_exceeding_balance_rejected() {
        let (_, owner, beneficiary, _, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);

        // Deposit a small amount, then try to withdraw more.
        client.deposit(&vault_id, &owner, &500i128);

        let result = client.try_withdraw(&vault_id, &owner, &501i128, &None, &None, &None);
        assert!(result.is_err(), "withdraw exceeding balance must fail");

        let err = result.unwrap_err().unwrap();
        assert_eq!(
            err,
            soroban_sdk::Error::from_contract_error(8), // InsufficientBalance = 8
            "expected ContractError::InsufficientBalance (8) for over-withdrawal"
        );
    }

    /// Withdrawing the exact balance must succeed (boundary check — off-by-one safety).
    #[test]
    fn test_1266_withdraw_exact_balance_succeeds() {
        let (_, owner, beneficiary, _, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);

        client.deposit(&vault_id, &owner, &500i128);

        let result = client.try_withdraw(&vault_id, &owner, &500i128, &None, &None, &None);
        assert!(result.is_ok(), "withdraw of exact balance must succeed");
    }

    /// After a rejected over-withdrawal the vault balance must remain intact —
    /// no partial deduction should occur.
    #[test]
    fn test_1266_vault_balance_unchanged_after_over_withdrawal_failure() {
        let (_, owner, beneficiary, _, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);

        client.deposit(&vault_id, &owner, &300i128);
        let balance_before = client.get_vault(&vault_id).balance;

        let _ = client.try_withdraw(&vault_id, &owner, &301i128, &None, &None, &None);
        let balance_after = client.get_vault(&vault_id).balance;

        assert_eq!(
            balance_before, balance_after,
            "vault balance must not change when over-withdrawal is rejected"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // #1267 — trigger_release: must not execute before vault TTL has expired
    // ──────────────────────────────────────────────────────────────────────────

    /// When release conditions include `TTLExpiry`, calling `trigger_release` on
    /// a non-expired vault must return `ContractError::NotExpired` (code 16).
    ///
    /// This directly exercises the expiry guard added for issue #1267.
    #[test]
    fn test_1267_trigger_release_before_expiry_returns_not_expired() {
        let (env, owner, beneficiary, _, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);

        // Deposit so the EmptyVault guard does not fire first.
        client.deposit(&vault_id, &owner, &1_000i128);

        // Register TTLExpiry as the release condition so the expiry check is reached.
        let conditions = vec![&env, ReleaseCondition::TTLExpiry];
        client.set_release_conditions(&vault_id, &owner, &conditions);

        // Vault was just created — it has not expired yet.
        let result = client.try_trigger_release(&vault_id);
        assert!(
            result.is_err(),
            "trigger_release must fail before TTL expiry"
        );
        let err = result.unwrap_err().unwrap();
        assert_eq!(
            err,
            soroban_sdk::Error::from_contract_error(16), // NotExpired = 16
            "expected ContractError::NotExpired (16) for premature trigger_release"
        );
    }

    /// Without any explicit release conditions stored, an attempt to release a
    /// non-expired vault must fail with `ContractError::ConditionsNotApproved`
    /// (code 33).  This confirms that the release pipeline rejects non-expired
    /// vaults regardless of whether conditions are set explicitly.
    #[test]
    fn test_1267_trigger_release_no_conditions_before_expiry_rejected() {
        let (_, owner, beneficiary, _, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);

        // Deposit so the EmptyVault guard does not fire first.
        client.deposit(&vault_id, &owner, &1_000i128);

        // No release conditions set — empty conditions → ConditionsNotApproved.
        let result = client.try_trigger_release(&vault_id);
        assert!(
            result.is_err(),
            "trigger_release must fail when vault has not expired (no conditions set)"
        );
    }

    /// After the vault TTL elapses, `trigger_release` with a `TTLExpiry` condition
    /// must succeed, confirming the expiry check works in both directions.
    #[test]
    fn test_1267_trigger_release_after_expiry_succeeds() {
        let (env, owner, beneficiary, _, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);

        client.deposit(&vault_id, &owner, &1_000i128);

        // Register TTLExpiry as the release condition.
        let conditions = vec![&env, ReleaseCondition::TTLExpiry];
        client.set_release_conditions(&vault_id, &owner, &conditions);

        // Advance past the check-in interval so the vault is expired.
        env.ledger().with_mut(|l| l.timestamp += INTERVAL + 1);

        let result = client.try_trigger_release(&vault_id);
        assert!(
            result.is_ok(),
            "trigger_release must succeed after TTL expiry"
        );
    }

    /// Calling `trigger_release` when there is still one second left on the TTL
    /// must fail — the vault is not expired until the full interval has elapsed.
    #[test]
    fn test_1267_trigger_release_one_second_before_expiry_rejected() {
        let (env, owner, beneficiary, _, client) = setup();
        let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);

        client.deposit(&vault_id, &owner, &1_000i128);

        let conditions = vec![&env, ReleaseCondition::TTLExpiry];
        client.set_release_conditions(&vault_id, &owner, &conditions);

        // Advance to one second before the interval ends.
        env.ledger().with_mut(|l| l.timestamp += INTERVAL - 1);

        let result = client.try_trigger_release(&vault_id);
        assert!(
            result.is_err(),
            "trigger_release must fail when vault has not yet fully expired"
        );
        let err = result.unwrap_err().unwrap();
        assert_eq!(
            err,
            soroban_sdk::Error::from_contract_error(16), // NotExpired = 16
            "expected ContractError::NotExpired (16) one second before expiry"
        );
    }
}
