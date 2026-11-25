# Oracle Integration Guide

TTL-Legacy supports two oracle types: **release-condition oracles** that determine whether a vault may be released, and **identity oracles** that attest whether a beneficiary has been KYC-verified. This guide explains how to register a new oracle, the expected signed payload format, the key rotation process, and how to test oracle-triggered releases on testnet.

---

## Oracle Types

### Release-Condition Oracle

A release-condition oracle answers a single yes/no question: _are the conditions for releasing this vault met?_ When a vault's `ReleaseCondition` is set to `Oracle(address)`, `trigger_release` calls the oracle before transferring funds.

**Interface required** (`oracle.rs`):

```rust
trait OracleInterface {
    fn query_release(env: Env) -> bool;
}
```

The oracle contract must expose `query_release` with no additional arguments. It returns `true` to approve release or `false` to block it. Any call failure (trap, panic) is treated as `false` — the vault is not released.

### Identity Oracle

An identity oracle answers whether a specific beneficiary address has passed KYC verification. When a vault has a `BeneficiaryIdentityOracle` configured, `verify_beneficiary_identity` queries the oracle before allowing a beneficiary to claim funds.

**Interface required** (`oracle.rs`):

```rust
trait IdentityOracleInterface {
    fn is_verified(env: Env, beneficiary: Address) -> bool;
}
```

The oracle contract must expose `is_verified(env, beneficiary)` returning `true` if the beneficiary is verified. Any call failure is treated as `false`.

---

## Registering a New Oracle

### Release-Condition Oracle

1. Deploy your oracle contract on Stellar. It must implement `query_release() -> bool`.

2. The vault owner sets the oracle as the vault's release condition:

   ```rust
   set_release_condition(vault_id, caller, ReleaseCondition::Oracle(oracle_address))
   ```

   Or, to combine it with other conditions (e.g. TTL expiry _and_ oracle approval):

   ```rust
   set_release_conditions(vault_id, caller, vec![
       ReleaseCondition::TTLExpiry,
       ReleaseCondition::Oracle(oracle_address),
   ])
   ```

   Only the vault owner can call either function.

3. Verify the registration:

   ```rust
   get_release_condition(vault_id)   // -> ReleaseCondition
   get_release_conditions(vault_id)  // -> Vec<ReleaseCondition>
   ```

### Identity Oracle

1. Deploy your identity oracle contract. It must implement `is_verified(env, beneficiary) -> bool`.

2. The vault owner registers the oracle address:

   ```rust
   set_beneficiary_identity_oracle(vault_id, oracle_address)
   ```

   This emits a `ben_id_or` event with the oracle address.

3. Anyone may then trigger an on-chain attestation for a specific beneficiary:

   ```rust
   verify_beneficiary_identity(vault_id, beneficiary)
   ```

   This calls `is_verified` on the registered oracle. On success, the contract stores a `BeneficiaryIdentityVerification` record and emits `ben_id_vf`. On failure, it returns `BeneficiaryNotVerified` (error 160).

---

## Signed Payload Format

The TTL-Legacy contract invokes oracle contracts via Soroban cross-contract calls, not via signed messages. The oracle contract is responsible for its own authentication and data sourcing. However, oracle providers that use off-chain data feeds typically follow this pattern:

### Attestation Pattern

1. An off-chain data source generates a result (e.g. price feed, compliance check outcome).
2. A trusted signer signs a payload using an Ed25519 or secp256k1 key known to the oracle contract.
3. The oracle contract stores or verifies the signed payload on-chain.
4. When TTL-Legacy queries `query_release()`, the oracle contract reads the stored attestation and returns the result.

### Freshness Enforcement

The `attest_milestone` function enforces a **5-minute freshness window** (`ORACLE_FRESHNESS_WINDOW = 300` seconds) on oracle timestamps. If your oracle supplies a signed timestamp with its attestation, the contract verifies:

```
now - oracle_timestamp <= 300 seconds
```

Payloads with a stale timestamp are rejected with `OracleAttestationStale` (error 159).

**Recommended payload structure for oracle attestations:**

```json
{
  "vault_id": "<u64>",
  "condition_met": true,
  "timestamp": "<unix_seconds>",
  "nonce": "<unique_per_attestation>"
}
```

The signed bytes should be `SHA-256(canonical_json(payload))`. The nonce prevents replay within the freshness window.

---

## Key Rotation for Oracle Providers

Oracle contracts typically hold a signing key or rely on an admin address that can update the state the contract reads. Key rotation is handled inside the oracle contract itself; TTL-Legacy does not manage oracle signing keys.

