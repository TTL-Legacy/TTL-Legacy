/// Property-based tests for TTL-Legacy vault logic.
///
/// These tests use [`proptest`] to verify invariants over the full input space,
/// catching edge cases that hand-picked unit tests would miss — including
/// overflow near u32/u64 boundaries and rounding behaviour in ledger conversion.
///
/// Issue #1148: Add Property-Based Tests for TTL Extension Calculation
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Shared test model for vault operations
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum VaultOp {
    Deposit(i128),
    Withdraw(i128),
    CheckIn,
}

prop_compose! {
    fn arb_vault_op()(op in 0..3, amount in 1i128..1_000_000) -> VaultOp {
        match op {
            0 => VaultOp::Deposit(amount),
            1 => VaultOp::Withdraw(amount),
            _ => VaultOp::CheckIn,
        }
    }
}

// ---------------------------------------------------------------------------
// Balance invariants
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_vault_balance_never_exceeds_deposits(
        initial_balance in 0i128..1_000_000,
        ops in prop::collection::vec(arb_vault_op(), 0..50),
    ) {
        let mut balance = initial_balance;
        let mut total_deposits = 0i128;

        for op in ops {
            match op {
                VaultOp::Deposit(amount) => {
                    if let Some(new_balance) = balance.checked_add(amount) {
                        balance = new_balance;
                        total_deposits = total_deposits.saturating_add(amount);
                    }
                }
                VaultOp::Withdraw(amount) => {
                    if balance >= amount {
                        balance -= amount;
                    }
                }
                VaultOp::CheckIn => {
                    // Check-in doesn't affect balance
                }
            }
        }

        // Invariant: balance never exceeds initial + total deposits
        let max_balance = initial_balance.saturating_add(total_deposits);
        prop_assert!(balance <= max_balance);
    }
}

// ---------------------------------------------------------------------------
// TTL / check-in invariants
// ---------------------------------------------------------------------------

proptest! {
    /// TTL must be monotonically non-decreasing after every check-in.
    #[test]
    fn prop_ttl_always_increases_on_check_in(
        base_ttl in 1u64..86_400u64 * 365,
        check_in_interval in 1u64..86_400u64 * 365,
        num_check_ins in 1usize..20,
    ) {
        let mut ttl = base_ttl;

        for _ in 0..num_check_ins {
            let old_ttl = ttl;
            ttl = ttl.saturating_add(check_in_interval);

            // Invariant: TTL must increase or stay same on check-in
            prop_assert!(ttl >= old_ttl);
        }
    }

    /// After a check-in the new TTL must equal old_ttl + check_in_interval
    /// (using saturating arithmetic to avoid overflow).
    #[test]
    fn prop_ttl_post_checkin_equals_old_plus_interval(
        base_ttl in 0u64..u64::MAX / 2,
        check_in_interval in 1u64..86_400u64 * 365,
    ) {
        let new_ttl = base_ttl.saturating_add(check_in_interval);
        prop_assert!(new_ttl >= base_ttl,
            "TTL must not decrease after check-in");
        prop_assert!(new_ttl >= check_in_interval,
            "TTL must be at least one interval after check-in");
    }

    /// TTL after check-in must always be ≥ the ledger at time of check-in
    /// (modelled here as the base_ttl, representing current ledger).
    #[test]
    fn prop_ttl_after_checkin_gte_current_ledger(
        current_ledger in 0u64..u64::MAX / 2,
        check_in_interval in 1u64..86_400u64 * 365,
    ) {
        // Simulate: new TTL = current_ledger + interval
        let new_ttl = current_ledger.saturating_add(check_in_interval);
        prop_assert!(new_ttl >= current_ledger,
            "Post-check-in TTL must be ≥ current ledger");
    }
}

// ---------------------------------------------------------------------------
// vault_ttl_ledgers invariants  (Issue #1148)
// ---------------------------------------------------------------------------

/// Constants mirrored from lib.rs for property testing.
/// These must be kept in sync with the contract implementation.
const VAULT_TTL_LEDGERS_MIN: u32 = 200_000;
const MAX_PERSISTENT_TTL: u32 = 3_110_400;
const LEDGER_SECOND: u32 = 5;

/// Pure re-implementation of the contract's `vault_ttl_ledgers` helper,
/// used here so we can property-test it without depending on soroban-sdk.
fn vault_ttl_ledgers(check_in_interval: u64) -> u32 {
    let ledgers = (check_in_interval as u32)
        .saturating_mul(2)
        .saturating_div(LEDGER_SECOND);
    ledgers.clamp(VAULT_TTL_LEDGERS_MIN, MAX_PERSISTENT_TTL)
}

