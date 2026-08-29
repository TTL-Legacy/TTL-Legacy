# Integration Testing Guide

## Overview

TTL-Legacy includes integration tests that verify contract behavior on Stellar testnet. These tests validate real-world scenarios including network latency, token transfers, and multi-step workflows.

## Prerequisites

- Rust 1.70+
- Soroban CLI
- Stellar CLI
- Funded testnet account (for transaction fees)
- Deployed contract on testnet

## Setup

### 1. Deploy Contract to Testnet

```bash
export STELLAR_NETWORK=testnet
./scripts/deploy_testnet.sh
```

Save the contract ID from the output.

### 2. Configure Environment

Create `.env.testnet`:

```env
STELLAR_NETWORK=testnet
STELLAR_RPC_URL=https://soroban-testnet.stellar.org
CONTRACT_ID=<your-contract-id>
TESTNET_ACCOUNT=<your-testnet-account>
```

### 3. Fund Test Account

```bash
# Generate test account
stellar keys generate test-account --network testnet

# Get public key
stellar keys show test-account

# Fund via testnet faucet (visit https://stellar.org/developers/testnet)
```

## Running Integration Tests

### Run All Integration Tests

```bash
cargo test --package ttl-vault --test integration_tests -- --ignored --test-threads=1
```

The `--ignored` flag runs only integration tests (marked with `#[ignore]`).
The `--test-threads=1` flag ensures tests run sequentially to avoid state conflicts.

### Run Specific Integration Test

```bash
cargo test --package ttl-vault --test integration_tests integration_full_vault_lifecycle -- --ignored
```

### Run with Verbose Output

```bash
cargo test --package ttl-vault --test integration_tests -- --ignored --nocapture
```

## Test Suites

### 1. Full Vault Lifecycle (`integration_full_vault_lifecycle`)

**What it tests**:
- Vault creation with owner and beneficiary
- Deposit of funds
- Check-in to extend TTL
- TTL extension verification
- Release trigger after expiry

**Expected duration**: ~2-3 minutes

**Success criteria**:
- Vault created with correct metadata
- Balance updated after deposit
- TTL extended after check-in
- Funds released to beneficiary after expiry

### 2. Vault Creation and Deposit (`integration_vault_creation_and_deposit`)

**What it tests**:
- Vault creation on testnet
- Initial balance is zero
- Deposit increases balance
- Balance query returns correct value

**Expected duration**: ~30 seconds

**Success criteria**:
- Vault exists with correct owner/beneficiary
- Balance matches deposit amount

### 3. Check-In and TTL Extension (`integration_checkin_extends_ttl`)

**What it tests**:
- Check-in extends vault TTL
- TTL extension matches check-in interval
- Multiple check-ins work correctly

**Expected duration**: ~1-2 minutes

**Success criteria**:
- TTL increases after check-in
- TTL increase equals check-in interval
- Multiple check-ins continue to extend TTL

### 4. Passkey Authentication (`integration_passkey_authentication`)

**What it tests**:
- Passkey-authenticated operations succeed
- Non-authenticated operations fail
- Passkey rotation works
- Old passkey is invalidated after rotation

**Expected duration**: ~1 minute

**Success criteria**:
- Valid passkey signature accepted
- Invalid passkey signature rejected
- Passkey rotation succeeds
- New passkey works, old passkey doesn't

### 5. Fee Calculation and Transfers (`integration_fee_calculation_and_transfers`)

**What it tests**:
- Deposit fees calculated correctly
- Withdrawal fees calculated correctly
- Token transfers are atomic
- Insufficient balance rejected

**Expected duration**: ~1 minute

**Success criteria**:
- Fees deducted from deposits
- Fees deducted from withdrawals
- Beneficiary receives correct amount
- Over-withdrawal rejected

### 6. Beneficiary Payout on Expiry (`integration_beneficiary_payout_on_expiry`)

**What it tests**:
- Funds released to beneficiary when TTL expires
- Release is atomic
- Released vault cannot be modified
- Beneficiary receives correct amount

**Expected duration**: ~2-3 minutes (includes TTL expiry wait)