The recommended rotation process:

1. Deploy a new oracle contract (or update the signing key inside the existing oracle contract, if it supports key rotation).
2. The vault owner updates the release condition to point to the new oracle address:
   ```rust
   set_release_condition(vault_id, caller, ReleaseCondition::Oracle(new_oracle_address))
   ```
   For identity oracles:
   ```rust
   set_beneficiary_identity_oracle(vault_id, new_oracle_address)
   ```
3. Verify the old oracle address is no longer referenced via `get_release_condition` or `get_release_conditions`.

There is no on-chain timelock on oracle address changes; the update takes effect in the same ledger as the transaction.

---

## Testing Oracle-Triggered Releases on Testnet

### Prerequisites

- Stellar testnet account with XLM funded via Friendbot.
- Soroban CLI installed (`stellar contract`).
- Contract WASM built for the ttl_vault contract.

### Step 1 — Deploy a Mock Oracle Contract

Create a minimal oracle that always returns `true`:

```rust
#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct MockOracle;

#[contractimpl]
impl MockOracle {
    pub fn query_release(_env: Env) -> bool {
        true
    }
}
```

Deploy it:

```bash
stellar contract deploy \
  --wasm mock_oracle.wasm \
  --source <YOUR_SECRET_KEY> \
  --network testnet
# Returns: ORACLE_CONTRACT_ID
```

### Step 2 — Create a Vault with an Oracle Release Condition

```bash
stellar contract invoke \
  --id <TTL_VAULT_CONTRACT_ID> \
  --source <OWNER_SECRET_KEY> \
  --network testnet \
  -- create_vault \
  --owner <OWNER_ADDRESS> \
  --beneficiary <BENEFICIARY_ADDRESS> \
  --check_in_interval 86400 \
  --token <XLM_TOKEN_ADDRESS>
# Note the returned vault_id
```

```bash
stellar contract invoke \
  --id <TTL_VAULT_CONTRACT_ID> \
  --source <OWNER_SECRET_KEY> \
  --network testnet \
  -- set_release_condition \
  --vault_id <VAULT_ID> \
  --caller <OWNER_ADDRESS> \
  --condition '{"oracle": "<ORACLE_CONTRACT_ID>"}'
```

### Step 3 — Deposit Funds and Let the Vault Expire

```bash
stellar contract invoke \
  --id <TTL_VAULT_CONTRACT_ID> \
  --source <OWNER_SECRET_KEY> \
  --network testnet \
  -- deposit \
  --vault_id <VAULT_ID> \
  --caller <OWNER_ADDRESS> \
  --amount 1000000
```

Wait for the vault's TTL to expire (or use a short `check_in_interval` during testing), then wait for the grace period to elapse.

### Step 4 — Trigger Release

```bash
stellar contract invoke \
  --id <TTL_VAULT_CONTRACT_ID> \
  --source <BENEFICIARY_SECRET_KEY> \
  --network testnet \
  -- trigger_release \
  --vault_id <VAULT_ID>
```

The contract will call `query_release` on `ORACLE_CONTRACT_ID`. If it returns `true`, funds are transferred to the beneficiary.

### Step 5 — Test a Blocking Oracle

Replace the mock oracle with one that returns `false`:

```rust
pub fn query_release(_env: Env) -> bool {
    false
}
```

Deploy and update the vault's release condition as in Step 2. `trigger_release` will return `ConditionsNotApproved` (error 33).

---

## Errors Related to Oracle Integration

| Error | Code | Meaning |
| :--- | :--- | :--- |
| `ConditionsNotApproved` | 33 | Oracle returned `false`; release blocked. |
| `OracleAttestationStale` | 159 | Attestation timestamp is outside the 5-minute freshness window. |
| `BeneficiaryNotVerified` | 160 | Identity oracle does not attest this beneficiary. |
| `IdentityOracleNotSet` | 162 | No identity oracle registered for the vault. |

---

## Reference: Oracle Invocation in `trigger_release`

Inside `trigger_release_internal`, the release condition is evaluated as follows:

```rust
ReleaseCondition::Oracle(ref addr) => {
    // Returns false on call failure — vault is not released.
    oracle::query(env, addr).unwrap_or(false)
}
```

For multi-condition vaults (`set_release_conditions`), all listed conditions must be satisfied. An `Oracle` condition in the list undergoes the same `query_release` call.

For identity-gated vaults, `verify_beneficiary_identity` must have been called (and succeeded) before `trigger_release` will transfer to that beneficiary.
