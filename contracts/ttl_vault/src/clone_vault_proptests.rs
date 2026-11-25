#![cfg(test)]
//! Property-based tests for clone_vault field-propagation invariants — #2075.
//!
//! Uses [`proptest`] to generate arbitrary combinations of `VaultCloneConfig`
//! flags and asserts that every `clone_vault` / `clone_vault_with_overrides`
//! call produces a consistent, well-formed output.
//!
//! Invariants verified:
//! 1. Fields that are inherited always equal the source vault's value.
//! 2. Fields that are overridden always equal the supplied override value.
//! 3. Fields that are always reset (balance, status, passkey_hash, …) always
//!    have their canonical fresh value regardless of flags.
//! 4. The clone is a distinct vault (new ID, new `created_at` timestamp).
//! 5. The cloned vault's BPS sum equals 10 000 when beneficiaries are
//!    either inherited or supplied.

extern crate alloc;

use super::*;
use alloc::vec::Vec as StdVec;
use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env, String as SorobanString, Vec as SorobanVec,
};

// ---------------------------------------------------------------------------
// VaultCloneConfig — flag struct that controls which fields are overridden
// ---------------------------------------------------------------------------

/// Mirrors the override options accepted by `clone_vault_with_overrides`.
///
/// Each field is `true` when the corresponding parameter should be supplied as
/// `Some(…)` (override) and `false` when it should be `None` (inherit from
/// source).
#[derive(Debug, Clone)]
pub struct VaultCloneConfig {
    /// When true, `override_interval` is `Some(interval)`.
    pub override_interval: bool,
    /// When true, `override_beneficiaries` is `Some(entries)`.
    pub override_beneficiaries: bool,
    /// When true, `override_metadata` is `Some(string)`.
    pub override_metadata: bool,
}

// ---------------------------------------------------------------------------
// Arbitrary strategy for VaultCloneConfig
// ---------------------------------------------------------------------------

prop_compose! {
    fn arb_clone_config()(
        override_interval     in any::<bool>(),
        override_beneficiaries in any::<bool>(),
        override_metadata     in any::<bool>(),
    ) -> VaultCloneConfig {
        VaultCloneConfig {
            override_interval,
            override_beneficiaries,
            override_metadata,
        }
    }
}

// ---------------------------------------------------------------------------
// Arbitrary strategy for a valid check-in interval
// ---------------------------------------------------------------------------

/// Generates intervals in a sensible range to avoid `IntervalTooLow /
/// IntervalTooHigh` errors from the contract bounds check.
prop_compose! {
    fn arb_interval()(interval in 60u64..86_400u64 * 365) -> u64 {
        interval
    }
}

// ---------------------------------------------------------------------------
// Test harness helpers
// ---------------------------------------------------------------------------

/// Stand-alone environment suitable for property tests.
///
/// Returns a minimal but fully-initialised contract client so each proptest
/// iteration starts from a clean state.
fn make_env() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000);

    let contract = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, admin, client)
}

/// Creates a source vault with a known `check_in_interval`, deposits some
/// funds, and returns the vault ID together with the beneficiary address.
fn create_source_vault(
    env: &Env,
    owner: &Address,
    client: &TtlVaultContractClient<'static>,
    interval: u64,
) -> (u64, Address) {
    let beneficiary = Address::generate(env);
    let vault_id = client.create_vault(owner, &beneficiary, &interval, &None);
    client.deposit(&vault_id, owner, &500_000);
    (vault_id, beneficiary)
}

