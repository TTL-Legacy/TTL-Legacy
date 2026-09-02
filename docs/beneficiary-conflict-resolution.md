# Beneficiary Conflict Resolution

**Issue**: #502, #1297  
**Status**: Implemented  
**Last Updated**: 2026-09-01

## Overview

Beneficiary Conflict Resolution provides **automated, deterministic** conflict resolution when multiple addresses claim the same vault as beneficiary. The system prevents deadlock by applying transparent resolution rules without requiring admin intervention in the common case.

### Resolution Rules (applied in order)

1. **Owner-designated priority**: If the vault owner has called `set_conflict_priority_beneficiary` and the designated address has filed a claim, that address wins.
2. **First-registered wins**: The claimant with the earliest `filed_at` timestamp wins.

Admins retain the ability to override manually at any time during an active dispute window.

## Use Cases

1. **Multiple Claimants**: When multiple parties claim to be the rightful beneficiary
2. **Disputed Inheritance**: When beneficiary status is contested
3. **Automated Resolution**: Deterministic, permissionless resolution after the dispute window closes
4. **Owner Priority Override**: Vault owner designates a preferred claimant before auto-resolution
5. **Admin Override**: Administrator manually resolves an escalated dispute

## API

### `file_beneficiary_conflict(vault_id: u64, reason: String) -> Result<(), ContractError>`

**Caller**: Current vault beneficiary  
**Auth**: Required (beneficiary)

The current vault beneficiary files a conflict claim. On the **first** claim, the dispute window deadline is calculated and stored.

**Errors**: `InvalidAmount` (empty reason), `ConflictAlreadyResolved` (conflict already settled)

**Events**: `BENEFICIARY_CONFLICT_FILED_TOPIC` (`ben_conf`)

---

### `claim_beneficiary_conflict(vault_id: u64, claimant: Address, reason: String) -> Result<(), ContractError>`

**Caller**: Any address  
**Auth**: Required (`claimant` must sign)

Allows **any** address to assert a competing claim for a vault — not just the current beneficiary. This is the primary entry point for competing claimants. On the **first** claim, the dispute window deadline is calculated and stored.

**Errors**: `InvalidAmount` (empty reason), `ConflictAlreadyResolved` (conflict already settled)

**Events**: `CONFLICT_CLAIMED_TOPIC` (`conf_clm`)

**Example**:
```rust
client.claim_beneficiary_conflict(&vault_id, &claimant_b, &String::from_str(&env, "Rightful heir"))?;
```

---

### `set_conflict_dispute_window(vault_id: u64, caller: Address, duration_seconds: u64) -> Result<(), ContractError>`

**Caller**: Vault owner  
**Auth**: Required (`caller` must be vault owner)

Sets the dispute window duration for a vault before any claims are filed. Once the first claim is recorded the window deadline is fixed.

**Bounds**: `MIN_CONFLICT_DISPUTE_WINDOW` (1 hour) to `MAX_CONFLICT_DISPUTE_WINDOW` (30 days).  
**Default**: `DEFAULT_CONFLICT_DISPUTE_WINDOW` = 72 hours (if not explicitly set).

**Errors**: `NotOwner`, `InvalidAmount` (out of range)

**Events**: `CONFLICT_DISPUTE_WINDOW_SET_TOPIC` (`conf_dw`)

---

### `set_conflict_priority_beneficiary(vault_id: u64, caller: Address, priority_beneficiary: Address) -> Result<(), ContractError>`

**Caller**: Vault owner  
**Auth**: Required (`caller` must be vault owner)

Designates an address to win during auto-resolution, overriding filing order. The priority address **must also have filed a claim** — if not, auto-resolution falls back to first-registered.

Can be called at any time while the conflict is `Pending`.

**Errors**: `NotOwner`, `ConflictAlreadyResolved`

**Events**: `CONFLICT_PRIORITY_SET_TOPIC` (`conf_pri`)

**Example**:
```rust
client.set_conflict_priority_beneficiary(&vault_id, &owner, &preferred_addr)?;
```

---

### `auto_resolve_beneficiary_conflict(vault_id: u64) -> Result<(), ContractError>`

**Caller**: Anyone  
**Auth**: None required

Deterministically resolves the conflict after the dispute window has closed. Applies resolution rules (priority → first-registered). Callable by any party, enabling permissionless settlement.

**Errors**:
| Error | Cause |
|-------|-------|
| `ConflictNotFound` | No conflict record exists |
| `ConflictNoClaimsFound` | Conflict record exists but no claims filed |
| `ConflictAlreadyResolved` | Conflict is already settled |
| `ConflictDisputeWindowActive` | Dispute window has not yet expired |

**Events**: `CONFLICT_AUTO_RESOLVED_TOPIC` (`conf_aut`) with `(vault_id, winner_address)`

