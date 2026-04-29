# TTL & State Archival Logic

## Overview

TTL-Legacy uses Stellar's Time-to-Live (TTL) and state archival features to automate inheritance without manual intervention.

## How TTL Works

Each vault tracks:
- `last_check_in`: Timestamp of last owner check-in
- `check_in_interval`: Duration (seconds) before vault expires

## Expiry Detection

```rust
pub fn is_expired(env: Env, vault_id: u64) -> bool {
    let vault = Self::load_vault(&env, vault_id);
    let current_time = env.ledger().timestamp();
    current_time >= vault.last_check_in + vault.check_in_interval
}
```

## Check-In Flow

1. Owner calls `check_in(vault_id)`
2. Contract updates `last_check_in` to current timestamp
3. TTL countdown resets

## Release Flow

1. Anyone calls `trigger_release(vault_id)`
2. Contract checks `is_expired()`
3. If expired: transfers funds to beneficiary
4. If not expired: returns `ContractError::NotExpired`

## State Archival

Soroban archives inactive contract state to reduce costs. TTL-Legacy extends TTL on:
- Vault creation
- Check-ins
- Deposits
- Withdrawals

This ensures vault data remains accessible while the owner is active.

## Vault Archival and Restoration

If an owner stops all activity, the vault's persistent storage entry will eventually be archived by the Soroban network. Archived entries are not deleted — they can be restored by re-extending their TTL.

### Detecting Archival

Operators can snapshot vault state before archival using off-chain tooling. The snapshot is stored under `DataKey::ArchivedVault(vault_id)` and can be queried:

```rust
get_archived_vault_info(vault_id) -> Option<ArchivedVaultInfo>
```

Returns `Some(ArchivedVaultInfo)` if a snapshot exists, `None` if the vault is live or was never snapshotted.

### Restoring an Archived Vault

Anyone can restore an archived vault by calling:

```rust
restore_vault(vault_id)
```

This re-extends the persistent entry TTL so the vault becomes accessible again. It also removes any stale archived-info snapshot and emits a `v_restore` event.

### Automatic Restoration in `trigger_release`

`trigger_release` automatically attempts to restore an archived vault before transferring funds. If an archived-info snapshot is present, the vault entry TTL is extended before the release logic runs. This ensures beneficiaries can always trigger release without a separate manual restoration step.

```
trigger_release(vault_id)
  └─ try_restore_archived_vault()   ← extends TTL if snapshot present
  └─ load_vault()
  └─ is_expired() check
  └─ transfer funds to beneficiary
```

## Archival and Restoration

When a vault's TTL expires, Soroban may archive the vault data to reduce ledger bloat. Archived vaults cannot be accessed directly.

To restore an archived vault:

1. Call `get_archived_vault_info(vault_id)` to check if archived metadata exists
2. Call `restore_vault(vault_id)` to restore the vault state
3. The vault becomes accessible again with extended TTL

The `trigger_release` function automatically attempts restoration if the vault is archived.
