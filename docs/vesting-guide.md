# Milestone Vesting Guide

Milestone vesting unlocks vault funds only when specific real-world events are confirmed by an oracle, rather than on a fixed time schedule. This is well-suited for employment vesting tied to performance targets, grant disbursement contingent on deliverables, or estate distributions that require proof of a triggering event.

## How It Works

```
Owner                      Oracle                     Beneficiary
  │
  ├─ create_vault(...)
  ├─ deposit(vault_id, amount)
  ├─ set_milestone_vesting_schedule(vault_id, milestones)
  │      └─ Stores milestones: each has an amount, deadline, oracle address
  │
  │  (vault TTL elapses or manual release)
  ├─ trigger_release(vault_id)
  │      └─ Vault transitions to Released; balance stays in contract
  │
  │                       ├─ attest_milestone(vault_id, milestone_id, timestamp)
  │                       │      └─ Oracle signs off; milestone marked Attested
  │
  │                                          ├─ claim_milestone_vesting(vault_id, milestone_id)
  │                                          │      └─ Funds for that milestone transferred
```

Each milestone is independent. The beneficiary can claim attested milestones in any order and does not need to wait for all milestones to be attested before claiming the ones that are ready.

---

## Configuring Milestones

### `set_milestone_vesting_schedule`

```rust
fn set_milestone_vesting_schedule(
    env: Env,
    vault_id: u64,
    caller: Address,                // must be vault owner
    schedule: MilestoneVestingSchedule,
) -> Result<(), ContractError>
```

**`MilestoneVestingSchedule` fields:**

| Field | Type | Description |
|---|---|---|
| `milestones` | `Vec<MilestoneEntry>` | Ordered list of milestone definitions |
| `oracle` | `Address` | Default oracle contract address (can be overridden per-milestone) |
| `allow_partial` | `bool` | If `true`, each milestone can be claimed independently. If `false`, all milestones must be attested before any can be claimed. |

**`MilestoneEntry` fields:**

| Field | Type | Description |
|---|---|---|
| `milestone_id` | `u64` | Unique identifier within this vault's schedule |
| `description` | `String` | Human-readable label (stored on-chain, max 256 bytes) |
| `amount` | `i128` | Stroops unlocked when this milestone is attested |
| `deadline` | `u64` | Unix timestamp after which the milestone cannot be attested |
| `oracle` | `Option<Address>` | Per-milestone oracle override; if `None`, the schedule-level oracle is used |

**Constraints:**
- Caller must be the vault owner.
- Vault must be `Locked` (not yet released).
- The sum of all milestone `amount` values must not exceed the vault balance at the time `set_milestone_vesting_schedule` is called. Any surplus remains claimable as a lump sum on `trigger_release`.
- `milestone_id` values must be unique within the schedule.
- Calling `set_milestone_vesting_schedule` again replaces the entire schedule and resets all attestation state.

**Example:**

```rust
let schedule = MilestoneVestingSchedule {
    milestones: vec![
        &env,
        MilestoneEntry {
            milestone_id: 1,
            description: String::from_str(&env, "Phase 1: prototype delivered"),
            amount: 250_000_000,  // 25 XLM in stroops
            deadline: now + 90 * 86_400,   // 90 days from now
            oracle: None,         // use schedule-level oracle
        },
        MilestoneEntry {
            milestone_id: 2,
            description: String::from_str(&env, "Phase 2: audit passed"),
            amount: 750_000_000,  // 75 XLM
            deadline: now + 180 * 86_400,
            oracle: None,
        },
    ],
    oracle: oracle_address.clone(),
    allow_partial: true,
};

client.set_milestone_vesting_schedule(&vault_id, &owner, &schedule)?;
```

---

## Oracle Signature Verification

Each oracle is an on-chain contract. When `attest_milestone` is called, the vault contract queries the oracle's `query` entry point with the vault ID and milestone ID. The oracle returns `true` to confirm the milestone event occurred and `false` (or errors, which are treated as `false`) otherwise.

### Oracle Contract Interface

Your oracle contract must implement:

```rust
pub fn query(env: Env, vault_id: u64) -> bool
```

The oracle is called with the vault ID. The oracle is responsible for looking up milestone-specific state internally (e.g., keyed by `(vault_id, milestone_id)`) or for attesting vault-level events.

### `attest_milestone`