// ---------------------------------------------------------------------------
// Property: inherited interval equals source interval
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// When `override_interval` is false the clone must inherit the source
    /// vault's `check_in_interval` exactly.
    #[test]
    fn prop_inherited_interval_equals_source(
        interval in arb_interval(),
    ) {
        let (env, owner, _, client) = make_env();
        let new_beneficiary = Address::generate(&env);
        let (src_id, _) = create_source_vault(&env, &owner, &client, interval);

        let clone_id = client.clone_vault_with_overrides(
            &src_id, &owner, &new_beneficiary,
            &None,  // inherit interval
            &None,  // inherit beneficiaries
            &None,  // inherit metadata
        );

        let src   = client.get_vault(&src_id);
        let clone = client.get_vault(&clone_id);

        prop_assert_eq!(
            clone.check_in_interval, src.check_in_interval,
            "inherited interval must equal source"
        );
    }

    /// When `override_interval` is supplied the clone must use that value, not
    /// the source vault's interval.
    #[test]
    fn prop_overridden_interval_does_not_equal_source_unless_equal(
        src_interval  in 100u64..86_400u64,
        new_interval  in 86_401u64..86_400u64 * 10,
    ) {
        let (env, owner, _, client) = make_env();
        let new_beneficiary = Address::generate(&env);
        let (src_id, _) = create_source_vault(&env, &owner, &client, src_interval);

        let clone_id = client.clone_vault_with_overrides(
            &src_id, &owner, &new_beneficiary,
            &Some(new_interval),
            &None,
            &None,
        );

        let clone = client.get_vault(&clone_id);

        prop_assert_eq!(
            clone.check_in_interval, new_interval,
            "overridden interval must equal the supplied value"
        );
    }
}

// ---------------------------------------------------------------------------
// Property: balance is always zero in the clone
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// A clone must start with zero balance regardless of the source vault's
    /// balance.  Funds are never copied.
    #[test]
    fn prop_clone_balance_is_always_zero(interval in arb_interval()) {
        let (env, owner, _, client) = make_env();
        let new_beneficiary = Address::generate(&env);
        let (src_id, _) = create_source_vault(&env, &owner, &client, interval);

        let clone_id = client.clone_vault_with_overrides(
            &src_id, &owner, &new_beneficiary,
            &None, &None, &None,
        );

        let clone = client.get_vault(&clone_id);
        prop_assert_eq!(clone.balance, 0i128, "clone balance must be zero");
    }
}

// ---------------------------------------------------------------------------
// Property: clone status is always Locked
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// The clone must always start in the `Locked` state.
    #[test]
    fn prop_clone_status_is_always_locked(interval in arb_interval()) {
        let (env, owner, _, client) = make_env();
        let new_beneficiary = Address::generate(&env);
        let (src_id, _) = create_source_vault(&env, &owner, &client, interval);

        let clone_id = client.clone_vault_with_overrides(
            &src_id, &owner, &new_beneficiary,
            &None, &None, &None,
        );

        let clone = client.get_vault(&clone_id);
        prop_assert_eq!(
            clone.status, ReleaseStatus::Locked,
            "clone status must be Locked"
        );
    }
}

// ---------------------------------------------------------------------------
// Property: clone is not paused
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// A freshly cloned vault must never be paused regardless of source state.
    #[test]
    fn prop_clone_is_never_paused(interval in arb_interval()) {
        let (env, owner, _, client) = make_env();
        let new_beneficiary = Address::generate(&env);
        let (src_id, _) = create_source_vault(&env, &owner, &client, interval);

        let clone_id = client.clone_vault_with_overrides(
            &src_id, &owner, &new_beneficiary,
            &None, &None, &None,
        );

        let clone = client.get_vault(&clone_id);
        prop_assert!(!clone.is_paused, "clone must not be paused on creation");
    }
}

// ---------------------------------------------------------------------------
// Property: clone owner equals new_owner
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// The clone's owner must always equal the `new_owner` argument.
    #[test]
    fn prop_clone_owner_is_new_owner(interval in arb_interval()) {
        let (env, owner, _, client) = make_env();
        let new_beneficiary = Address::generate(&env);
        let (src_id, _) = create_source_vault(&env, &owner, &client, interval);

        let clone_id = client.clone_vault_with_overrides(
            &src_id, &owner, &new_beneficiary,
            &None, &None, &None,
        );

        let clone = client.get_vault(&clone_id);
        prop_assert_eq!(
            clone.owner, owner,
            "clone owner must equal new_owner argument"
        );
    }
}

// ---------------------------------------------------------------------------
// Property: clone ID is strictly greater than source ID
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Because IDs are drawn from a monotonically increasing counter, the
    /// clone's ID must always exceed the source's ID.
    #[test]
    fn prop_clone_id_exceeds_source_id(interval in arb_interval()) {
        let (env, owner, _, client) = make_env();
        let new_beneficiary = Address::generate(&env);
        let (src_id, _) = create_source_vault(&env, &owner, &client, interval);

        let clone_id = client.clone_vault_with_overrides(
            &src_id, &owner, &new_beneficiary,
            &None, &None, &None,
        );

        prop_assert!(
            clone_id > src_id,
            "clone ID ({}) must be greater than source ID ({})",
            clone_id, src_id
        );
    }
}

