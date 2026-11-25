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

### Emitted Events

| Event | Topic | When | Data |
|-------|-------|------|------|
| `CheckInRecorded` | `ci_rec` | On every successful check-in | `(owner, new_expiry)` |
| `CheckIn` | `check_in` | On every successful check-in | `timestamp` |
| `CheckInGeo` | `ci_geo` | On geo check-in | `(lat, lon, country, timestamp)` |
| `CheckInPoW` | `ci_pow` | On PoW check-in | `(nonce, difficulty, timestamp)` |
| `CheckInRateLimited` | `ci_rl` | When rate-limited | `cooldown_remaining` |

The `CheckInRecorded` event (`ci_rec`) is emitted immediately after the `check_in` topic in all check-in paths: `check_in`, `batch_check_in`, `sync_vault_ttls`, and `batch_check_in_v2`.

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

## Beneficiary Delegation

Beneficiaries can delegate their role to another address, creating a chain of custody.

### Delegation

The beneficiary (or the current delegate) can call:
```rust
delegate_beneficiary_role(vault_id, delegate_address)
```

This updates the delegation chain and emits a `del_ben` event.

The `trigger_release` function automatically attempts restoration if the vault is archived.

## TTL Borrowing (Emergency)

Vault owners can temporarily borrow TTL from another vault they own during emergencies:

```rust
borrow_ttl(borrower_vault_id, lender_vault_id, caller, borrow_seconds) -> Result<(), ContractError>
repay_ttl_borrow(borrower_vault_id, caller) -> Result<(), ContractError>
get_ttl_borrow(borrower_vault_id) -> Option<TtlBorrowRecord>
```

- The lender vault's `last_check_in` is reduced by `borrow_seconds` (shortening its TTL)
- The borrower vault's `last_check_in` is extended by `borrow_seconds` (pushing its expiry forward)
- A `TtlBorrowRecord` is stored on-chain for auditability
- The borrow can be repaid to restore the lender's TTL
- Events: `ttl_bor` (borrow created), `ttl_rep` (borrow repaid)

## Check-in Rate Limiting

To prevent storage abuse from excessive check-ins, a minimum cooldown can be enforced:

```rust
set_min_checkin_cooldown(cooldown_seconds)   // admin-only
get_min_checkin_cooldown() -> u64
get_last_checkin_time(vault_id) -> Option<u64>
```

- Default cooldown: 60 seconds
- Set to 0 to disable rate limiting
- Check-ins within the cooldown window return `CheckInTooFrequent` (error 54)
- Event: `ci_rl` emitted when the cooldown setting is updated

## Accelerated TTL Decay

Owners can voluntarily shorten their vault's remaining TTL to make it expire sooner:

```rust
accelerate_ttl_decay(vault_id, caller, accelerate_by_seconds) -> Result<(), ContractError>
```

- Reduces `last_check_in` by `accelerate_by_seconds`, moving the expiry deadline forward
- Capped at 30 days (`MAX_ACCELERATE_SECONDS = 2_592_000`) per call
- Cannot push expiry to the current time or past (must leave ≥ 1 second remaining)
- Returns `InsufficientTtlToAccelerate` (error 55) if the remaining TTL is too small
- Event: `ttl_acc` with `(accelerated_seconds, new_remaining_ttl)`

## Geographic Check-in Tracking

Check-ins can include geographic location metadata for security and anomaly detection:

```rust
check_in_with_geo(
    vault_id, caller, passkey_hash,
    latitude_micro, longitude_micro, country_code
) -> Result<(), ContractError>
get_geo_checkin_log(vault_id) -> Vec<GeoCheckInEntry>
```

- `latitude_micro` / `longitude_micro` are in microdegrees (e.g. `37_422_000` = 37.422°)
- `country_code` is an ISO 3166-1 alpha-2 string (e.g. `"US"`)
- All standard `check_in` validations apply (owner auth, rate limiting, passkey expiry, TTL cap)
- Location history is stored persistently under `CheckInGeoLog(vault_id)` and queryable on-chain
- Off-chain indexers can detect anomalies (e.g. check-in from unexpected country)
- Events: `ci_geo` (location + timestamp) and `check_in` (standard check-in event)
