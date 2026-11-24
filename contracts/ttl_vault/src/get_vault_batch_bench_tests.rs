//! Benchmark tests for `get_vault_batch` gas cost — Issue #2091
//!
//! Measures the CPU instruction cost of `get_vault_batch` at batch sizes of
//! 10, 50, and 100 vaults, using Soroban's `budget()` API.
//!
//! # Assertions
//! * Every batch completes under the Soroban on-chain CPU instruction limit
//!   (100 M instructions per transaction).
//! * Cost scales sub-linearly: the per-vault instruction cost must decrease as
//!   the batch grows (i.e. the marginal cost of an extra vault must fall).
//!
//! # How to run
//! ```
//! cargo test bench_get_vault_batch -- --nocapture
//! ```

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env, Vec as SorobanVec,
};

// ── Soroban on-chain limits ───────────────────────────────────────────────────

/// Soroban CPU instruction limit per transaction.
const SOROBAN_CPU_LIMIT: u64 = 100_000_000;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Create a minimal test environment with a registered vault contract.
fn setup_bench() -> (Env, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    // Mint enough for all deposits.
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000i128);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, client)
}

/// Create `n` vaults, deposit 1_000 tokens into each, and return their IDs.
fn create_vaults(env: &Env, owner: &Address, client: &TtlVaultContractClient, n: u32) -> SorobanVec<u64> {
    let mut ids = SorobanVec::new(env);
    for _ in 0..n {
        let beneficiary = Address::generate(env);
        // Use the minimum check-in interval so create_vault always succeeds.
        let vault_id = client.create_vault(owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
        client.deposit(&vault_id, owner, &1_000i128);
        ids.push_back(vault_id);
    }
    ids
}

/// Measure the CPU instruction and memory cost of a single `get_vault_batch`
/// call for a pre-populated list of `n` vault IDs.
///
/// Returns `(cpu_instructions, memory_bytes, vault_ids)`.
fn measure_get_vault_batch(n: u32) -> (u64, u64) {
    let (env, owner, client) = setup_bench();
    let vault_ids = create_vaults(&env, &owner, &client, n);

    // Reset budget counters immediately before the call under measurement.
    env.budget().reset_default();
    let result = client.get_vault_batch(&vault_ids);

    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();

    // Sanity-check: all n vaults must be returned (no unknowns).
    let returned = result.expect("get_vault_batch must succeed");
    assert_eq!(
        returned.len(),
        n,
        "get_vault_batch(n={n}) must return exactly {n} entries"
    );
    for (i, entry) in returned.iter().enumerate() {
        assert!(
            entry.is_some(),
            "vault at index {i} must be Some (all vaults were created)"
        );
    }

    (cpu, mem)
}

// ── Benchmark tests ───────────────────────────────────────────────────────────

#[test]
fn bench_get_vault_batch_10() {
    let (cpu, mem) = measure_get_vault_batch(10);
    println!("get_vault_batch(n= 10) → cpu={cpu:>12} mem={mem:>10}");
    assert!(
        cpu < SOROBAN_CPU_LIMIT,
        "n=10: cpu {cpu} exceeds on-chain limit {SOROBAN_CPU_LIMIT}"
    );
}

#[test]
fn bench_get_vault_batch_50() {
    let (cpu, mem) = measure_get_vault_batch(50);
    println!("get_vault_batch(n= 50) → cpu={cpu:>12} mem={mem:>10}");
    assert!(
        cpu < SOROBAN_CPU_LIMIT,
        "n=50: cpu {cpu} exceeds on-chain limit {SOROBAN_CPU_LIMIT}"
    );
}

#[test]
fn bench_get_vault_batch_100() {
    // Note: MAX_VAULT_BATCH_SIZE = 50 at the time of writing.  If the constant
    // is raised in the future, this test validates the new limit.
    // If 100 > MAX_VAULT_BATCH_SIZE, get_vault_batch returns BatchTooLarge;
    // the test adapts accordingly so it always documents the actual behaviour.
    let (env, owner, client) = setup_bench();

    if 100 > MAX_VAULT_BATCH_SIZE {
        // Demonstrate the guard rejects oversized batches cleanly.
        let vault_ids = create_vaults(&env, &owner, &client, MAX_VAULT_BATCH_SIZE + 1);
        env.budget().reset_default();
        let result = client.try_get_vault_batch(&vault_ids);
        let cpu = env.budget().cpu_instruction_cost();
        let mem = env.budget().memory_bytes_cost();
        println!(
            "get_vault_batch(n={})  → BatchTooLarge guard fires; cpu={cpu:>12} mem={mem:>10}",
            MAX_VAULT_BATCH_SIZE + 1
        );
        assert!(
            matches!(result, Err(Ok(ContractError::BatchTooLarge))),
            "exceeding MAX_VAULT_BATCH_SIZE must return BatchTooLarge; got {:?}",
            result
        );
    } else {
        let (cpu, mem) = measure_get_vault_batch(100);
        println!("get_vault_batch(n=100) → cpu={cpu:>12} mem={mem:>10}");
        assert!(
            cpu < SOROBAN_CPU_LIMIT,
            "n=100: cpu {cpu} exceeds on-chain limit {SOROBAN_CPU_LIMIT}"
        );
    }
}

// ── Sub-linearity assertion ───────────────────────────────────────────────────

/// The per-vault CPU cost must fall as the batch grows, confirming sub-linear
/// scaling.  Fixed overheads (deserialization, auth, budget accounting) are
/// amortised across a larger batch, so the per-item cost decreases.
#[test]
fn bench_get_vault_batch_cost_scales_sub_linearly() {
    // Use batch sizes that are safely within MAX_VAULT_BATCH_SIZE.
    let n_small: u32 = 10;
    let n_large: u32 = MAX_VAULT_BATCH_SIZE.min(50);

    if n_large <= n_small {
        // Not enough range to compare — skip sub-linearity check.
        return;
    }

    let (cpu_small, _) = measure_get_vault_batch(n_small);
    let (cpu_large, _) = measure_get_vault_batch(n_large);

    // Per-vault cost at each batch size.
    let cost_per_vault_small = cpu_small / u64::from(n_small);
    let cost_per_vault_large = cpu_large / u64::from(n_large);

    println!(
        "sub-linearity check: cost/vault at n={n_small}: {cost_per_vault_small}, \
         at n={n_large}: {cost_per_vault_large}"
    );

    assert!(
        cost_per_vault_large < cost_per_vault_small,
        "get_vault_batch should scale sub-linearly: \
         cost/vault at n={n_large} ({cost_per_vault_large}) must be less than \
         cost/vault at n={n_small} ({cost_per_vault_small})"
    );
}

// ── BatchTooLarge guard ───────────────────────────────────────────────────────

/// Requesting more than `MAX_VAULT_BATCH_SIZE` vaults must return
/// `ContractError::BatchTooLarge` and must do so cheaply (the guard fires
/// before iterating over any storage).
#[test]
fn bench_get_vault_batch_too_large_rejected() {
    let (env, owner, client) = setup_bench();
    // Create only 1 vault — we just need any IDs to build the oversized list.
    let one_id = {
        let beneficiary = Address::generate(&env);
        client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None)
    };

    // Build a list longer than MAX_VAULT_BATCH_SIZE by repeating the single ID.
    let mut ids = SorobanVec::new(&env);
    for _ in 0..=(MAX_VAULT_BATCH_SIZE) {
        ids.push_back(one_id);
    }

    env.budget().reset_default();
    let result = client.try_get_vault_batch(&ids);
    let guard_cpu = env.budget().cpu_instruction_cost();

    println!(
        "get_vault_batch(n={}) BatchTooLarge guard → cpu={guard_cpu}",
        MAX_VAULT_BATCH_SIZE + 1
    );

    assert!(
        matches!(result, Err(Ok(ContractError::BatchTooLarge))),
        "n > MAX_VAULT_BATCH_SIZE must yield BatchTooLarge"
    );

    // The guard should fire with well under 1 M instructions (no vault loads).
    assert!(
        guard_cpu < 1_000_000,
        "BatchTooLarge guard cost {guard_cpu} is suspiciously high \
         (expected < 1 M instructions)"
    );
}

