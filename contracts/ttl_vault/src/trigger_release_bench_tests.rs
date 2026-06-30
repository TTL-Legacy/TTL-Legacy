/// Benchmarks for trigger_release instruction usage vs. beneficiary count.
///
/// These tests use Soroban's `budget()` API to measure CPU instructions and
/// memory bytes consumed by `trigger_release` as the beneficiary list grows.
/// The results confirm the safe maximum (`MAX_BENEFICIARIES = 20`) stays well
/// below Soroban's on-chain 100M instruction limit.
///
/// Run with:
///   cargo test bench_trigger_release -- --nocapture
extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn setup_bench() -> (Env, Address, Address, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let admin = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract_v2(token_admin).address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &100_000_000i128);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, beneficiary, admin, client)
}

/// Build `n` unique beneficiary entries with equal BPS splits.
///
/// BPS must sum to exactly 10_000; this distributes evenly and assigns any
/// remainder to the last entry.
fn make_beneficiaries(env: &Env, n: u32) -> soroban_sdk::Vec<BeneficiaryEntry> {
    assert!(n > 0 && n <= 10_000, "n must be 1..=10_000");
    let base = 10_000u32 / n;
    let remainder = 10_000u32 - base * n;
    let mut entries = soroban_sdk::Vec::new(env);
    for i in 0..n {
        let bps = if i == n - 1 { base + remainder } else { base };
        entries.push_back(BeneficiaryEntry {
            address: Address::generate(env),
            bps,
            minimum_threshold: 0,
        });
    }
    entries
}

/// Set up a vault with `n` beneficiaries, advance past TTL, then call
/// `trigger_release` and return the budget snapshot.
fn measure_trigger_release(n: u32) -> (u64, u64) {
    let (env, owner, primary_beneficiary, _admin, client) = setup_bench();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &primary_beneficiary, &interval, &None);
    client.deposit(&vault_id, &owner, &1_000_000i128);

    if n > 1 {
        let entries = make_beneficiaries(&env, n);
        client.set_beneficiaries(&vault_id, &owner, &entries).unwrap();
    }

    // Advance ledger past the check-in interval so the vault expires
    env.ledger().with_mut(|l| l.timestamp = interval + 1);

    // Reset budget counters immediately before the call under measurement
    env.budget().reset_default();
    client.trigger_release(&vault_id);

    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    (cpu, mem)
}

// ── benchmark tests ──────────────────────────────────────────────────────────

/// Soroban on-chain CPU instruction limit per transaction.
const SOROBAN_CPU_LIMIT: u64 = 100_000_000;

#[test]
fn bench_trigger_release_1_beneficiary() {
    let (cpu, mem) = measure_trigger_release(1);
    println!("trigger_release(n=1 ) → cpu={cpu:>12} mem={mem:>10}");
    assert!(
        cpu < SOROBAN_CPU_LIMIT,
        "n=1: cpu {cpu} exceeds on-chain limit {SOROBAN_CPU_LIMIT}"
    );
}

#[test]
fn bench_trigger_release_5_beneficiaries() {
    let (cpu, mem) = measure_trigger_release(5);
    println!("trigger_release(n=5 ) → cpu={cpu:>12} mem={mem:>10}");
    assert!(
        cpu < SOROBAN_CPU_LIMIT,
        "n=5: cpu {cpu} exceeds on-chain limit {SOROBAN_CPU_LIMIT}"
    );
}

#[test]
fn bench_trigger_release_10_beneficiaries() {
    let (cpu, mem) = measure_trigger_release(10);
    println!("trigger_release(n=10) → cpu={cpu:>12} mem={mem:>10}");
    assert!(
        cpu < SOROBAN_CPU_LIMIT,
        "n=10: cpu {cpu} exceeds on-chain limit {SOROBAN_CPU_LIMIT}"
    );
}

#[test]
fn bench_trigger_release_20_beneficiaries() {
    let (cpu, mem) = measure_trigger_release(20);
    println!("trigger_release(n=20) → cpu={cpu:>12} mem={mem:>10}");
    // n=20 is MAX_BENEFICIARIES — must be comfortably under the limit
    assert!(
        cpu < SOROBAN_CPU_LIMIT,
        "n=20 (MAX): cpu {cpu} exceeds on-chain limit {SOROBAN_CPU_LIMIT}"
    );
}

#[test]
fn bench_trigger_release_50_beneficiaries() {
    // n=50 bypasses set_beneficiaries (which would reject it) by constructing
    // the beneficiary list directly to show why the guard is needed.
    let (env, owner, primary_beneficiary, _admin, client) = setup_bench();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &primary_beneficiary, &interval, &None);
    client.deposit(&vault_id, &owner, &1_000_000i128);

    // Directly set vault beneficiaries, bypassing the MAX_BENEFICIARIES guard.
    // This simulates the scenario set_beneficiaries now prevents.
    let entries = make_beneficiaries(&env, 50);
    {
        let mut vault = client.get_vault(&vault_id);
        vault.beneficiaries = entries;
        // Persist via the internal helper by using the public API path:
        // we can't call the internal helper from tests, so we use a raw
        // storage write to simulate legacy / pre-guard data.
        env.as_contract(&client.address, || {
            let key = DataKey::Vault(vault_id);
            env.storage().persistent().set(&key, &vault);
        });
    }

    env.ledger().with_mut(|l| l.timestamp = interval + 1);
    env.budget().reset_default();
    client.trigger_release(&vault_id);

    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("trigger_release(n=50) → cpu={cpu:>12} mem={mem:>10}  ← why guard is needed");
    // This test documents cost without asserting a pass/fail on the limit,
    // since n=50 is explicitly blocked by the runtime guard. Its purpose is
    // to show the instruction curve and justify MAX_BENEFICIARIES = 20.
}

// ── guard tests ──────────────────────────────────────────────────────────────

#[test]
fn test_set_beneficiaries_rejects_above_max() {
    let (env, owner, primary_beneficiary, _admin, client) = setup_bench();
    let vault_id = client.create_vault(&owner, &primary_beneficiary, &1_000u64, &None);

    let entries = make_beneficiaries(&env, MAX_BENEFICIARIES + 1);
    let err = client
        .try_set_beneficiaries(&vault_id, &owner, &entries)
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from_contract_error(ContractError::TooManyBeneficiaries as u32)
    );
}

#[test]
fn test_set_beneficiaries_accepts_max() {
    let (env, owner, primary_beneficiary, _admin, client) = setup_bench();
    let vault_id = client.create_vault(&owner, &primary_beneficiary, &1_000u64, &None);

    let entries = make_beneficiaries(&env, MAX_BENEFICIARIES);
    client
        .set_beneficiaries(&vault_id, &owner, &entries)
        .unwrap();
    assert_eq!(client.get_vault(&vault_id).beneficiaries.len(), MAX_BENEFICIARIES);
}