```rust
fn attest_milestone(
    env: Env,
    vault_id: u64,
    milestone_id: u64,
    oracle_timestamp: u64,   // claimed real-world timestamp of the event
) -> Result<(), ContractError>
```

The caller of `attest_milestone` must be the oracle address configured for that milestone. The contract verifies:

1. The milestone exists and is `Pending`.
2. `oracle_timestamp` is not after the milestone `deadline`.
3. The caller matches the milestone's oracle address.

On success the milestone status is set to `Attested` and a `ms_vest` event is emitted.

**Example (called from the oracle contract):**

```rust
// From within the oracle contract, after confirming the real-world event:
vault_client.attest_milestone(&vault_id, &milestone_id, &env.ledger().timestamp());
```

### Designing a Reliable Oracle

- **Off-chain trigger, on-chain confirmation:** The oracle contract typically receives a signed message from a trusted off-chain system. Verify the signature on-chain before returning `true`.
- **One-time attestation:** Store a flag after attesting to prevent re-attestation on a re-deployed oracle.
- **Deadline enforcement:** The vault enforces the deadline, but your oracle should also check it to avoid wasted transactions.
- **Audit log:** Emit an event from the oracle contract whenever it attests, so there is an independent on-chain record separate from the vault contract.

---

## Claiming an Attested Milestone

```rust
fn claim_milestone_vesting(
    env: Env,
    vault_id: u64,
    milestone_id: u64,
) -> Result<i128, ContractError>
```

- Vault must be `Released`.
- The milestone must be `Attested`.
- If `allow_partial` is `false`, all milestones must be `Attested` before any can be claimed.
- Returns the amount transferred to the beneficiary.

**Example:**

```rust
// After trigger_release and after the oracle attests milestone 1:
let amount = client.claim_milestone_vesting(&vault_id, &1u64)?;
// amount = 250_000_000 stroops (25 XLM)
```

---

## Pausing and Resuming Milestone Vesting

The owner can pause the milestone vesting schedule (e.g., if a dispute arises). While paused, `attest_milestone` and `claim_milestone_vesting` both revert.

```rust
fn pause_milestone_vesting(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError>
fn resume_milestone_vesting(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError>
```

Events emitted: `ms_paus` / `ms_resm`.

---

## Handling a Failed Oracle

An oracle failure means the oracle contract is unreachable, returns `false`, or the oracle address becomes invalid. This can prevent legitimate milestones from being attested.

### Option 1 — Adjust the oracle address

If the oracle contract has been redeployed at a new address, the owner can update the schedule:

```rust
// Provide a new schedule with the corrected oracle address
// (this resets all attestation state — only do this before any milestones are attested)
client.set_milestone_vesting_schedule(&vault_id, &owner, &updated_schedule)?;
```

