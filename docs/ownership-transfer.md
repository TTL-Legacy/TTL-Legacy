# Vault Ownership Transfer

Vault owners can transfer ownership to another address — useful for account recovery or business transfers. The process uses a **2-step flow with a 24-hour time-lock** to prevent accidental or malicious transfers.

## Flow

```
Owner                          New Owner
  |                                |
  |-- initiate_ownership_transfer -->  (pending request stored, 24h time-lock starts)
  |                                |
  |       [24 hours pass]          |
  |                                |
  |                <-- accept_ownership_transfer --  (ownership transferred)
```

### Step 1 — Initiate (current owner)

```rust
initiate_ownership_transfer(vault_id: u64, caller: Address, new_owner: Address) -> Result<u64, ContractError>
```

- Only the current vault owner can call this.
- Stores a `PendingOwnershipTransfer` with:
  - `new_owner` — the proposed new owner
  - `unlocks_at` — `now + 24 hours` (time-lock; new owner cannot accept before this)
  - `expires_at` — `now + 7 days` (request expires if not accepted)
- Returns `unlocks_at` so the caller knows when the new owner can accept.
- Replaces any existing pending request for the vault.

### Step 2 — Accept (new owner)

```rust
accept_ownership_transfer(vault_id: u64, new_owner: Address) -> Result<(), ContractError>
```

- Only the designated `new_owner` from the pending request can call this.
- Must be called **after** `unlocks_at` and **before** `expires_at`.
- On success: vault `owner` is updated, owner indexes are updated, pending request is cleared.

### Cancel (current owner)

```rust
cancel_ownership_transfer(vault_id: u64, caller: Address) -> Result<(), ContractError>
```

- Only the current vault owner can cancel.
- Removes the pending request; the proposed new owner can no longer accept.

### Query pending request

```rust
get_pending_ownership_transfer(vault_id: u64) -> Option<OwnershipTransferRequest>
```

Returns the pending request if one exists, `None` otherwise.

## Security Properties

| Property | Detail |
|---|---|
| **Time-lock** | 24-hour delay before new owner can accept — prevents rushed/coerced transfers |
| **Expiry** | Request expires after 7 days — prevents stale pending transfers |
| **Approval required** | New owner must explicitly accept — no unilateral transfers |
| **Owner-only initiation** | Only the current owner can start or cancel a transfer |
| **Beneficiary invariant** | `new_owner` cannot equal the vault's beneficiary |
| **Vault must be Locked** | Transfers are blocked on Released/Cancelled vaults |
| **Contract pause respected** | Initiation and acceptance are blocked when contract is paused |

## Events

| Event | Topic constant | Data |
|---|---|---|
| Transfer initiated | `OWNERSHIP_INITIATED_TOPIC` (`own_init`) | `(old_owner, new_owner, unlocks_at)` |
| Transfer accepted | `OWNERSHIP_ACCEPTED_TOPIC` (`own_acc`) | `(old_owner, new_owner)` |
| Transfer cancelled | `OWNERSHIP_CANCELLED_TOPIC` (`own_can`) | `(owner, cancelled_new_owner)` |
| Legacy compat | `OWNERSHIP_TOPIC` (`own_xfer`) | `(old_owner, new_owner)` — emitted on accept for backwards compatibility |

## Error Codes

| Code | Constant | Meaning |
|---|---|---|
| `#34` | `NoPendingOwnershipTransfer` | No pending request exists for this vault |
| `#35` | `OwnershipTransferExpired` | The 7-day acceptance window has passed |
| `#36` | `OwnershipTransferTimeLocked` | The 24-hour time-lock has not yet elapsed |

## Example

```rust
// Step 1: owner initiates
let unlocks_at = client.initiate_ownership_transfer(&vault_id, &owner, &new_owner)?;

// Step 2: wait 24 hours, then new owner accepts
client.accept_ownership_transfer(&vault_id, &new_owner)?;

// Or: owner changes their mind and cancels
client.cancel_ownership_transfer(&vault_id, &owner)?;
```
