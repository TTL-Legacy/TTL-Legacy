# Performance Benchmarking Guide

## Overview

TTL-Legacy includes a comprehensive benchmarking suite using [Criterion.rs](https://bheisler.github.io/criterion.rs/book/) to track contract performance and detect regressions.

## Running Benchmarks

### Run All Benchmarks

```bash
cargo bench --package ttl-vault
```

### Run Specific Benchmark

```bash
cargo bench --package ttl-vault -- vault_creation
```

### Run with Verbose Output

```bash
cargo bench --package ttl-vault -- --verbose
```

### Generate HTML Report

Criterion automatically generates HTML reports in `target/criterion/`. Open `target/criterion/report/index.html` in a browser to view detailed results.

## Benchmark Suites

### 1. Vault Creation (`bench_vault_creation`)

Measures the cost of creating a new vault with a beneficiary and check-in interval.

**Metrics**:
- Time to execute `create_vault()`
- Memory allocation overhead
- Storage write operations

**Target**: < 5ms per vault creation

### 2. Check-In (`bench_check_in`)

Measures the cost of extending a vault's TTL via check-in.

**Metrics**:
- Time to execute `check_in()`
- TTL extension overhead
- Storage update cost

**Target**: < 2ms per check-in

### 3. Deposit (`bench_deposit`)

Measures deposit performance with varying amounts (1K, 100K, 1M XLM).

**Metrics**:
- Time to execute `deposit()` for each amount
- Token transfer overhead
- Balance update cost

**Target**: < 3ms per deposit (independent of amount)

### 4. Withdrawal (`bench_withdrawal`)

Measures withdrawal performance with varying amounts (1K, 100K, 1M XLM).

**Metrics**:
- Time to execute `withdraw()` for each amount
- Token transfer overhead
- Balance validation cost

**Target**: < 3ms per withdrawal (independent of amount)

### 5. Release (`bench_release`)

Measures the cost of triggering fund release after TTL expiry.

**Metrics**:
- Time to execute `trigger_release()`
- Beneficiary payout overhead
- State cleanup cost

**Target**: < 5ms per release

## Performance Regression Detection

### CI Integration

Benchmarks run on every PR to detect regressions:

```yaml
# .github/workflows/ci.yml
- name: Run benchmarks
  run: cargo bench --package ttl-vault -- --output-format bencher | tee output.txt

- name: Store benchmark result
  uses: benchmark-action/github-action@v1
  with:
    tool: 'cargo'
    output-file-path: output.txt
    github-token: ${{ secrets.GITHUB_TOKEN }}
    auto-push: true
```

### Regression Thresholds

- **Warning**: 10% performance degradation
- **Failure**: 20% performance degradation

If a PR introduces a regression, the CI will fail and require investigation.

## Interpreting Results

### Criterion Output

```
vault_creation             time:   [4.523 ms 4.612 ms 4.712 ms]
                           change: [-2.34% +1.23% +5.12%] (within noise)
```

- **time**: Measured execution time (lower bound, estimate, upper bound)
- **change**: Comparison to previous benchmark run

### HTML Report

The HTML report includes:
- Line graphs showing performance over time
- Statistical analysis (mean, std dev, outliers)
- Regression detection
- Comparison to baseline

## Adding New Benchmarks

1. Add a new benchmark function in `benches/benchmarks.rs`:

```rust
fn bench_new_operation(c: &mut Criterion) {
    c.bench_function("new_operation", |b| {
        b.iter(|| {
            // Setup
            let (env, owner, beneficiary, admin, token_address) = setup_env();
            
            // Operation to benchmark
            // client.new_operation(...);
        });
    });
}
```

2. Add to the `criterion_group!` macro:

```rust
criterion_group!(
    benches,
    bench_vault_creation,
    bench_check_in,
    bench_deposit,
    bench_withdrawal,
    bench_release,
    bench_new_operation  // Add here
);
```

3. Run benchmarks to establish baseline:

```bash
cargo bench --package ttl-vault
```

## Performance Optimization Tips

### Identify Bottlenecks

1. Run benchmarks with verbose output
2. Check HTML report for slowest operations
3. Profile with `perf` or `flamegraph` for detailed analysis

### Common Optimizations

- **Reduce storage reads**: Cache frequently accessed data
- **Batch operations**: Combine multiple operations into one
- **Optimize token transfers**: Use efficient token contract calls
- **Minimize allocations**: Pre-allocate vectors and strings

### Before Optimization

Always benchmark before and after to measure impact:

```bash
# Before
cargo bench --package ttl-vault > before.txt

# Make changes

# After
cargo bench --package ttl-vault > after.txt

# Compare
diff before.txt after.txt
```

## Troubleshooting

### Benchmarks Are Noisy

- Increase sample size: `cargo bench --package ttl-vault -- --sample-size 1000`
- Run on a quiet machine (close other applications)
- Use `--warm-up-time 5` to allow CPU to stabilize

### Benchmarks Fail in CI

- Check for resource constraints (CPU, memory)
- Verify RPC endpoint is responsive
- Review recent code changes for performance regressions

### Benchmark Results Vary Widely

- This is normal for smart contracts (network latency, ledger state)
- Use statistical analysis in HTML report to identify true regressions
- Focus on relative changes, not absolute values

## References

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Benchmarking Best Practices](https://doc.rust-lang.org/cargo/commands/cargo-bench.html)
- [Soroban Performance Guide](https://soroban.stellar.org/docs/learn/storing-data)

### 6. `trigger_release` vs. Beneficiary Count (`bench_trigger_release_*`)

Measures how `trigger_release` CPU instruction usage and memory cost scale as the number of beneficiaries grows. This benchmark drives the `MAX_BENEFICIARIES` runtime guard (Issue #872).

**Why it matters**: `trigger_release` iterates over every beneficiary entry to filter, rebalance, and transfer. Each extra beneficiary adds storage reads and token transfers. Without a cap the function could silently exhaust the Soroban instruction budget on-chain (100M instructions).

**Test cases** (run with `cargo test bench_trigger_release -- --nocapture`):

| Test | Beneficiaries | Assertion |
|---|---|---|
| `bench_trigger_release_1_beneficiary` | 1 | cpu < 100M |
| `bench_trigger_release_5_beneficiaries` | 5 | cpu < 100M |
| `bench_trigger_release_10_beneficiaries` | 10 | cpu < 100M |
| `bench_trigger_release_20_beneficiaries` | 20 (MAX) | cpu < 100M |
| `bench_trigger_release_50_beneficiaries` | 50 (over limit, legacy data) | documents cost curve |

**How to run**:

```bash
cargo test --package ttl-vault bench_trigger_release -- --nocapture
```

**Sample output** (native Rust, underestimates WASM cost):

```
trigger_release(n=1 ) → cpu=     123456 mem=      4096
trigger_release(n=5 ) → cpu=     234567 mem=      8192
trigger_release(n=10) → cpu=     345678 mem=     12288
trigger_release(n=20) → cpu=     456789 mem=     16384
trigger_release(n=50) → cpu=     789012 mem=     24576  ← why guard is needed
```

> Note: The Soroban test environment measures native Rust costs which are **significantly lower** than compiled WASM costs on-chain. The on-chain instruction limit is 100,000,000. Use the native measurements as relative guidance and apply a safety margin when choosing `MAX_BENEFICIARIES`.

**Derived safe maximum**: `MAX_BENEFICIARIES = 20` (constant in `lib.rs`). At this count the native instruction count has a wide margin below 100M. The WASM overhead multiplier is typically 5–10×, so 20 beneficiaries remains safe even accounting for WASM compilation.

**Runtime guard**: `set_beneficiaries` rejects lists longer than `MAX_BENEFICIARIES` with `ContractError::TooManyBeneficiaries` (code 83). Legacy vaults with more than 20 beneficiaries stored before this guard was introduced will still trigger correctly (the 50-beneficiary test documents this path).

**Target**: `trigger_release` with `MAX_BENEFICIARIES` (20) must complete within the Soroban instruction budget on-chain.
