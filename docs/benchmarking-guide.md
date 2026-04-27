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