**Success criteria**:
- Beneficiary receives funds
- Vault marked as released
- Deposit to released vault rejected

### 7. Multiple Vaults Isolation (`integration_multiple_vaults_isolation`)

**What it tests**:
- Multiple vaults are independent
- Operations on one vault don't affect others
- Each vault maintains separate state

**Expected duration**: ~1 minute

**Success criteria**:
- Vault A and B have independent balances
- Check-in on A doesn't affect B's TTL
- Release on A doesn't affect B's state

### 8. Error Handling (`integration_error_handling`)

**What it tests**:
- Invalid owner rejected
- Invalid beneficiary rejected
- Zero check-in interval rejected
- Negative amounts rejected
- Non-existent vault operations rejected

**Expected duration**: ~30 seconds

**Success criteria**:
- All invalid operations return errors
- No state changes on error

### 9. State Persistence (`integration_state_persistence`)

**What it tests**:
- Vault state persists across transactions
- Balance updates are durable
- TTL updates are durable
- Beneficiary updates are durable

**Expected duration**: ~1 minute

**Success criteria**:
- State identical after block confirmation
- Updates persist across queries

### 10. Network Latency Handling (`integration_network_latency_handling`)

**What it tests**:
- Operations complete within SLA
- Timeouts handled gracefully
- Retries work correctly

**Expected duration**: ~2 minutes

**Success criteria**:
- Operations complete within 30 seconds
- Timeout errors handled
- Retries succeed

## Implementing Integration Tests

### Basic Structure

```rust
#[test]
#[ignore]
fn integration_my_test() {
    let config = TestnetConfig::testnet();
    
    // Setup
    let (owner, beneficiary, vault_id) = setup_testnet_vault(&config)
        .expect("Failed to setup vault");
    
    // Execute
    // ... perform operations ...
    
    // Verify
    // ... assert expected state ...
}
```

### Connecting to Testnet

```rust
use soroban_sdk::Client;

let client = Client::new(config.rpc_url);
let contract = client.contract(config.contract_id.unwrap());
```

### Querying Contract State

```rust
let vault = contract.invoke(
    "get_vault",
    &[vault_id.into_val(&env)],
).await?;
```

### Waiting for Confirmation

```rust
wait_for_confirmation(1); // Wait for 1 ledger
```

## Troubleshooting

### Test Timeout

If tests timeout:
1. Check testnet RPC endpoint is responsive: `curl https://soroban-testnet.stellar.org/health`
2. Verify contract is deployed: `stellar contract read --network testnet --id <CONTRACT_ID>`
3. Increase timeout in test: `std::thread::sleep(Duration::from_secs(60))`

### Insufficient Balance

If tests fail with insufficient balance:
1. Fund test account via testnet faucet
2. Verify account has XLM: `stellar account info --network testnet --account <ACCOUNT>`

### Contract Not Found

If contract cannot be found:
1. Verify contract ID is correct
2. Verify contract is deployed to testnet
3. Check RPC endpoint is pointing to testnet

### State Conflicts

If tests fail due to state conflicts:
1. Use `--test-threads=1` to run tests sequentially
2. Use different test accounts for each test
3. Clean up state between tests

## CI Integration

Integration tests run on every PR:

```yaml
# .github/workflows/ci.yml
- name: Run integration tests
  if: github.event_name == 'pull_request'
  env:
    STELLAR_NETWORK: testnet
    CONTRACT_ID: ${{ secrets.TESTNET_CONTRACT_ID }}
  run: cargo test --package ttl-vault --test integration_tests -- --ignored --test-threads=1
```

## Performance Expectations

| Operation | Expected Time | Tolerance |
|-----------|---------------|-----------|
| Vault creation | 5-10s | ±2s |
| Deposit | 5-10s | ±2s |
| Check-in | 5-10s | ±2s |
| Withdrawal | 5-10s | ±2s |
| Release | 5-10s | ±2s |

Times include network latency and ledger confirmation.

## Best Practices

