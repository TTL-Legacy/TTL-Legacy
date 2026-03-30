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