// ── Empty batch ───────────────────────────────────────────────────────────────

/// An empty vault ID list must succeed immediately and return an empty result.
#[test]
fn bench_get_vault_batch_empty() {
    let (env, _owner, client) = setup_bench();
    let empty: SorobanVec<u64> = SorobanVec::new(&env);

    env.budget().reset_default();
    let result = client.get_vault_batch(&empty);
    let cpu = env.budget().cpu_instruction_cost();

    println!("get_vault_batch(n= 0) → cpu={cpu:>12}");

    let returned = result.expect("empty batch must succeed");
    assert_eq!(returned.len(), 0, "empty batch must return empty result");
    assert!(
        cpu < 1_000_000,
        "empty batch should be nearly free; got {cpu} instructions"
    );
}

// ── Unknown IDs ───────────────────────────────────────────────────────────────

/// A batch containing vault IDs that do not exist must return `None` for each
/// unknown entry without panicking.
#[test]
fn bench_get_vault_batch_unknown_ids_return_none() {
    let (env, _owner, client) = setup_bench();

    // Use IDs that were never registered.
    let mut ids = SorobanVec::new(&env);
    for id in [999_001u64, 999_002, 999_003] {
        ids.push_back(id);
    }

    let result = client.get_vault_batch(&ids).expect("must not error on unknowns");
    assert_eq!(result.len(), 3, "result length must match input length");
    for (i, entry) in result.iter().enumerate() {
        assert!(
            entry.is_none(),
            "unknown vault at index {i} must be None"
        );
    }
}
