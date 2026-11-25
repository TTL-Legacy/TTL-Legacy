# Fuzz Testing for TTL Vault Contract

This directory contains fuzz testing infrastructure for the TTL Vault Soroban smart contract. Fuzz testing helps discover panics, unexpected errors, and boundary condition violations by testing entry points with random or semi-random inputs.

## Overview

Fuzz testing uses [libfuzzer](https://llvm.org/docs/LibFuzzer/) via the [libfuzzer-sys](https://docs.rs/libfuzzer-sys/latest/libfuzzer_sys/) Rust crate. Libfuzzer is a coverage-guided fuzzer that:

1. Starts with seed corpus inputs
2. Generates mutations of those inputs
3. Tracks which code paths have been exercised
4. Prioritizes mutations that trigger new coverage
5. Automatically detects and reports crashes, panics, and memory errors

## Fuzz Targets

The following contract entry points have fuzz targets:

### 1. `fuzz_create_vault`
**File**: `src/fuzz_create_vault.rs`

Tests the `create_vault` entry point with arbitrary inputs:
- Owner and beneficiary addresses
- Check-in intervals
- Optional token addresses

**Invariants tested**:
- No panics on any input combination
- Zero interval returns error (not panic)
- Owner == beneficiary returns error (not panic)
- Duplicate vault detection works
- Vault capacity limits are enforced

### 2. `fuzz_deposit`
**File**: `src/fuzz_deposit.rs`

Tests the `deposit` entry point with arbitrary amounts and vault states:
- Vault IDs
- Depositor addresses
- Deposit amounts (including negative, zero, overflow cases)
- Current balances and deposit limits

**Invariants tested**:
- Negative/zero amounts return error (not panic)
- Balance overflow is handled gracefully
- Deposit limits are enforced
- Expired vault detection works
- No integer overflow in balance calculations

### 3. `fuzz_withdraw`
**File**: `src/fuzz_withdraw.rs`

Tests the `withdraw` entry point with arbitrary amounts and vault states:
- Vault IDs
- Caller addresses
- Withdrawal amounts (including negative, zero, exceeding balance)
- Vault balances and approval thresholds
- Minimum balance guards

**Invariants tested**:
- Negative/zero amounts return error (not panic)
- Insufficient balance is detected
- Balance underflow protection works
- Withdrawal approval thresholds are enforced
- Minimum balance guards are respected
- No integer underflow in balance calculations

### 4. `fuzz_check_in`
**File**: `src/fuzz_check_in.rs`

Tests the `check_in` entry point with arbitrary vault states and timestamps:
- Vault IDs
- Caller addresses
- Passkey hashes and nonces
- Timestamps (current, last check-in)
- Check-in intervals
- Inactivity penalties

**Invariants tested**:
- Zero interval returns error (not panic)
- Expired vault detection (current_time >= last_check_in + interval)
- TTL cap enforcement (deadline > max_ttl)
- Passkey validation works
- Nonce replay protection works
- Inactivity penalty calculation doesn't overflow
- Check-in history and streak tracking
- No saturating arithmetic overflow in elapsed time calculations

### 6. `fuzz_xdr_scval`
**File**: `src/fuzz_xdr_scval.rs`

Tests the XDR deserialization path that contract function inputs go through.
Soroban contract arguments are XDR-encoded `ScVal`s; this target feeds
arbitrary bytes through `ScVal::from_xdr` / `Vec<ScVal>::from_xdr` (the exact
deserialization the host performs before invoking a contract function):
- Single `ScVal` and full `ScVec` argument lists
- Lossless encode → decode round trips
- Malformed, truncated, and oversized XDR

**Invariants tested**:
- Malformed XDR is rejected with an error, never a panic
- Any successfully decoded value survives a decode → encode → decode round trip
- The multi-sig numeric payload decoders (`decode_i128` / `decode_u64`
  semantics) never panic on arbitrary payload bytes

### 7. `fuzz_contract_xdr`
**File**: `src/fuzz_contract_xdr.rs`

Full-stack fuzz target: registers the real `ttl_vault` contract in a Soroban
test environment and drives its input-parsing paths:
- **mode 0** - a single `ScVal` decoded from raw XDR, dispatched to
  one-argument entry points (`pause`, `propose_upgrade`, `validate_upgrade`, ...)
- **mode 1** - a `Vec<ScVal>` decoded from raw XDR (the full argument list of a
  call), dispatched to multi-argument entry points (`file_dispute`, `deposit`,
  `check_in`, `propose_multisig`, ...)
- **mode 2** - a structured multi-sig payload (operation + arbitrary `Bytes`
  payload) pushed through `propose_multisig` → `approve_multisig` →
  `execute_multisig`, exercising the contract's internal payload parsers
  (`decode_i128` / `decode_u64`)

**Invariants tested**:
- Malformed XDR never panics — deserialization fails with an error
- Semantically invalid `ScVal`s are rejected during conversion, not by panicking
- Arbitrary payloads (short, empty, oversized) produce contract errors, never
  panics

## Building Fuzz Targets

### Prerequisites

Ensure you have the nightly Rust toolchain:
```bash
rustup install nightly
rustup component add rust-src --toolchain nightly
```

### Build All Targets

```bash
cd contracts/ttl_vault/fuzz
cargo +nightly fuzz build
```

### Build Specific Target

```bash
cargo +nightly fuzz build fuzz_create_vault
```

> Note: `fuzz_contract_xdr` links the contract and soroban-sdk testutils, so
> its first build takes longer than the pure-parsing targets.

## Running Fuzz Tests

### Local Development

Run a single fuzz target for a limited time:

```bash
# Run for 30 seconds
cargo +nightly fuzz run fuzz_create_vault -- -max_total_time=30

# Run for 10 minutes with corpus
cargo +nightly fuzz run fuzz_deposit corpus/fuzz_deposit -- -max_total_time=600

# Run with specific options
cargo +nightly fuzz run fuzz_withdraw \
  corpus/fuzz_withdraw \
  -- \
  -max_len=1024 \
  -timeout=10 \
  -rss_limit_mb=2048 \
  -max_total_time=600
```

### Common Fuzzer Options

| Option | Purpose | Example |
|--------|---------|---------|
| `-max_len=N` | Maximum input length | `-max_len=1024` |
| `-timeout=N` | Timeout per input (seconds) | `-timeout=10` |
| `-max_total_time=N` | Total fuzzing time (seconds) | `-max_total_time=600` |
| `-rss_limit_mb=N` | Memory limit | `-rss_limit_mb=2048` |
| `-dict=FILE` | Use a dictionary | `-dict=tokens.dict` |

### Interpreting Results

#### Successful Run
```
#123456  NEW    cov: 1234 ft: 567  corp: 89/45K  exec/s: 1200  ...
#234567  COVERED cov: 1245 ft: 572  corp: 92/47K  exec/s: 1350  ...
```

- `NEW`: Input triggered new coverage
- `cov`: Total coverage (basic block count)
- `ft`: Feature count
- `corp`: Corpus size / total bytes
- `exec/s`: Executions per second

#### Test Failure

If a panic or crash is found:
```
artifact_prefix='corpus/'; Test unit written to corpus/crash-0acb5eafc...
```

The fuzzer saves the failing input for regression testing.

## CI Integration

Fuzz testing is integrated into the CI pipeline via `nightly-fuzz.yml`:

1. **Nightly Job**: Runs for 10+ minutes per target
2. **Failure Detection**: Stops on first panic/crash
3. **Corpus Maintenance**: Committed failures to prevent regressions
4. **Coverage Report**: Tracks overall code coverage

See [.github/workflows/nightly-fuzz.yml](.github/workflows/nightly-fuzz.yml).

## Handling Crashes

When the fuzzer finds a crash:

1. **Locate the Input**: Check the corpus file path (usually in `corpus/crash-xxx`)
2. **Reproduce Locally**: Copy the crash file and run the target with it
3. **Minimize**: Use `cargo fuzz cmin` to reduce the input
4. **Debug**: Add assertions or logging to understand the issue
5. **Fix**: Address the underlying bug in the contract
6. **Verify**: Re-run fuzzer to confirm fix

Example:
```bash
# Reproduce crash locally
cp corpus/crash-0acb5eafc fuzz_create_vault_crash
cargo +nightly fuzz run fuzz_create_vault fuzz_create_vault_crash

# Minimize the input
cargo +nightly fuzz cmin fuzz_create_vault fuzz_create_vault_crash

# After fixing, re-run to verify
cargo +nightly fuzz run fuzz_create_vault -- -max_total_time=600
```

## Corpus Management

### Generating Initial Corpus

Use the provided Python script to generate seed inputs:

```bash
python3 generate_corpus.py
```

This creates seed files in `corpus/fuzz_*` directories representing:
- Valid inputs (should succeed)
- Boundary conditions (edge cases)
- Invalid inputs (should error gracefully)

### Expanding Corpus

The fuzzer automatically expands the corpus by saving:
- Inputs that trigger new coverage
- Inputs that trigger bugs or panics
- Minimized versions of complex inputs

After extended fuzzing (hours or days), commit the expanded corpus to prevent regressions:

```bash
git add corpus/
git commit -m "Update fuzz corpus after 24-hour fuzzing run"
```

### Corpus Minimization

If the corpus grows too large, minimize it:

```bash
cargo +nightly fuzz cmin fuzz_create_vault corpus/fuzz_create_vault
```

This keeps only the inputs necessary for full coverage.

## Performance Optimization

### Improving Fuzzer Speed

1. **Reduce Input Size**: `-max_len=512` (instead of default 4KB)
2. **Add Timeout**: `-timeout=5` catches infinite loops faster
3. **Memory Limit**: `-rss_limit_mb=1024` kills runaway memory use
4. **Use Dictionary**: Provide known tokens/values via `-dict=file`

### Parallelization

Run multiple fuzzer instances in parallel:

```bash
# Terminal 1
cargo +nightly fuzz run fuzz_create_vault corpus/fuzz_create_vault

# Terminal 2
cargo +nightly fuzz run fuzz_create_vault corpus/fuzz_create_vault

# Terminal 3
cargo +nightly fuzz run fuzz_create_vault corpus/fuzz_create_vault
```

Shared corpus automatically merges findings.

## Troubleshooting

### Build Fails with "libfuzzer not found"
Ensure nightly and rust-src are installed:
```bash
rustup update nightly
rustup component add rust-src --toolchain nightly
```

### Fuzzer Crashes Immediately
Check that corpus files are valid binary data. Re-generate if needed:
```bash
rm -rf corpus/
python3 generate_corpus.py
```

### Memory Usage Explodes
Add memory limits:
```bash
cargo +nightly fuzz run fuzz_create_vault -- -rss_limit_mb=512
```

### No Coverage Progress
Check if the fuzz target is actually being called. Verify binary was built with fuzzer instrumentation.

## Best Practices

1. **Regular Runs**: Run fuzzer for hours/days, not just minutes
2. **Corpus Commits**: Save discovered bugs and edge cases in corpus
3. **CI Integration**: Run nightly on CI to catch regressions
4. **Minimization**: Use `cmin` to understand crash inputs better
5. **Documentation**: Document any discovered edge cases

## References

- [libfuzzer Documentation](https://llvm.org/docs/LibFuzzer/)
- [libfuzzer-sys Crate](https://docs.rs/libfuzzer-sys/)
- [Cargo Fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html)
- [TTL Vault Contract](../src/lib.rs)