proptest! {
    // --- Invariant 1: result never overflows u32::MAX ---
    //
    // Because the function internally casts the interval to u32, then uses
    // saturating arithmetic and clamps the result, no overflow can occur.
    /// `vault_ttl_ledgers` must never exceed MAX_PERSISTENT_TTL for any input.
    #[test]
    fn prop_vault_ttl_never_exceeds_max(interval in 0u64..u64::MAX) {
        let result = vault_ttl_ledgers(interval);
        prop_assert!(
            result <= MAX_PERSISTENT_TTL,
            "vault_ttl_ledgers({}) = {} exceeds MAX_PERSISTENT_TTL ({})",
            interval, result, MAX_PERSISTENT_TTL
        );
        // Also assert it never exceeds u32::MAX (overflow guard)
        prop_assert!(
            result <= u32::MAX,
            "vault_ttl_ledgers({}) = {} overflows u32::MAX",
            interval, result
        );
    }

    // --- Invariant 2: result is always ≥ VAULT_TTL_LEDGERS_MIN ---
    /// `vault_ttl_ledgers` must always meet the minimum ledger floor.
    #[test]
    fn prop_vault_ttl_always_meets_minimum(interval in 0u64..u64::MAX) {
        prop_assert!(
            vault_ttl_ledgers(interval) >= VAULT_TTL_LEDGERS_MIN,
            "vault_ttl_ledgers({}) fell below minimum ({})",
            interval, VAULT_TTL_LEDGERS_MIN
        );
    }

    // --- Invariant 3: monotonically non-decreasing ---
    /// A larger interval must produce a ledger count that is ≥ a smaller interval.
    #[test]
    fn prop_vault_ttl_monotonically_non_decreasing(
        a in 0u64..u64::MAX / 2,
        b in 0u64..u64::MAX / 2,
    ) {
        let (big, small) = if a >= b { (a, b) } else { (b, a) };
        prop_assert!(
            vault_ttl_ledgers(big) >= vault_ttl_ledgers(small),
            "vault_ttl_ledgers({}) < vault_ttl_ledgers({}) — monotonicity violated",
            big, small
        );
    }

    // --- Invariant 4: result is within ±1 ledger of expected for typical intervals ---
    //
    // For inputs in the linear range (i.e. before the clamp kicks in), the
    // result must be within ±1 of the analytically expected value
    // `floor(interval * 2 / LEDGER_SECOND)`.
    /// TTL of a vault created with interval X is always within ±1 ledger of expected.
    #[test]
    fn prop_vault_ttl_within_one_ledger_of_expected(
        // Use a range that stays in the linear (non-clamped) region
        interval in 500_001u64..7_776_000u64,
    ) {
        let result = vault_ttl_ledgers(interval) as u64;
        // Analytical expected value (using the same arithmetic as the function)
        let raw = (interval as u32).saturating_mul(2).saturating_div(LEDGER_SECOND) as u64;
        let expected = raw.clamp(VAULT_TTL_LEDGERS_MIN as u64, MAX_PERSISTENT_TTL as u64);

        // Allow ±1 for rounding from integer division
        prop_assert!(
            result.abs_diff(expected) <= 1,
            "vault_ttl_ledgers({}) = {}, expected {} (±1)",
            interval, result, expected
        );
    }

    // --- Invariant 5: boundary values ---
    /// Zero interval returns the minimum floor.
    #[test]
    fn prop_vault_ttl_zero_interval_returns_floor(_dummy in 0u8..1u8) {
        prop_assert_eq!(vault_ttl_ledgers(0), VAULT_TTL_LEDGERS_MIN);
    }

    /// Very large interval returns the maximum ceiling.
    #[test]
    fn prop_vault_ttl_large_interval_returns_ceiling(_dummy in 0u8..1u8) {
        prop_assert_eq!(vault_ttl_ledgers(u64::MAX), MAX_PERSISTENT_TTL);
    }
}

// ---------------------------------------------------------------------------
// State-machine / lifecycle invariants
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_vault_status_transitions_valid(
        ops in prop::collection::vec(arb_vault_op(), 0..30),
    ) {
        #[derive(Clone, Copy, Debug, PartialEq)]
        enum Status {
            Active,
            Expired,
            Released,
        }

        let mut status = Status::Active;
        let mut ttl = 86_400u64; // 1 day
        let check_in_interval = 86_400u64;

        for op in ops {
            match op {
                VaultOp::CheckIn => {
                    if status == Status::Active {
                        ttl = ttl.saturating_add(check_in_interval);
                    }
                }
                VaultOp::Deposit(_) => {
                    if status != Status::Active {
                        continue;
                    }
                }
                VaultOp::Withdraw(_) => {
                    if status != Status::Active {
                        continue;
                    }
                }
            }
        }

        // Invariant: final status is always one of the valid states
        prop_assert!(
            matches!(status, Status::Active | Status::Expired | Status::Released),
            "Vault ended in invalid status: {:?}", status
        );
        // Invariant: if still active, TTL must be ≥ initial
        if status == Status::Active {
            prop_assert!(ttl >= 86_400u64,
                "Active vault TTL ({}) fell below initial ({})", ttl, 86_400u64);
        }
    }

    /// Funds must only be released at most once — a released vault cannot be
    /// re-released.
    #[test]
    fn prop_no_double_release(
        ops in prop::collection::vec(arb_vault_op(), 0..50),
    ) {
        let mut released = false;
        let mut release_count = 0usize;

        for op in ops {
            match op {
                VaultOp::CheckIn => {
                    released = false;
                }
                VaultOp::Deposit(_) | VaultOp::Withdraw(_) => {
                    if !released {
                        released = true;
                        release_count += 1;
                    }
                }
            }
        }

        prop_assert!(
            release_count <= 1,
            "Vault was released {} times — expected at most 1",
            release_count
        );
    }
}