**Example**:
```rust
// After dispute window expires, anyone can trigger resolution
client.auto_resolve_beneficiary_conflict(&vault_id)?;
```

---

### `resolve_beneficiary_conflict(vault_id: u64, approved_beneficiary: Address) -> Result<(), ContractError>`

**Caller**: Admin only  
**Auth**: Required

Administrator manually approves a specific beneficiary. Can be called **during** the dispute window (before it closes) for escalation scenarios. Supersedes auto-resolution.

**Errors**: `ConflictNotFound`, `ConflictAlreadyResolved`, `NotAdmin`

**Events**: `BENEFICIARY_CONFLICT_RESOLVED_TOPIC` (`ben_res`)

---

### `get_beneficiary_conflict(vault_id: u64) -> Option<BeneficiaryConflict>`

**Caller**: Anyone  
**Auth**: Not required

Returns the full conflict record or `None`.

## Data Structures

```rust
pub struct BeneficiaryConflict {
    pub vault_id: u64,
    pub claims: Vec<BeneficiaryConflictClaim>,
    pub resolution: ConflictResolution,
    pub resolved_at: Option<u64>,
    /// Unix timestamp after which auto_resolve may be called.
    pub dispute_window_ends_at: Option<u64>,
    /// Owner-designated priority claimant (overrides filing order if they have a claim).
    pub priority_beneficiary: Option<Address>,
}

pub struct BeneficiaryConflictClaim {
    pub claimant: Address,
    pub reason: String,
    pub filed_at: u64,
}

pub enum ConflictResolution {
    Pending,
    Approved(Address),
    Rejected,
}
```

## Constants

| Constant | Value | Description |
|---|---|---|
| `DEFAULT_CONFLICT_DISPUTE_WINDOW` | 72 hours | Default window if owner never called `set_conflict_dispute_window` |
| `MIN_CONFLICT_DISPUTE_WINDOW` | 1 hour | Minimum allowed window |
| `MAX_CONFLICT_DISPUTE_WINDOW` | 30 days | Maximum allowed window |

## Error Codes

| Code | Value | Meaning |
|------|-------|---------|
| `ConflictAlreadyResolved` | 144 | Conflict has a non-Pending resolution; no further changes allowed |
| `ConflictDisputeWindowActive` | 145 | Dispute window has not yet closed; auto-resolve blocked |
| `ConflictNotFound` | 146 | No conflict record exists for this vault |
| `ConflictNoClaimsFound` | 147 | Conflict record exists but no claims have been filed |

## Behavior

### Lifecycle

```
[No conflict]
     │
     │  claim_beneficiary_conflict / file_beneficiary_conflict
     ▼
[Pending — dispute window running]
     │                    │
     │ window expires      │ admin override
     │                    │
     ▼                    ▼
auto_resolve_...    resolve_beneficiary_conflict
     │                    │
     └────────────────────┘
              │
              ▼
     [Approved(winner)]   (immutable)
```

### Dispute Window

- Started when the **first** claim is filed, not when `set_conflict_dispute_window` is called.
- Default is 72 hours; vault owner can customise before any claims are filed.
- Once the first claim is filed, the deadline is fixed in `dispute_window_ends_at`.
- `auto_resolve_beneficiary_conflict` is blocked until `now >= dispute_window_ends_at`.
- Admin manual override (`resolve_beneficiary_conflict`) is **not** blocked by the window.

### First-Registered Rule

The claimant with the **lowest `filed_at` timestamp** wins when no priority is set (or when the priority claimant never filed). Claims are compared by timestamp regardless of storage order.

### Owner-Designated Priority

The owner can call `set_conflict_priority_beneficiary` at any point while the conflict is Pending. The designated address wins during auto-resolution only if it has a matching claim entry. If the priority address never filed, the system falls back to first-registered.

### Immutability After Resolution

Once resolved (`ConflictResolution::Approved(...)`), no new claims can be added and the resolution cannot be changed. Attempting to file a claim returns `ConflictAlreadyResolved`.

## Events

| Topic | Symbol | Data | Trigger |
|-------|--------|------|---------|
| `BENEFICIARY_CONFLICT_FILED_TOPIC` | `ben_conf` | `(vault_id, beneficiary)` | `file_beneficiary_conflict` |
| `CONFLICT_CLAIMED_TOPIC` | `conf_clm` | `(vault_id, claimant)` | `claim_beneficiary_conflict` |
| `CONFLICT_DISPUTE_WINDOW_SET_TOPIC` | `conf_dw` | `(vault_id, duration_seconds)` | `set_conflict_dispute_window` |
| `CONFLICT_PRIORITY_SET_TOPIC` | `conf_pri` | `(vault_id, priority_address)` | `set_conflict_priority_beneficiary` |
| `CONFLICT_AUTO_RESOLVED_TOPIC` | `conf_aut` | `(vault_id, winner)` | `auto_resolve_beneficiary_conflict` |
| `BENEFICIARY_CONFLICT_RESOLVED_TOPIC` | `ben_res` | `(vault_id, approved_address)` | `resolve_beneficiary_conflict` (admin) |
| `CONFLICT_EXPIRED_TOPIC` | `conf_exp` | `vault_id` | Admin resolve attempted on 30-day-old conflict |