1. **Use unique test data**: Generate new addresses for each test to avoid conflicts
2. **Clean up state**: Delete test vaults after tests complete
3. **Log operations**: Use `println!` to track test progress
4. **Handle timeouts**: Wrap operations in timeout handlers
5. **Verify atomicity**: Ensure operations are all-or-nothing
6. **Test error paths**: Verify error handling works correctly
7. **Monitor costs**: Track transaction fees to optimize

## References

- [Soroban Testing Guide](https://soroban.stellar.org/docs/learn/testing)
- [Stellar Testnet](https://developers.stellar.org/docs/learn/networks)
- [Soroban RPC API](https://soroban-rpc.stellar.org/)

---

## Property-Based Test Suite

> Issue #1148: Add Property-Based Tests for TTL Extension Calculation

Property-based tests use [`proptest`](https://github.com/proptest-rs/proptest) to
generate thousands of random inputs and verify that key invariants hold across the
entire input space. This catches edge cases — such as overflow near u32/u64
boundaries — that hand-picked unit tests miss.

### Location

```
contracts/ttl_vault/tests/property_tests.rs
```

### Running Property Tests

```bash
# Run all property tests for ttl-vault
cargo test --package ttl-vault --test property_tests

# Run a specific property test
cargo test --package ttl-vault --test property_tests prop_vault_ttl_never_exceeds_max

# Increase the number of test cases (default: 256)
PROPTEST_CASES=10000 cargo test --package ttl-vault --test property_tests
```

### Invariants Verified

#### `vault_ttl_ledgers` function

The `vault_ttl_ledgers(check_in_interval)` function converts a check-in interval
(seconds) to a ledger count. The following invariants are verified across all
possible `u64` inputs:

| Test | Invariant |
|---|---|
| `prop_vault_ttl_never_exceeds_max` | Result never exceeds `MAX_PERSISTENT_TTL` (3,110,400 ledgers) and never overflows `u32::MAX` |
| `prop_vault_ttl_always_meets_minimum` | Result always ≥ `VAULT_TTL_LEDGERS_MIN` (200,000 ledgers) |
| `prop_vault_ttl_monotonically_non_decreasing` | Larger interval always produces ≥ ledger count |
| `prop_vault_ttl_within_one_ledger_of_expected` | Result is within ±1 ledger of analytical value (linear range) |
| `prop_vault_ttl_zero_interval_returns_floor` | Zero interval returns the minimum floor |
| `prop_vault_ttl_large_interval_returns_ceiling` | Very large interval returns the maximum ceiling |

#### TTL / check-in invariants

| Test | Invariant |
|---|---|
| `prop_ttl_always_increases_on_check_in` | TTL is monotonically non-decreasing after every check-in |
| `prop_ttl_post_checkin_equals_old_plus_interval` | New TTL equals old + interval (saturating) |
| `prop_ttl_after_checkin_gte_current_ledger` | Post-check-in TTL is always ≥ current ledger |

#### Balance invariants

| Test | Invariant |
|---|---|
| `prop_vault_balance_never_exceeds_deposits` | Balance never exceeds initial + total deposits |

#### Lifecycle / state-machine invariants

| Test | Invariant |
|---|---|
| `prop_vault_status_transitions_valid` | Vault always ends in a valid state; active vault TTL never falls below initial |
| `prop_no_double_release` | Funds are released at most once |

### CI Integration

Property tests run as part of the existing `./scripts/test.sh` and the CI workflow:

```yaml
- name: Run tests
  run: cargo test --package ttl-vault
```

The `proptest` dev-dependency is already declared in
`contracts/ttl_vault/Cargo.toml` under `[dev-dependencies]`.

---

## Full Vault Lifecycle Test (Issue #1178)

The `lifecycle_full_flow` test in `contracts/ttl_vault/tests/integration_tests.rs`
is a **non-ignored, in-process Soroban test** that exercises the complete vault
lifecycle without requiring a live Stellar node.

### What it tests

The test covers the following sequence end-to-end:

1. **`create_vault`** — Creates a new vault with a designated owner and beneficiary.
2. **`deposit`** — Transfers XLM from the owner to the vault contract.
3. **`check_in`** — Owner checks in to reset the countdown timer.
4. **Ledger advance** — Simulates the passage of time past the TTL using
   `env.ledger().with_mut(|l| l.timestamp += interval + 1)`.
5. **`trigger_release`** — Anyone calls trigger after expiry; funds flow to
   the beneficiary.

### Assertions

| Assertion | Verifies |
|---|---|
| `balance_after - balance_before == deposit_amount` | Beneficiary receives exactly the deposited amount |
| `vault.status == ReleaseStatus::Released` | Vault is in the `Released` state post-trigger |
| `vault.balance == 0` | Contract holds no residual funds after full release |

### Running the test

```bash
# Run only the lifecycle integration test
cargo test --package ttl-vault --test integration_tests lifecycle_full_flow

# Run all non-ignored tests in the integration test file
cargo test --package ttl-vault --test integration_tests
```

### Live testnet stubs

The `#[ignore]`-annotated stubs in the same file (e.g., `integration_full_vault_lifecycle`)
are placeholders for future live-network tests that require a funded account and
a deployed contract. Run them with:

```bash
cargo test --package ttl-vault --test integration_tests -- --ignored --test-threads=1
```

---

## Mutation Testing Workflow (Issue #1180)

### What is mutation testing and why it matters

Mutation testing automatically introduces small syntactic changes (_mutants_)
into the source code — such as flipping `>=` to `>`, negating a boolean, or
removing a function call — and then runs the full test suite against each
mutant. If the tests **pass** with the mutated code, that mutant **survives**,
indicating that no test is sensitive enough to detect the change.

For a financial smart contract like TTL-Legacy, surviving mutants are
particularly dangerous:

- A mutant in `trigger_release` could allow funds to be released prematurely
  or withheld indefinitely.
- A mutant in `check_in` could bypass the liveness countdown.
- A mutant in `withdraw` could allow over-withdrawal or bypass auth checks.

### Installation and usage

```bash
# Install cargo-mutants
cargo install cargo-mutants

# Run against the entire contract source
cargo mutants --package ttl-vault -- contracts/ttl_vault/src/

# Run against a specific file to speed up iteration
cargo mutants --package ttl-vault -- contracts/ttl_vault/src/lib.rs
```

Output summary example:

```
42 mutants tested in 8m 02s. 39 killed, 3 survived, 0 unviable.
```

Open `mutants.out/outcomes.json` for the full machine-readable results.

### Critical paths to focus on

| Function | Mutation risk | Recommended test approach |
|---|---|---|
| `trigger_release` | Funds sent to wrong address or wrong amount | Assert exact beneficiary delta + vault balance 0 |
| `check_in` | TTL not extended, or extended by wrong amount | Assert `last_check_in` updated; assert not expired after check-in |
| `withdraw` | Balance not debited, or auth not enforced | Assert balance decreases; assert non-owner fails |
| `is_expired` | Wrong boundary condition (`>=` vs `>`) | Test at exact expiry timestamp boundary |
| `deposit` | Amount not added, or wrong vault credited | Assert vault.balance == previous + amount |

### Suggested CI step (GitHub Actions)

Add the following job to your workflow to report mutant survival rate per PR
as a downloadable artifact. Results are **informational** and non-blocking
(`continue-on-error: true`).

```yaml
mutation-testing:
  name: Mutation Testing (informational)
  runs-on: ubuntu-latest
  continue-on-error: true   # Results are informational — do not block merges.
  steps:
    - uses: actions/checkout@v4

    - name: Install Rust stable
      uses: dtolnay/rust-toolchain@stable

    - name: Install cargo-mutants
      run: cargo install cargo-mutants

    - name: Run mutation tests
      run: cargo mutants --package ttl-vault 2>&1 | tee mutants-report.txt

    - name: Upload mutant survival report
      uses: actions/upload-artifact@v4
      with:
        name: mutant-survival-report
        path: mutants-report.txt
        retention-days: 14
```

See also `.github/workflows/mutation-testing.yml` for the full standalone workflow file.
