# Duplicate Vault Prevention

## Overview

TTL-Legacy prevents accidental creation of duplicate vaults — vaults with identical `(owner, beneficiary, check_in_interval)` parameters — by maintaining an on-chain fingerprint registry. A second `create_vault` call with the same triple is rejected with `DuplicateVault` (error 57) as long as the original vault is still active (`Locked`).

## How It Works

On every successful `create_vault`:

1. A SHA-256 fingerprint is computed over `owner || beneficiary || check_in_interval`.
2. The fingerprint is stored under `DataKey::VaultFingerprint(hash)` mapping to the existing `vault_id`.
3. If a fingerprint already exists when `create_vault` is called, a `dup_vlt` event is emitted (carrying the conflicting `vault_id`) and the call panics with `DuplicateVault`.

The fingerprint is removed when the vault leaves the `Locked` state:
- `cancel_vault` — owner cancels the vault
- `trigger_release` — vault expires and funds are released

This means the same parameters can be reused once the original vault is no longer active.

## Fingerprint Key

```
fingerprint = sha256(owner.to_xdr() || beneficiary.to_xdr() || check_in_interval_be_bytes)
```

Stored as `DataKey::VaultFingerprint(BytesN<32>)` → `vault_id: u64`.

## What Counts as a Duplicate

Two vaults are duplicates if and only if all three of these match:

| Field | Must match |
|---|---|
| `owner` | ✅ |
| `beneficiary` | ✅ |
| `check_in_interval` | ✅ |

Changing any one of these creates a distinct vault and is allowed.

## Error

```rust
ContractError::DuplicateVault = 57
```

## Event

A `dup_vlt` event is emitted **before** the panic so off-chain indexers can observe the attempt:

| Topic | Payload |
|---|---|
| `dup_vlt` | `(owner, beneficiary, check_in_interval, existing_vault_id)` |

## Example

```rust
// First vault — succeeds
let id = client.create_vault(&owner, &beneficiary, &3600)?;

// Exact same params — rejected
client.create_vault(&owner, &beneficiary, &3600)?; // DuplicateVault (57)

// Different interval — allowed
client.create_vault(&owner, &beneficiary, &7200)?; // OK

// After cancelling the original:
client.cancel_vault(&id, &owner)?;
client.create_vault(&owner, &beneficiary, &3600)?; // OK — fingerprint was cleared
```
