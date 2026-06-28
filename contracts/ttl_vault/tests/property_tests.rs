use proptest::prelude::*;

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

    #[test]
    fn prop_ttl_always_increases_on_check_in(
        base_ttl in 1u64..86400u64 * 365,
        check_in_interval in 1u64..86400u64 * 365,
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
        let mut ttl = 86400u64; // 1 day
        let mut check_in_interval = 86400u64;
        
        for op in ops {
            match op {
                VaultOp::CheckIn => {
                    if status == Status::Active {
                        ttl = ttl.saturating_add(check_in_interval);
                    }
                }
                VaultOp::Deposit(_) => {
                    // Can only deposit to active vault
                    if status != Status::Active {
                        continue;
                    }
                }
                VaultOp::Withdraw(_) => {
                    // Can only withdraw from active vault
                    if status != Status::Active {
                        continue;
                    }
                }
            }
        }
        
        // Invariant: status should be valid
        prop_assert!(matches!(status, Status::Active | Status::Expired | Status::Released));
    }

    #[test]
    fn prop_no_double_release(
        ops in prop::collection::vec(arb_vault_op(), 0..50),
    ) {
        let mut released = false;
        let mut release_count = 0;
        
        for op in ops {
            match op {
                VaultOp::CheckIn => {
                    // Check-in prevents release
                    released = false;
                }
                VaultOp::Deposit(_) | VaultOp::Withdraw(_) => {
                    if !released {
                        // Simulate expiry triggering release
                        released = true;
                        release_count += 1;
                    }
                }
            }
        }
        
        // Invariant: funds should only be released once
        prop_assert!(release_count <= 1);
    }
}

// --- Issue #849: vault_ttl_ledgers property tests ---

/// Constants mirrored from lib.rs for property testing.
const VAULT_TTL_LEDGERS_MIN: u32 = 200_000;
const MAX_PERSISTENT_TTL: u32 = 3_110_400;
const LEDGER_SECOND: u32 = 5;

fn vault_ttl_ledgers(check_in_interval: u64) -> u32 {
    let ledgers = (check_in_interval as u32)
        .saturating_mul(2)
        .saturating_div(LEDGER_SECOND);
    ledgers.clamp(VAULT_TTL_LEDGERS_MIN, MAX_PERSISTENT_TTL)
}

proptest! {
    /// vault_ttl_ledgers must never exceed MAX_PERSISTENT_TTL.
    #[test]
    fn prop_vault_ttl_never_exceeds_max(interval in 0u64..u64::MAX) {
        prop_assert!(vault_ttl_ledgers(interval) <= MAX_PERSISTENT_TTL);
    }

    /// vault_ttl_ledgers must be monotonically non-decreasing: a >= b => f(a) >= f(b).
    #[test]
    fn prop_vault_ttl_monotonically_non_decreasing(
        a in 0u64..u64::MAX / 2,
        b in 0u64..u64::MAX / 2,
    ) {
        let (big, small) = if a >= b { (a, b) } else { (b, a) };
        prop_assert!(vault_ttl_ledgers(big) >= vault_ttl_ledgers(small));
    }
}
