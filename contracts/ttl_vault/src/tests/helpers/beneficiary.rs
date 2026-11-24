//! Shared constructors for beneficiary test entries.

use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

use crate::BeneficiaryEntry;

/// Creates a beneficiary entry with no minimum threshold.
pub fn make_beneficiary(_env: &Env, address: Address, bps: u32) -> BeneficiaryEntry {
    make_beneficiary_with_threshold(_env, address, bps, 0)
}

/// Creates a beneficiary entry with an explicit minimum threshold.
pub fn make_beneficiary_with_threshold(
    _env: &Env,
    address: Address,
    bps: u32,
    minimum_threshold: i128,
) -> BeneficiaryEntry {
    BeneficiaryEntry {
        address,
        bps,
        minimum_threshold,
    }
}

/// Creates a generated beneficiary entry with no minimum threshold.
pub fn make_generated_beneficiary(env: &Env, bps: u32) -> BeneficiaryEntry {
    make_beneficiary(env, Address::generate(env), bps)
}

/// Builds `n` unique beneficiary entries with equal BPS splits.
///
/// The split totals exactly 10,000 BPS; any remainder is assigned to the final
/// entry.
pub fn make_beneficiaries(env: &Env, n: u32) -> Vec<BeneficiaryEntry> {
    assert!(n > 0 && n <= 10_000, "n must be 1..=10_000");

    let base = 10_000u32 / n;
    let remainder = 10_000u32 - base * n;
    let mut entries = Vec::new(env);

    for i in 0..n {
        let bps = if i == n - 1 {
            base + remainder
        } else {
            base
        };
        entries.push_back(make_generated_beneficiary(env, bps));
    }

    entries
}
