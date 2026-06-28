# Multi-Sig Vault Configuration

Vault owners can require multiple approvals before sensitive operations execute. This protects against single-key compromise and is suitable for shared custody or high-value vaults.

## How It Works

Multi-sig uses a **propose → approve → execute** flow with optional veto:

```
Owner                    Co-Signers               Veto Path
  │
  ├─ configure_multisig(signers, threshold)
  │
  ├─ propose_multisig(operation, payload)  ← owner auto-approves
  │
  │                    ├─ approve_multisig(proposal_id)
  │                    └─ approve_multisig(proposal_id)  ← threshold reached
  │                                  │
  │                                  ├─ veto_multisig_proposal(proposal_id)
  │                                  │  └─ Proposal cancelled, operation blocked
  │
  └─ execute_multisig(proposal_id)         ← operation runs (if no veto)
```

## Setup

```rust
// 2-of-3 multi-sig (owner + 2 co-signers, threshold = 2)
configure_multisig(vault_id, owner, [signer1, signer2], 2)
```

- `signers` — co-signer addresses (must not include the owner)
- `threshold` — total approvals needed (1 ≤ threshold ≤ signers.len() + 1)
- The owner always counts as one approver

## Operations Requiring Multi-Sig

Once configured, these operations require a proposal:

| Operation | Payload |
|---|---|
| `Withdraw` | `encode_i128_payload(amount)` |
| `UpdateBeneficiary` | `address_payload = Some(new_beneficiary)` |
| `CancelVault` | empty `Bytes` |
| `TransferOwnership` | `address_payload = Some(new_owner)` |
| `UpdateCheckInInterval` | `encode_u64_payload(new_interval)` |

## API Reference

### Configure
```rust
configure_multisig(vault_id, caller, signers, threshold) -> Result<(), ContractError>
remove_multisig(vault_id, caller) -> Result<(), ContractError>
get_multisig_config(vault_id) -> Option<MultiSigConfig>
has_multisig(vault_id) -> bool
```

### Propose
```rust
propose_multisig(vault_id, caller, operation, payload, address_payload) -> Result<u64, ContractError>
// Returns proposal_id. Owner is auto-approved.
```

### Approve / Reject
```rust
approve_multisig(vault_id, proposal_id, caller) -> Result<(), ContractError>
reject_multisig(vault_id, proposal_id, caller) -> Result<(), ContractError>
```

### Execute
```rust
execute_multisig(vault_id, proposal_id, caller) -> Result<(), ContractError>
// Proposal must be in Approved status.
```

### Query
```rust
get_multisig_proposal(vault_id, proposal_id) -> Option<MultiSigProposal>
get_multisig_proposal_count(vault_id) -> u64
```

### Payload Helpers
```rust
encode_i128_payload(value: i128) -> Bytes   // for Withdraw
encode_u64_payload(value: u64) -> Bytes     // for UpdateCheckInInterval
// For address operations, pass address_payload = Some(address)
```

## Proposal Lifecycle

| Status | Meaning |
|---|---|
| `Pending` | Created, collecting approvals |
| `Approved` | Threshold reached, ready to execute |
| `Executed` | Operation completed |
| `Rejected` | Owner rejected the proposal |
| `Expired` | Not executed within 7 days |

Proposals expire **7 days** after creation. Expired proposals cannot be approved or executed.

## Veto Workflow

### Overview

Any co-signer can **veto** a proposal after it reaches the approval threshold. Veto blocks execution even if all approvals are gathered.

```
Proposal reaches threshold (Approved)
                │
                ├─ Co-signer vetoes
                │  └─ Status → Vetoed (immutable, cannot execute)
                │
                └─ No veto within 24 hours
                   └─ Can be executed
```

### Veto Use Cases

- **Emergency block**: Halt an operation if a co-signer detects an error
- **Dispute resolution**: Prevent execution pending investigation
- **Last-minute changes**: Block execution if vault state has changed unexpectedly

### API

```rust
/// Veto a proposal that has reached the approval threshold.
/// Only co-signers can veto (not the owner).
/// Proposal must be in Approved status (not yet executed or rejected).
veto_multisig_proposal(vault_id, proposal_id, caller) -> Result<(), ContractError>

/// Get veto information for a proposal
get_multisig_proposal_veto(vault_id, proposal_id) -> Option<VetoRecord>
// Returns: { vetoed_by: Address, vetoed_at: u64 }
```

### Veto Constraints

- **Only co-signers can veto**: The owner cannot veto their own proposals
- **Time window**: 24 hours after proposal creation (or before execution, whichever is sooner)
- **Cannot veto twice**: Only one veto per proposal
- **Vetoed status is final**: Cannot reverse a veto; must create a new proposal

