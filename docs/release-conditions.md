# Release Conditions

A TTL-Legacy vault releases its funds when one or more configured *release conditions* are satisfied. By default a vault uses `TTLExpiry` — it releases automatically once the owner misses a check-in deadline. The owner can override this or combine multiple conditions using `set_release_conditions`.

## Overview

```
Vault (Locked)
  │
  ├─ set_release_conditions(vault_id, owner, [condition, ...])
  │
  └─ trigger_release(vault_id)   ← checks all conditions; any one satisfied → release
       or
     trigger_release_manual(vault_id)  ← satisfies OwnerInitiated condition
```

The contract evaluates conditions in the order they are stored. Release proceeds as soon as **any** condition in the list is met (OR logic). Once released, `ReleaseAttempted` is set and the vault cannot be released again.

## Condition Types

### `TTLExpiry`

**What it means:** Release when the vault's TTL countdown reaches zero — that is, when the owner has not called `check_in` within `check_in_interval` seconds of the last check-in.

**Configuration fields:**

| Field | Set via | Description |
|---|---|---|
| `check_in_interval` | `create_vault` / `update_check_in_interval` | Seconds the owner has between check-ins |
| `last_check_in` | updated on every `check_in` call | Ledger timestamp of the most recent check-in |
| `grace_period` | `set_release_grace_period` (admin) | Extra seconds after expiry before release is allowed |

The vault is considered expired when:
```
now ≥ last_check_in + check_in_interval + hibernated_seconds + grace_period
```

`hibernated_seconds` accumulates while the vault is in hibernation mode (see [`docs/hibernation.md`](hibernation.md)).

**Evaluation order:** `TTLExpiry` is checked first in `trigger_release_internal`. If the TTL has not elapsed the call reverts with `NotExpired` (when it is the only condition) or silently skips to the next condition.

**Example:**

```rust
// Default: vault releases automatically on TTL expiry (no explicit set needed)
let vault_id = client.create_vault(&owner, &beneficiary, &check_in_interval, &None);

// Or set it explicitly alongside another condition
client.set_release_conditions(
    &vault_id,
    &owner,
    &vec![&env, ReleaseCondition::TTLExpiry],
)?;

// After TTL elapses, anyone can trigger release
client.trigger_release(&vault_id);
```

---

### `OwnerInitiated` (ManualRelease)

**What it means:** Release when the vault owner explicitly decides to release early, before the TTL has expired. This is the "manual override" path.

**Configuration fields:** None — there are no extra fields. The condition is satisfied solely by calling `trigger_release_manual`.

**Evaluation order:** Checked when `trigger_release_manual` is called (passes `ReleaseTrigger::Manual` internally). A `trigger_release` call (which passes `ReleaseTrigger::Expiry`) does **not** satisfy this condition.

**Constraints:**
- Caller must be the vault owner when calling `trigger_release_manual`.
- Vault must be `Locked` and not frozen, paused, or owner-locked.
- Beneficiary veto applies: if the beneficiary has called `veto_release_condition`, the release is blocked even after the owner initiates it.

**Example:**

```rust
// Set OwnerInitiated as the (only) condition
client.set_release_conditions(
    &vault_id,
    &owner,
    &vec![&env, ReleaseCondition::OwnerInitiated],
)?;

// Owner decides to release early
client.trigger_release_manual(&vault_id);
```

Combine with `TTLExpiry` to allow either path:

```rust
client.set_release_conditions(
    &vault_id,
    &owner,
    &vec![&env, ReleaseCondition::TTLExpiry, ReleaseCondition::OwnerInitiated],
)?;
```

---

### `Oracle(Address)` (OracleTrigger)

**What it means:** Release when an on-chain oracle contract returns `true` for this vault. This delegates the release decision to an external data source — price feed, identity verifier, proof-of-life service, or any custom logic that exposes the oracle interface.

**Configuration fields:**

| Field | Type | Description |
|---|---|---|
| `oracle_address` | `Address` | Contract address that implements the oracle interface |

The oracle contract must expose a `query(env, vault_id) -> bool` entry point. The TTL Vault contract calls it during `trigger_release_internal`; a return of `true` satisfies the condition, `false` or any error (via `unwrap_or(false)`) does not.

**Evaluation order:** Checked after `TTLExpiry` in the default iteration order. The vault owner can reorder conditions by calling `set_release_conditions` with the preferred slice order.

**Constraints:**
- The oracle address must be a deployed contract, not an account address.
- The vault owner sets the oracle with `set_release_condition` (single-condition API) or inside a `Vec` passed to `set_release_conditions`.
- The oracle is queried at release time, not at configuration time — make sure the oracle contract is live when `trigger_release` is called.

**Example:**

