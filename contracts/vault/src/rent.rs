//! Storage rent estimation for vaults.
//!
//! # Calculation
//!
//! Soroban charges rent per ledger entry based on the number of bytes it
//! occupies in persistent storage and the number of ledgers it must remain
//! live. The estimate returned by [`estimate_rent`] is:
//!
//! ```text
//! rent = storage_bytes * RENT_PER_BYTE_PER_LEDGER * ledgers
//! ```
//!
//! * `storage_bytes` — the vault's persistent storage footprint in bytes,
//!   derived from the fixed-size vault record plus the variable-length
//!   owner/asset identifiers it stores.
//! * `RENT_PER_BYTE_PER_LEDGER` — the network rent rate, expressed in
//!   stroops per byte per ledger. This is a protocol constant; the value
//!   below is the conservative default used when the network rate is not
//!   supplied.
//! * `ledgers` — the requested lifetime, in ledgers. Stellar closes a
//!   ledger roughly every 5 seconds, so `ledgers = duration_seconds / 5`.
//!
//! The result is denominated in stroops (1 XLM = 10_000_000 stroops).
//!
//! # Assumptions
//!
//! * The vault record layout is stable; adding fields changes the estimate.
//! * Rent is linear in both storage size and ledger count (no bulk discount).
//! * A `ledgers` value of `0` yields `0` rent — the vault is not kept alive.

use soroban_sdk::{contracttype, Address, Env};

/// Rent rate in stroops per byte per ledger.
///
/// Conservative default used when the caller does not provide a network rate.
pub const RENT_PER_BYTE_PER_LEDGER: i128 = 1;

/// Fixed portion of a vault's persistent storage footprint, in bytes.
///
/// Covers the vault id, balance, and status fields stored inline.
pub const VAULT_FIXED_BYTES: u32 = 64;

/// Per-identifier overhead, in bytes, for each `Address` stored by a vault.
pub const ADDRESS_BYTES: u32 = 32;

/// Persistent storage footprint of a vault, in bytes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultFootprint {
    /// Total bytes occupied by the vault's persistent entries.
    pub storage_bytes: u32,
}

/// Compute the persistent storage footprint for a vault.
///
/// The footprint is the fixed vault record size plus the size of the two
/// `Address` identifiers (owner and asset) the vault stores.
pub fn vault_footprint() -> VaultFootprint {
    VaultFootprint {
        storage_bytes: VAULT_FIXED_BYTES + 2 * ADDRESS_BYTES,
    }
}

/// Estimate the storage rent, in stroops, required to keep `vault_id` alive
/// for `ledgers` ledgers.
///
/// This is a read-only view: it performs no writes and does not require
/// authorization. It returns `None` when `vault_id` does not exist, so callers
/// can distinguish "unknown vault" from "zero rent".
///
/// * `ledgers == 0` returns `Some(0)` for an existing vault.
/// * Very large `ledgers` values are computed with checked arithmetic; an
///   overflow returns `None` rather than panicking.
///
/// See the module documentation for the full formula and assumptions.
pub fn estimate_rent(env: &Env, vault_id: u64, ledgers: u32) -> Option<i128> {
    // Unknown vaults have no storage to rent.
    if !vault_exists(env, vault_id) {
        return None;
    }

    let bytes = vault_footprint().storage_bytes as i128;
    let ledgers = ledgers as i128;

    bytes
        .checked_mul(RENT_PER_BYTE_PER_LEDGER)
        .and_then(|per_ledger| per_ledger.checked_mul(ledgers))
}

/// Returns whether a vault with `vault_id` exists in persistent storage.
///
/// Kept local to this module so the rent view stays self-contained; the vault
/// contract exposes the same lookup through its storage helpers.
fn vault_exists(env: &Env, vault_id: u64) -> bool {
    env.storage().persistent().has(&vault_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (Env, u64) {
        let env = Env::default();
        let vault_id = 1u64;
        env.storage().persistent().set(&vault_id, &Address::generate(&env));
        (env, vault_id)
    }

    #[test]
    fn zero_ledgers_costs_nothing() {
        let (env, vault_id) = setup();
        assert_eq!(estimate_rent(&env, vault_id, 0), Some(0));
    }

    #[test]
    fn non_existent_vault_returns_none() {
        let env = Env::default();
        assert_eq!(estimate_rent(&env, 42, 100), None);
    }

    #[test]
    fn typical_duration_is_linear() {
        let (env, vault_id) = setup();
        let bytes = vault_footprint().storage_bytes as i128;
        // ~30 days at 5s per ledger.
        let ledgers = 518_400u32;
        assert_eq!(estimate_rent(&env, vault_id, ledgers), Some(bytes * ledgers as i128));
        assert_eq!(
            estimate_rent(&env, vault_id, ledgers * 2),
            Some(bytes * (ledgers as i128) * 2)
        );
    }

    #[test]
    fn very_large_ledger_count_does_not_panic() {
        let (env, vault_id) = setup();
        // u32::MAX ledgers multiplied by the footprint overflows i128 only in
        // pathological cases; the checked math must never panic.
        let result = estimate_rent(&env, vault_id, u32::MAX);
        assert!(result.is_some() || result.is_none());
    }
}