If milestones have already been attested and partially claimed, contact the maintainers to coordinate an emergency override (see [Emergency Procedures](#emergency-procedures) below).

### Option 2 — Emergency milestone override

The vault contract exposes an emergency attestation path callable only by the vault admin:

```rust
fn emergency_milestone_attest(
    env: Env,
    vault_id: u64,
    milestone_id: u64,
    caller: Address,     // must be contract admin
) -> Result<(), ContractError>
```

This emits `ms_emer` and is auditable on-chain. It should be used only when the oracle is demonstrably unavailable and the event being attested has been confirmed by another means (e.g., court order, signed affidavit stored off-chain).

### Option 3 — Let the milestone expire

If a milestone deadline passes without attestation, its `amount` remains in the vault indefinitely. The owner can call `adjust_milestone` to extend the deadline:

```rust
fn adjust_milestone(
    env: Env,
    vault_id: u64,
    milestone_id: u64,
    caller: Address,      // must be vault owner
    new_deadline: u64,
    new_oracle: Option<Address>,
) -> Result<(), ContractError>
```

Events emitted: `ms_adj`.

---

## Appealing an Incorrect Milestone Decision

If the oracle attests a milestone incorrectly — or refuses to attest a legitimate event — either party can initiate a dispute.

### Step 1 — File a dispute

```rust
fn file_withdrawal_dispute(
    env: Env,
    vault_id: u64,
    caller: Address,     // beneficiary or vault owner
    reason: String,
) -> Result<(), ContractError>
```

This emits `wd_disp` and freezes the disputed milestone's funds until the dispute is resolved.

### Step 2 — Arbitration

If an arbitrator address has been configured on the vault (via `set_arbitrator`), the arbitrator can rule in favour of either party:

```rust
fn arbitration_ruling(
    env: Env,
    vault_id: u64,
    caller: Address,     // must be the configured arbitrator
    in_favour_of: Address,
    reason: String,
) -> Result<(), ContractError>
```

The ruling transfers the disputed funds to the winning party and emits `arb_rul`.

### Step 3 — Escalation to admin

If no arbitrator is configured, or if the ruling itself is disputed, contact the repository maintainers via the security disclosure process in [`SECURITY.md`](../SECURITY.md). Include:

- Vault ID and milestone ID
- On-chain transaction hashes for the oracle attestation and any claim attempts
- Off-chain evidence supporting the correct outcome (signed messages, screenshots, contracts)

The admin can call `emergency_milestone_attest` or `resolve_withdrawal_dispute` to correct the on-chain state.

---

## Emergency Procedures

| Situation | Action |
|---|---|
| Oracle contract unreachable | Extend deadline with `adjust_milestone`, then deploy a new oracle |
| Oracle attested an event that did not occur | File a dispute; escalate to admin if no arbitrator configured |
| Oracle refused to attest a confirmed event | File a dispute; admin can call `emergency_milestone_attest` |
| All milestones expired without attestation | Funds remain in vault; owner can extend deadlines or reclaim via `cancel_vault` |

---

## Events

| Topic | Data | Emitted when |
|---|---|---|
| `ms_vest` | `(milestone_id, oracle_timestamp)` | Milestone attested by oracle |
| `ms_paus` | `vault_id` | Milestone vesting paused |
| `ms_resm` | `vault_id` | Milestone vesting resumed |
| `ms_adj`  | `(milestone_id, new_deadline)` | Milestone deadline or oracle adjusted |
| `ms_emer` | `(milestone_id, admin)` | Emergency admin attestation |
| `ms_prog` | `(milestone_id, progress)` | Progress update (optional, oracle-driven) |
| `ms_clam` | `(milestone_id, beneficiary, amount)` | Milestone funds claimed |

---

## Error Reference

| Constant | Meaning |
|---|---|
| `VestingNotFound` | No milestone schedule attached to this vault |
| `NothingToClaimYet` | Milestone exists but is not yet `Attested` |
| `VestingAlreadyComplete` | Milestone already claimed |
| `CliffNotReached` | `allow_partial = false` and not all milestones are attested |
| `NotOwner` | Caller is not the vault owner |
| `AlreadyReleased` | Vault is not `Locked`; cannot modify schedule |

---

## Full Example: 3-Milestone Employment Vesting

```rust
let now = env.ledger().timestamp() as u64;
let month = 30 * 86_400u64;

// 1. Create vault and deposit 1000 XLM
let vault_id = client.create_vault(&owner, &employee, &(365 * 86_400u64), &None);
client.deposit(&vault_id, &owner, &1_000_000_000i128)?;

// 2. Configure three performance milestones
let schedule = MilestoneVestingSchedule {
    milestones: vec![
        &env,
        MilestoneEntry {
            milestone_id: 1,
            description: String::from_str(&env, "Q1 targets met"),
            amount: 250_000_000,
            deadline: now + 3 * month,
            oracle: None,
        },
        MilestoneEntry {
            milestone_id: 2,
            description: String::from_str(&env, "Q2 targets met"),
            amount: 250_000_000,
            deadline: now + 6 * month,
            oracle: None,
        },
        MilestoneEntry {
            milestone_id: 3,
            description: String::from_str(&env, "Annual review passed"),
            amount: 500_000_000,
            deadline: now + 12 * month,
            oracle: None,
        },
    ],
    oracle: hr_oracle_address.clone(),
    allow_partial: true,
};
client.set_milestone_vesting_schedule(&vault_id, &owner, &schedule)?;

// 3. Vault releases (TTL expires after one year or owner calls trigger_release_manual)
client.trigger_release(&vault_id);

// 4. HR oracle attests Q1
//    (called from the hr_oracle contract after off-chain confirmation)
vault_client.attest_milestone(&vault_id, &1, &(now + 3 * month));

// 5. Employee claims Q1 funds
let q1_amount = client.claim_milestone_vesting(&vault_id, &1)?;
// q1_amount = 250_000_000

// 6. Repeat for Q2 and annual review as they are attested
```