```rust
// Deploy or identify your oracle contract
let oracle_address = Address::from_string(&env, "CXXX...");

// Configure vault to release when the oracle approves
client.set_release_condition(
    &vault_id,
    &owner,
    &ReleaseCondition::Oracle(oracle_address.clone()),
)?;

// When the oracle returns true, anyone can trigger release
client.trigger_release(&vault_id);
```

Combine oracle with TTL expiry as a fallback:

```rust
client.set_release_conditions(
    &vault_id,
    &owner,
    &vec![
        &env,
        ReleaseCondition::Oracle(oracle_address),
        ReleaseCondition::TTLExpiry,       // fallback if oracle is unavailable
    ],
)?;
```

---

## Multi-Signature Release (MultiSigApproval)

Multi-signature approval is not a standalone `ReleaseCondition` variant. Instead it gates the operations that *set* or *trigger* a condition via the multi-sig proposal workflow.

To require multi-sig approval before a vault can be released:

1. Configure multi-sig on the vault (see [`docs/multi-sig.md`](multi-sig.md)).
2. Set `OwnerInitiated` as the release condition.
3. When the owner wants to release, they create a multi-sig proposal for `OwnerInitiated` release.
4. Co-signers approve; once the threshold is met the owner executes the proposal, which calls `trigger_release_manual` internally.

This arrangement means no single key can release the vault unilaterally.

---

## Scheduled Release (ScheduledRelease)

A time-scheduled release is achieved by combining `OwnerInitiated` with a withdrawal schedule rather than a separate `ReleaseCondition` variant. The owner pre-authorises a release at a specific ledger timestamp using `set_withdrawal_schedule`, and the backend scheduler calls `execute_scheduled_withdrawal` at the configured time.

For milestone-gated or oracle-confirmed scheduled releases, combine the `Oracle` condition with an oracle contract that checks the current ledger timestamp against a target.

---

## API Reference

### Single-condition API (legacy)

```rust
/// Set one release condition, replacing any existing single condition.
/// Does not affect the conditions stored via set_release_conditions.
set_release_condition(
    env: Env,
    vault_id: u64,
    caller: Address,          // must be vault owner
    condition: ReleaseCondition,
) -> Result<(), ContractError>

/// Get the single condition stored in the vault struct.
get_release_condition(env: Env, vault_id: u64) -> ReleaseCondition
```

### Multi-condition API (recommended)

```rust
/// Set a list of release conditions. Any one being satisfied triggers release.
/// Replaces any previously stored conditions.
set_release_conditions(
    env: Env,
    vault_id: u64,
    caller: Address,              // must be vault owner
    conditions: Vec<ReleaseCondition>,
) -> Result<(), ContractError>

/// Get all conditions. Returns [TTLExpiry] by default if none were explicitly set.
get_release_conditions(env: Env, vault_id: u64) -> Vec<ReleaseCondition>
```

### Triggering release

```rust
/// Satisfy TTLExpiry or Oracle conditions.
trigger_release(env: Env, vault_id: u64)

/// Satisfy OwnerInitiated condition. Caller must be the vault owner.
trigger_release_manual(env: Env, vault_id: u64)
```

---

## Evaluation Order and OR Logic

When `set_release_conditions` has stored a non-empty list, the contract iterates through it and sets `condition_met = true` on the first match. The list order determines which condition is evaluated first, which matters if conditions have side effects (oracle calls) or when diagnosing revert reasons.

If `set_release_conditions` has never been called, the contract falls back to the single `release_condition` field on the vault struct (default `TTLExpiry`).

```
stored_conditions is empty?
  └─ Yes → evaluate vault.release_condition (TTLExpiry / OwnerInitiated / Oracle)
  └─ No  → iterate stored_conditions, stop at first match
```

---

## Beneficiary Veto

Before the TTL has elapsed, the beneficiary can call `veto_release_condition` to block an early release triggered by `OwnerInitiated` or `Oracle`. The veto has no effect once the TTL has expired.

```rust
// Beneficiary blocks an early release
veto_release_condition(env, vault_id, beneficiary)

// Veto can be removed by the beneficiary
remove_release_condition_veto(env, vault_id, beneficiary)
```

---

## Events

| Topic | Emitted when |
|---|---|
| `release` | Vault is successfully released (any condition) |
| `cond_acc` | Beneficiary accepts release conditions set by the owner |

---

## Error Reference

| Code | Constant | Condition |
|---|---|---|
| `NotExpired` | `26` | `TTLExpiry` condition not met when it is the sole condition |
| `ConditionsNotApproved` | varies | No condition in the list was satisfied |
| `GracePeriodActive` | varies | TTL elapsed but grace period has not yet passed |
| `BeneficiaryVetoReleaseCondition` | `109` | Beneficiary vetoed early release |
| `AlreadyReleased` | varies | Vault is not `Locked`; cannot release again |
