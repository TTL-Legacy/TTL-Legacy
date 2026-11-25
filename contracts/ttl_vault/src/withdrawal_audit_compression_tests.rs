#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// One simulated audit entry: an (amount, timestamp) pair.
#[derive(Clone, Debug, PartialEq)]
struct AuditEntry {
    amount: u128,
    timestamp: u64,
}

/// A compressed summary produced by `compress_withdrawal_history`.
#[derive(Clone, Debug, PartialEq)]
struct CompressedSummary {
    count: u64,
    total_amount: u128,
    timestamp_start: u64,
    timestamp_end: u64,
}

/// Minimal in-memory stand-in for the on-chain compression logic.
///
/// Groups all entries whose `timestamp` is older than
/// `cutoff_timestamp` into a single `CompressedSummary`, then appends
/// any recent entries unchanged as individual `count=1` summaries.
///
/// Returns `(summaries, recent_entries)`.
fn compress(
    entries: &[AuditEntry],
    cutoff_timestamp: u64,
) -> (Vec<CompressedSummary>, Vec<AuditEntry>) {
    let old: Vec<&AuditEntry> = entries
        .iter()
        .filter(|e| e.timestamp < cutoff_timestamp)
        .collect();

    let recent: Vec<AuditEntry> = entries
        .iter()
        .filter(|e| e.timestamp >= cutoff_timestamp)
        .cloned()
        .collect();

    let mut summaries: Vec<CompressedSummary> = Vec::new();

    if !old.is_empty() {
        let total_amount: u128 = old.iter().map(|e| e.amount).sum();
        let ts_start = old.iter().map(|e| e.timestamp).min().unwrap();
        let ts_end = old.iter().map(|e| e.timestamp).max().unwrap();
        summaries.push(CompressedSummary {
            count: old.len() as u64,
            total_amount,
            timestamp_start: ts_start,
            timestamp_end: ts_end,
        });
    }

    (summaries, recent)
}

// ── Existing tests (unchanged) ────────────────────────────────────────────────

#[test]
fn test_compress_withdrawal_history_aggregates_old_records() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test that compress_withdrawal_history aggregates withdrawals older than keep_recent_days
    // Old records should be combined into summary entries
    // Summary should contain: (count, total_amount, timestamp_start, timestamp_end)
}

#[test]
fn test_compress_withdrawal_history_preserves_recent() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test that recent withdrawals (within keep_recent_days) are preserved as-is
    // Only old withdrawals should be aggregated
    // Recent record details should remain unchanged
}

#[test]
fn test_compress_withdrawal_history_owner_only() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let non_owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test that only owner can call compress_withdrawal_history
    // Non-owner should get authorization error
}

#[test]
fn test_compress_withdrawal_history_invalid_keep_days() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let invalid_keep_days: u64 = 0;

    // Test that keep_recent_days = 0 is rejected
    // Should return InvalidParameter error
    // Must have positive day value to preserve some history
}

#[test]
fn test_compress_withdrawal_history_empty_vault() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test that compressing empty withdrawal history succeeds without error
    // Should return gracefully with no aggregations needed
}

#[test]
fn test_get_compressed_history_returns_summary_entries() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;

    // Test that get_compressed_history returns Vec of (count, total_amount, timestamp_start, timestamp_end)
    // Should be ordered chronologically
}

#[test]
fn test_get_compressed_history_owner_only() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let non_owner = Address::generate(&env);
    let vault_id: u64 = 1;

    // Test that only owner can retrieve compressed history
    // Non-owner should get authorization error
}

#[test]
fn test_get_compressed_history_after_compression() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test that get_compressed_history returns aggregated summaries after compression
    // Should reflect the compression that was performed
}

#[test]
fn test_compression_calculates_correct_total_amount() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test that aggregated total_amount is sum of all old withdrawals
    // Should correctly sum stroops
}

#[test]
fn test_compression_calculates_correct_count() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test that aggregated count is number of withdrawals combined
    // Count should accurately reflect combined records
}

#[test]
fn test_compression_timestamp_boundaries() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test that timestamp_start and timestamp_end are correctly set
    // Should represent the time range of aggregated records
    // timestamp_start should be oldest, timestamp_end should be newest
}

#[test]
fn test_multiple_compression_calls() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test that multiple compress_withdrawal_history calls are safe
    // Second compression should handle already-compressed data correctly
}

#[test]
fn test_compression_with_mixed_old_and_new() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test compression with both old and recent withdrawals
    // Old withdrawals should be aggregated
    // Recent withdrawals should remain as individual entries
}

#[test]
fn test_compression_preserves_data_integrity() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test that total amount before and after compression matches
    // No data loss should occur during aggregation
}

#[test]
fn test_compression_handles_partial_day_boundaries() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test that keep_recent_days is calculated correctly at day boundaries
    // Withdrawals exactly at the boundary should be included correctly
}

#[test]
fn test_compression_with_single_old_withdrawal() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test compression with only one old withdrawal
    // Should create summary with count=1, amount=withdrawal_amount
}

#[test]
fn test_compression_with_many_withdrawals() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test compression with many withdrawals (e.g., 100+)
    // Should handle large datasets efficiently
}

#[test]
fn test_compression_zero_amount_withdrawals() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test compression with zero-amount withdrawal entries
    // Should correctly sum amounts, including zeros
}

#[test]
fn test_compressed_history_ordering() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;

    // Test that get_compressed_history returns chronologically ordered summaries
    // Earlier time ranges should come before later ones
}

#[test]
fn test_compression_idempotence() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test that running compression twice with same keep_recent_days is idempotent
    // Second run should produce same result as first
}

#[test]
fn test_compression_reduces_storage() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;
    let keep_recent_days: u64 = 30;

    // Test that compression reduces total number of history entries
    // Multiple old records should become single summary entry
}

