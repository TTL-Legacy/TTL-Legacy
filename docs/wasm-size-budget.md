# WASM Size Budget

## Overview

The TTL-Vault contract WASM size is monitored in CI to prevent regressions that would impact deployment costs and instruction budget constraints on Soroban.

## Budget Threshold

**256 KB** — Maximum recommended WASM size for `ttl_vault` contract

### Rationale

- **Deployment Cost**: Soroban charges per byte uploaded; every kilobyte increases deployment fees
- **Instruction Budget**: Soroban has hard upload limits; staying well below 512 KB provides safety margin
- **Performance**: Smaller WASM loads faster and uses less memory during contract execution
- **Read-Heavy Paths**: Frontends and indexers cache contract metadata; smaller binaries reduce bandwidth

## Monitoring

The CI pipeline checks the WASM size on every push and pull request:

```bash
cargo build --package ttl-vault --target wasm32-unknown-unknown --release
stat -c%s target/wasm32-unknown-unknown/release/ttl_vault.wasm
```

If the size exceeds 256 KB, the CI build will fail with a clear message indicating the overage.

## Optimization Strategies

If the WASM size grows beyond the threshold, consider these optimizations:

### 1. Enable LTO (Link-Time Optimization)

Edit `contracts/ttl_vault/Cargo.toml`:

```toml
[profile.release]
lto = true
codegen-units = 1
```

### 2. Strip Unnecessary Dependencies

Review `Cargo.toml` for unused or redundant dependencies. Audit transitive dependencies:

```bash
cargo tree --package ttl-vault --duplicates
```

### 3. Reduce Debug Symbols

Ensure `strip = true` in release profile:

```toml
[profile.release]
strip = true
```

### 4. Use `wasm-opt`

After building, optimize with `binaryen`:

```bash
npm install -g binaryen
wasm-opt -Oz target/wasm32-unknown-unknown/release/ttl_vault.wasm -o ttl_vault.wasm
```

### 5. Refactor Large Functions

Break monolithic functions into smaller, modular components to improve compiler optimization.

## Updating the Threshold

If legitimate growth requires increasing the threshold:

1. Justify the increase in the PR description
2. Update both the CI step and this document
3. Ensure the new threshold still leaves adequate margin below Soroban's limits
4. Tag as a breaking change if it affects deployment pipeline

## CI Failure Example

```
❌ WASM size exceeds threshold by 50 KB
WASM size: 306 KB (threshold: 256 KB)
```

Action: Apply one or more optimizations above, then re-push.

## zk_verifier Contract Budget

### Threshold

**256 KB** — Maximum recommended WASM size for the `zk_verifier` contract

### Rationale

- **Deployment Cost**: Soroban charges per byte uploaded; every kilobyte increases deployment fees for the verifier contract
- **Instruction Budget**: Verification paths are invoked frequently; a smaller binary keeps upload and execution costs predictable
- **Performance**: Smaller WASM loads faster and uses less memory during proof verification
- **Parity with Vault**: Keeping both contracts under the same threshold simplifies monitoring and keeps CI checks consistent

### Monitoring

The CI pipeline checks the `zk_verifier` WASM size on every push and pull request:

```bash
cargo build --package zk_verifier --target wasm32-unknown-unknown --release
stat -c%s target/wasm32-unknown-unknown/release/zk_verifier.wasm
```

If the size exceeds 256 KB, the CI build will fail with a clear message indicating the overage.

### Optimization Strategies

The same optimizations described above apply to `contracts/zk_verifier`:

- Enable LTO and `codegen-units = 1` in `contracts/zk_verifier/Cargo.toml`
- Audit dependencies with `cargo tree --package zk_verifier --duplicates`
- Ensure `strip = true` in the release profile
- Run `wasm-opt -Oz` on the built artifact
- Refactor large verification functions into smaller, modular components

### CI Failure Example

```
❌ zk_verifier WASM size exceeds threshold by 50 KB
WASM size: 306 KB (threshold: 256 KB)
```

Action: Apply one or more optimizations above, then re-push.