### Events

| Event | Topic | Data |
|---|---|---|
| Vetoed | `ms_veto` | `(proposal_id, veto_issuer, vetoed_at)` |

---

## Signer Removal

### Overview

Vault owners can remove co-signers from the multi-sig configuration at any time. This is useful for:

- Revoking access if a signer loses their key
- Rotating signers for security
- Updating the approval threshold

### API

```rust
/// Remove a co-signer from the multi-sig configuration.
/// Caller must be the vault owner.
/// Automatically adjusts threshold if needed (cannot exceed remaining signers + 1).
remove_multisig_signer(vault_id, caller, signer_to_remove) -> Result<(), ContractError>

/// Get the current list of co-signers
get_multisig_signers(vault_id) -> Vec<Address>

/// Check if a signer is currently configured
is_multisig_signer(vault_id, address) -> bool
```

### Removal Behavior

1. **Signer removed from active config** immediately
2. **Pending proposals** with the removed signer:
   - If already approved by threshold: remains executable (no change)
   - If not yet approved: removed signer's approval counts toward threshold (current approvals remain)
3. **Threshold auto-adjustment**: If new signer count drops below threshold:
   - Threshold is reduced to match new signer count + 1 (maximum)
   - Example: If removing a signer from 3-of-5, new threshold becomes at most 4-of-4

### Removal Constraints

- **Owner cannot remove themselves** (owner is always a signer)
- **Cannot remove all co-signers**: At least one co-signer must remain (minimum 2 total)
- **Irreversible**: Removed signer must be re-added via `configure_multisig` or `add_multisig_signer`

### Proposal Cancellation on Signer Removal

If a signer is removed while holding a veto on a proposal:

- **Veto is lifted** (cannot execute a veto from removed signer)
- **Proposal may now be executable** if threshold is met

Example:

```
3 signers: [A, B, C], threshold = 2
Proposal has: [A (approved), B (approved), C (vetoed)]
Proposal status: Vetoed

Action: Remove C

Result:
  Signers: [A, B], threshold = auto-adjusted to 2
  Proposal approvals: [A, B] ✓ threshold met
  Veto: Lifted (C is no longer a signer)
  Status: Approved → executable!
```

---

## Events

| Event | Topic | Data |
|---|---|---|
| Configured | `ms_cfg` | `threshold` |
| Proposed | `ms_prop` | `(proposal_id, operation, expires_at)` |
| Approved | `ms_appr` | `(proposal_id, approver, approval_count)` |
| Executed | `ms_exec` | `proposal_id` |
| Rejected | `ms_rej` | `proposal_id` |
| Vetoed | `ms_veto` | `(proposal_id, veto_issuer, vetoed_at)` |
| SignerRemoved | `ms_signer_rm` | `(removed_signer, new_threshold)` |

## Error Codes

| Code | Constant | Meaning |
|---|---|---|
| `#34` | `MultiSigRequired` | Vault has no multi-sig config |
| `#35` | `AlreadyApproved` | Caller already approved this proposal |
| `#36` | `ProposalNotFound` | Proposal does not exist or is not Pending |
| `#37` | `ProposalExpired` | Proposal passed its 7-day expiry |
| `#38` | `ProposalNotApproved` | Proposal has not reached threshold yet |
| `#39` | `NotASigner` | Caller is not the owner or a configured co-signer |
| `#40` | `InvalidThreshold` | Threshold is 0 or exceeds total signers |
| `#41` | `VetoWindowClosed` | Cannot veto; 24 hours have passed or proposal already executed |
| `#42` | `AlreadyVetoed` | Proposal already has a veto from this or another signer |
| `#43` | `OwnerCannotVeto` | Owner cannot veto their own proposal |
| `#44` | `ProposalVetoed` | Cannot execute; proposal is vetoed |
| `#45` | `CannotRemoveOwner` | Cannot remove the owner from signers |
| `#46` | `CannotRemoveLastSigner` | Must keep at least one co-signer |
| `#47` | `SignerNotFound` | Signer not in the current configuration |

## Example: 2-of-3 Withdraw

```rust
// 1. Configure
configure_multisig(vault_id, owner, [alice, bob], 2);

// 2. Owner proposes a 500-stroop withdrawal
let payload = encode_i128_payload(500);
let pid = propose_multisig(vault_id, owner, Withdraw, payload, None);
// → owner auto-approved (1/2)

// 3. Alice approves → threshold reached (2/2)
approve_multisig(vault_id, pid, alice);

// 4. Owner executes
execute_multisig(vault_id, pid, owner);
// → 500 stroops transferred to owner
```