## Examples

### Example 1: Standard automated flow

```rust
// Two parties file competing claims
client.claim_beneficiary_conflict(&vault_id, &alice, &String::from_str(&env, "Primary heir"))?;
client.claim_beneficiary_conflict(&vault_id, &bob, &String::from_str(&env, "Alternate heir"))?;

// --- 72+ hours later ---

// Anyone can trigger deterministic resolution
client.auto_resolve_beneficiary_conflict(&vault_id)?;

// Alice wins (first-registered)
if let Some(conflict) = client.get_beneficiary_conflict(&vault_id) {
    assert_eq!(conflict.resolution, ConflictResolution::Approved(alice));
}
```

### Example 2: Owner sets priority

```rust
client.claim_beneficiary_conflict(&vault_id, &alice, &reason)?;
client.claim_beneficiary_conflict(&vault_id, &bob, &reason)?;

// Owner designates Bob as priority
client.set_conflict_priority_beneficiary(&vault_id, &owner, &bob)?;

// After window expires, Bob wins
client.auto_resolve_beneficiary_conflict(&vault_id)?;
```

### Example 3: Custom dispute window

```rust
// Owner sets a 7-day window before any claims
client.set_conflict_dispute_window(&vault_id, &owner, &(7 * 24 * 3600))?;

// Claims filed — window deadline is now fixed at filed_at + 7 days
client.claim_beneficiary_conflict(&vault_id, &alice, &reason)?;
```

### Example 4: Admin override during window

```rust
// Conflict filed, window still active
client.claim_beneficiary_conflict(&vault_id, &alice, &reason)?;

// Admin overrides immediately without waiting for window
client.resolve_beneficiary_conflict(&vault_id, &alice)?;
```

## Security Considerations

1. **Open claim filing**: `claim_beneficiary_conflict` allows any address to file — this is intentional to prevent the current beneficiary from blocking legitimate competing claims.
2. **Dispute window**: Prevents race-condition resolution; competing parties have guaranteed time to respond.
3. **Permissionless auto-resolution**: Anyone can call `auto_resolve_beneficiary_conflict` after the window closes. The outcome is deterministic and independent of who calls it.
4. **Immutable resolution**: Once approved, the conflict record is final and cannot be reopened.
5. **Owner priority requires a claim**: The owner cannot silently assign a winner who never filed; the priority address must have submitted a claim.
6. **Admin 30-day cap**: The admin manual override rejects conflicts older than 30 days to force auto-resolution for stale records.

## Testing

Covered in `contracts/ttl_vault/src/beneficiary_conflict_resolution_tests.rs`:

- `test_get_conflict_returns_none_when_no_conflict`
- `test_file_beneficiary_conflict_records_claim`
- `test_file_beneficiary_conflict_empty_reason_fails`
- `test_file_beneficiary_conflict_dispute_window_uses_default`
- `test_claim_beneficiary_conflict_any_address`
- `test_claim_beneficiary_conflict_multiple_claimants`
- `test_claim_beneficiary_conflict_empty_reason_fails`
- `test_set_conflict_dispute_window_owner_only`
- `test_set_conflict_dispute_window_non_owner_fails`
- `test_set_conflict_dispute_window_below_min_fails`
- `test_set_conflict_dispute_window_above_max_fails`
- `test_set_conflict_dispute_window_applied_on_first_claim`
- `test_set_priority_beneficiary_owner_only`
- `test_set_priority_beneficiary_non_owner_fails`
- `test_set_priority_beneficiary_after_resolution_fails`
- `test_auto_resolve_first_registered_wins`
- `test_auto_resolve_owner_priority_wins`
- `test_auto_resolve_priority_fallback_to_first_registered`
- `test_auto_resolve_dispute_window_active_fails`
- `test_auto_resolve_no_conflict_record_fails`
- `test_auto_resolve_no_claims_fails`
- `test_auto_resolve_already_resolved_fails`
- `test_resolve_beneficiary_conflict_admin_only`
- `test_resolve_beneficiary_conflict_no_conflict_fails`
- `test_resolve_beneficiary_conflict_already_resolved_fails`
- `test_no_new_claims_after_resolution`
- `test_file_beneficiary_conflict_rejected_after_auto_resolve`
- `test_resolved_at_is_set_on_auto_resolve`
- `test_resolved_at_is_set_on_manual_resolve`