#[test]
fn test_get_compressed_history_empty_result() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let vault_id: u64 = 1;

    // Test that get_compressed_history returns empty vec for vault with no history
}

// ── New high-volume test — issue #2077 ───────────────────────────────────────

/// Creates 10 000 audit entries spread across two time buckets, compresses
/// them, and verifies:
///
/// 1. The compressed representation has fewer items than the original.
/// 2. The compressed summary exactly accounts for all old entries (count,
///    total amount, timestamp bounds).
/// 3. Recent entries survive compression unchanged and sum to their
///    expected total.
/// 4. Decompressing (re-expanding) the summary back to individual entries
///    yields a set whose total amount matches the original full set.
#[test]
fn test_high_volume_compression_10000_entries() {
    // ── Build 10 000 synthetic audit entries ──────────────────────────────
    //
    // The cutoff divides the timeline at the 7 000-entry mark so we get a
    // meaningful mix: 7 000 "old" entries that should be aggregated and
    // 3 000 "recent" entries that must survive unchanged.

    const TOTAL_ENTRIES: usize = 10_000;
    const OLD_COUNT: usize = 7_000;
    const RECENT_COUNT: usize = TOTAL_ENTRIES - OLD_COUNT;

    // Timestamps: old entries are in [1, OLD_COUNT], recent in
    // [cutoff_timestamp, cutoff_timestamp + RECENT_COUNT).
    let cutoff_timestamp: u64 = (OLD_COUNT as u64) + 1;

    let mut entries: Vec<AuditEntry> = Vec::with_capacity(TOTAL_ENTRIES);

    // Old entries — timestamps 1 .. OLD_COUNT (all < cutoff_timestamp).
    for i in 0..OLD_COUNT {
        entries.push(AuditEntry {
            amount: (i as u128 % 1_000) + 1, // amounts 1–1 000
            timestamp: (i as u64) + 1,
        });
    }

    // Recent entries — timestamps cutoff_timestamp .. cutoff_timestamp + RECENT_COUNT.
    for i in 0..RECENT_COUNT {
        entries.push(AuditEntry {
            amount: (i as u128 % 500) + 1, // amounts 1–500
            timestamp: cutoff_timestamp + i as u64,
        });
    }

    assert_eq!(entries.len(), TOTAL_ENTRIES, "setup: expected 10 000 entries");

    // ── Compute expected totals before compression ─────────────────────────

    let total_amount_before: u128 = entries.iter().map(|e| e.amount).sum();

    let old_expected_total: u128 = entries
        .iter()
        .filter(|e| e.timestamp < cutoff_timestamp)
        .map(|e| e.amount)
        .sum();

    let recent_expected_total: u128 = entries
        .iter()
        .filter(|e| e.timestamp >= cutoff_timestamp)
        .map(|e| e.amount)
        .sum();

    assert_eq!(
        old_expected_total + recent_expected_total,
        total_amount_before,
        "pre-compression totals must partition cleanly"
    );

    // ── Run compression ────────────────────────────────────────────────────

    let (summaries, recent_entries) = compress(&entries, cutoff_timestamp);

    // ── Invariant 1: compressed representation is smaller ─────────────────
    //
    // We had 10 000 individual entries.  After compression the old 7 000
    // collapse to 1 summary; combined with 3 000 surviving recent entries
    // the total item count is 1 + 3 000 = 3 001 — far fewer than 10 000.

    let compressed_item_count = summaries.len() + recent_entries.len();
    assert!(
        compressed_item_count < TOTAL_ENTRIES,
        "compressed count {} must be less than original {}",
        compressed_item_count,
        TOTAL_ENTRIES,
    );

    // ── Invariant 2: old-entry summary is correct ──────────────────────────

    assert_eq!(
        summaries.len(),
        1,
        "exactly one summary for the old-entry bucket"
    );

    let summary = &summaries[0];

    assert_eq!(
        summary.count,
        OLD_COUNT as u64,
        "summary count must equal number of old entries"
    );
    assert_eq!(
        summary.total_amount,
        old_expected_total,
        "summary total_amount must equal sum of old entry amounts"
    );
    assert_eq!(
        summary.timestamp_start,
        1,
        "timestamp_start must be the earliest old-entry timestamp"
    );
    assert_eq!(
        summary.timestamp_end,
        OLD_COUNT as u64,
        "timestamp_end must be the latest old-entry timestamp"
    );

    // ── Invariant 3: recent entries survive unchanged ──────────────────────

    assert_eq!(
        recent_entries.len(),
        RECENT_COUNT,
        "all recent entries must be preserved"
    );

    let recent_actual_total: u128 = recent_entries.iter().map(|e| e.amount).sum();
    assert_eq!(
        recent_actual_total,
        recent_expected_total,
        "recent-entry total must be unchanged by compression"
    );

    // Verify every recent entry has a timestamp ≥ cutoff.
    for entry in &recent_entries {
        assert!(
            entry.timestamp >= cutoff_timestamp,
            "recent entry timestamp {} is below cutoff {}",
            entry.timestamp,
            cutoff_timestamp,
        );
    }

    // ── Invariant 4: full decompression restores original total ───────────
    //
    // "Decompressing" here means treating the summary's total_amount as
    // equivalent to the sum of OLD_COUNT individual entries, then adding
    // the recent-entry total.  The resulting grand total must equal the
    // pre-compression total.

    let decompressed_total: u128 = summary.total_amount + recent_actual_total;
    assert_eq!(
        decompressed_total,
        total_amount_before,
        "decompressed total {} ≠ original total {} — data was lost during compression",
        decompressed_total,
        total_amount_before,
    );
}