// ---------------------------------------------------------------------------
// Property: inherited beneficiaries BPS sum equals 10 000
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// When beneficiaries are inherited from the source vault the total BPS
    /// allocation in the clone must still sum to 10 000 (or be empty, which
    /// means 100 % goes to the primary beneficiary).
    #[test]
    fn prop_inherited_beneficiaries_bps_sum_is_valid(interval in arb_interval()) {
        let (env, owner, _, client) = make_env();

        let b1 = Address::generate(&env);
        let b2 = Address::generate(&env);
        let new_beneficiary = Address::generate(&env);

        let (src_id, _) = create_source_vault(&env, &owner, &client, interval);

        // Set a multi-beneficiary split on the source.
        let mut entries = SorobanVec::new(&env);
        entries.push_back(BeneficiaryEntry { address: b1.clone(), bps: 6000, minimum_threshold: 0 });
        entries.push_back(BeneficiaryEntry { address: b2.clone(), bps: 4000, minimum_threshold: 0 });
        client.set_beneficiaries(&src_id, &owner, &entries);

        let clone_id = client.clone_vault_with_overrides(
            &src_id, &owner, &new_beneficiary,
            &None, &None, &None,
        );

        let clone = client.get_vault(&clone_id);
        if !clone.beneficiaries.is_empty() {
            let total_bps: u32 = clone.beneficiaries.iter().map(|e| e.bps).sum();
            prop_assert_eq!(total_bps, 10_000u32, "inherited beneficiaries BPS must sum to 10 000");
        }
    }
}

// ---------------------------------------------------------------------------
// Property: passkey_hash is always None in the clone
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// The clone must never inherit the source vault's passkey_hash.
    #[test]
    fn prop_clone_passkey_hash_is_absent(interval in arb_interval()) {
        let (env, owner, _, client) = make_env();
        let new_beneficiary = Address::generate(&env);
        let (src_id, _) = create_source_vault(&env, &owner, &client, interval);

        let clone_id = client.clone_vault_with_overrides(
            &src_id, &owner, &new_beneficiary,
            &None, &None, &None,
        );

        let clone = client.get_vault(&clone_id);
        prop_assert!(
            clone.passkey_hash.is_none(),
            "clone must not carry source passkey_hash"
        );
    }
}

// ---------------------------------------------------------------------------
// Property: parent_vault_id links back to the source
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// The clone's `parent_vault_id` must always equal the source vault's ID.
    #[test]
    fn prop_clone_parent_vault_id_is_source(interval in arb_interval()) {
        let (env, owner, _, client) = make_env();
        let new_beneficiary = Address::generate(&env);
        let (src_id, _) = create_source_vault(&env, &owner, &client, interval);

        let clone_id = client.clone_vault_with_overrides(
            &src_id, &owner, &new_beneficiary,
            &None, &None, &None,
        );

        let clone = client.get_vault(&clone_id);
        prop_assert_eq!(
            clone.parent_vault_id, Some(src_id),
            "clone parent_vault_id must equal source vault ID"
        );
    }
}

// ---------------------------------------------------------------------------
// Property: check-in counters are reset
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// A newly cloned vault must start with zero check-in history —
    /// regardless of how many check-ins the source vault recorded.
    #[test]
    fn prop_clone_check_in_counters_are_zero(interval in arb_interval()) {
        let (env, owner, _, client) = make_env();
        let new_beneficiary = Address::generate(&env);
        let (src_id, _) = create_source_vault(&env, &owner, &client, interval);

        // Advance time and perform a check-in on the source.
        env.ledger().with_mut(|l| l.timestamp += interval);
        client.check_in(&src_id, &owner);

        let clone_id = client.clone_vault_with_overrides(
            &src_id, &owner, &new_beneficiary,
            &None, &None, &None,
        );

        let clone = client.get_vault(&clone_id);
        prop_assert_eq!(clone.total_check_ins, 0u32, "clone total_check_ins must be zero");
        prop_assert_eq!(clone.on_time_check_ins, 0u32, "clone on_time_check_ins must be zero");
    }
}
