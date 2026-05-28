# Vault Hibernation

## Overview

Hibernation lets a vault owner temporarily suspend the check-in requirement for a fixed period, then resume normal operation automatically. This is useful for planned absences (travel, medical leave, etc.) where the owner knows they will be unavailable but does not want the vault to expire.

## How It Works

1. The owner calls `enter_hibernation(vault_id, caller, duration_seconds)`.
2. A `HibernationEntry` is stored on-chain recording `started_at` and `duration_seconds`.
3. While the hibernation window is open (`now < started_at + duration_seconds`), `is_expired` always returns `false` — the vault cannot be released.
4. Once the window closes, the vault's effective expiry deadline is extended by the full `duration_seconds`, so the owner has their normal `check_in_interval` remaining before the vault expires.
5. The owner can exit hibernation early by calling `exit_hibernation(vault_id, caller)`. The elapsed hibernation time is credited to `last_check_in`, preserving the remaining TTL.

## Expiry Logic

```
is_expired = now >= last_check_in + check_in_interval + hibernated_seconds
```

Where `hibernated_seconds` is:
- `0` if no hibernation entry exists
- `0` (and returns `false` immediately) if the hibernation window is still open
- `duration_seconds` once the window has closed

## API

### Enter Hibernation

```rust
enter_hibernation(vault_id: u64, caller: Address, duration_seconds: u64) -> Result<(), ContractError>
```

| Condition | Error |
|---|---|
| `caller` is not the vault owner | `NotOwner` (6) |
| Vault is not `Locked` | `AlreadyReleased` (7) |
| Vault is already hibernating | `AlreadyHibernating` (55) |
| `duration_seconds` is zero | `InvalidInterval` (2) |

Emits event: `hib_ent` with `(caller, started_at, duration_seconds)`

### Exit Hibernation (Early)

```rust
exit_hibernation(vault_id: u64, caller: Address) -> Result<(), ContractError>
```

| Condition | Error |
|---|---|
| `caller` is not the vault owner | `NotOwner` (6) |
| Vault is not `Locked` | `AlreadyReleased` (7) |
| Vault is not hibernating | `NotHibernating` (56) |

Emits event: `hib_ext` with `(caller, exited_at, elapsed_seconds)`

### Query Hibernation Status

```rust
get_hibernation(vault_id: u64) -> Option<HibernationEntry>
```

Returns `Some(HibernationEntry)` if the vault is currently hibernating, `None` otherwise.

```rust
pub struct HibernationEntry {
    pub started_at: u64,        // Unix timestamp when hibernation started
    pub duration_seconds: u64,  // Total hibernation duration
}
```

## Example

```rust
// Owner plans a 30-day absence
let thirty_days = 30 * 24 * 3600; // 2_592_000 seconds
client.enter_hibernation(&vault_id, &owner, &thirty_days)?;

// Vault will not expire for 30 days regardless of check-ins.
// Owner returns early after 10 days:
client.exit_hibernation(&vault_id, &owner)?;
// last_check_in is bumped by 10 days; normal TTL countdown resumes.
```

## Events

| Topic | Payload | Description |
|---|---|---|
| `hib_ent` | `(caller, started_at, duration_seconds)` | Vault entered hibernation |
| `hib_ext` | `(caller, exited_at, elapsed_seconds)` | Vault exited hibernation (early or natural) |

## Security Considerations

- Only the vault owner can enter or exit hibernation.
- Hibernation does not affect deposits, withdrawals, or check-ins — those remain available.
- A vault cannot enter hibernation twice simultaneously; the existing entry must expire or be exited first.
- Hibernation duration is bounded by the same `max_ttl_seconds` constraint applied to check-ins (the combined `check_in_interval + duration_seconds` is used when computing storage TTL).
