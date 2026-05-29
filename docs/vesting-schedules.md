# Vesting Schedules

Vesting schedules allow vault funds to be released to beneficiaries in equal installments over time, rather than as a lump sum on `trigger_release`.

## How It Works

1. **Owner attaches a schedule** (while vault is `Locked`) via `set_vesting_schedule`.
2. **Vault expires** and anyone calls `trigger_release`. Because a schedule is attached, the vault transitions to `Released` but the balance stays in the contract.
3. **Beneficiary claims** available installments via `claim_vested_installment`. Each call transfers all unlocked-but-unclaimed tranches.

## Example: 25% per year for 4 years

```rust
// 1. Create vault and deposit 1,000 XLM
let vault_id = client.create_vault(&owner, &beneficiary, &check_in_interval);
client.deposit(&vault_id, &owner, &1_000_000_000); // 100 XLM in stroops

// 2. Attach vesting schedule with a 1-year cliff
//    start_time:       Unix timestamp of first claimable installment
//    interval:         seconds between installments (1 year ≈ 31_536_000 s)
//    num_installments: 4
//    cliff_period:     seconds after start_time before any claim is allowed (1 year)
client.set_vesting_schedule(
    &vault_id,
    &owner,
    &start_time,          // e.g. env.ledger().timestamp() + 31_536_000
    &31_536_000u64,       // 1 year in seconds
    &4u32,
    &31_536_000u64,       // 1-year cliff
);

// 3. After vault expires, anyone triggers release (no funds move yet)
client.trigger_release(&vault_id);

// 4. Beneficiary claims each year (first claim only possible after cliff)
client.claim_vested_installment(&vault_id); // year 1: 250 XLM
// ... one year later ...
client.claim_vested_installment(&vault_id); // year 2: 250 XLM
```

## API Reference

### `set_vesting_schedule`

```rust
fn set_vesting_schedule(
    env: Env,
    vault_id: u64,
    caller: Address,      // must be vault owner
    start_time: u64,      // Unix timestamp of first claimable installment
    interval: u64,        // seconds between installments (must be > 0)
    num_installments: u32 // total number of tranches (must be > 0)
    cliff_period: u64,    // seconds after start_time before any claim is allowed (0 = no cliff)
) -> Result<(), ContractError>
```

Constraints:
- Caller must be the vault owner.
- Vault must be `Locked` (not yet released or cancelled).
- `interval` and `num_installments` must both be > 0.
- Vault balance must be > 0.
- Replaces any previously set schedule (claimed_installments resets to 0).
- `cliff_period` may be 0 (disables cliff enforcement).

### `get_vesting_schedule`

```rust
fn get_vesting_schedule(env: Env, vault_id: u64) -> Option<VestingSchedule>
```

Returns the attached schedule, or `None` if none exists.

### `claim_vested_installment`

```rust
fn claim_vested_installment(env: Env, vault_id: u64) -> Result<i128, ContractError>
```

Claims all installments that have become available since the last claim. Returns the total amount transferred.

Constraints:
- Vault must be `Released`.
- A vesting schedule must be attached.
- At least one new installment window must have elapsed since `start_time`.
- All installments must not already be claimed.

Errors:
| Code | Name | Meaning |
|------|------|---------|
| 22 | `VestingNotFound` | No schedule attached to this vault |
| 23 | `NothingToClaimYet` | No new installments available (before `start_time` or between windows) |
| 24 | `VestingAlreadyComplete` | All installments have been claimed |
| 55 | `CliffNotReached` | Current time is before `start_time + cliff_period` |

## Cliff Periods

A cliff period prevents any installment from being claimed until a minimum duration has elapsed since `start_time`. This is useful for enforcing a lock-up before vesting begins.

- Set `cliff_period > 0` in `set_vesting_schedule` to enable.
- Set `cliff_period = 0` to disable (default behaviour, no lock-up).
- Attempting to claim before `start_time + cliff_period` returns `CliffNotReached` (error 55).
- A `clif_rch` event is emitted on the **first successful claim** after the cliff (only once per schedule).

### Example: 1-year cliff, then quarterly vesting

```rust
// Cliff of 1 year, then 4 quarterly installments
client.set_vesting_schedule(
    &vault_id, &owner,
    &start_time,
    &7_884_000u64,   // ~91 days per installment
    &4u32,
    &31_536_000u64,  // 1-year cliff
);
```

## Installment Calculation

Each installment = `total_amount / num_installments` (integer division).  
The **last installment** absorbs any remainder to ensure the full balance is distributed.

Example with 1,000 stroops over 3 installments:
- Installment 1: 333
- Installment 2: 333
- Installment 3: 334 (absorbs remainder)

## Multi-Beneficiary Vesting

Vesting is compatible with `set_beneficiaries`. Each claim distributes the installment amount proportionally by BPS, with the last beneficiary absorbing dust.

```rust
// Set 60/40 split
client.set_beneficiaries(&vault_id, &owner, &entries);

// Attach vesting schedule
client.set_vesting_schedule(&vault_id, &owner, &start, &interval, &4u32);

// Each claim splits the installment: 60% to ben_a, 40% to ben_b
client.claim_vested_installment(&vault_id);
```

## Vesting Reversal (Issue #548)

The vault owner may configure a reversal grace period on a per-claim basis. Instead of calling `claim_vested_installment` (which transfers immediately), use the 2-phase flow:

1. **`initiate_vesting_claim(vault_id, reversal_window_seconds)`** — calculates the claimable amount and holds it in escrow inside the contract. The schedule counter is advanced to block double-initiation. Returns the escrowed amount.

2. **`reverse_vesting_claim(vault_id, caller)`** (owner only) — cancels the pending claim within the reversal window. The schedule counter is rolled back so the installments become claimable again.

3. **`finalize_vesting_claim(vault_id)`** (anyone) — after the reversal window closes, completes the token transfer to the beneficiary.

### API Reference

```rust
fn initiate_vesting_claim(env: Env, vault_id: u64, reversal_window_seconds: u64) -> Result<i128, ContractError>
fn reverse_vesting_claim(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError>
fn finalize_vesting_claim(env: Env, vault_id: u64) -> Result<i128, ContractError>
fn get_pending_vesting_claim(env: Env, vault_id: u64) -> Option<VestingPendingClaim>
```

| Error | Code | Meaning |
|-------|------|---------|
| `VestingReversalNotFound` | 61 | No pending claim on this vault |
| `VestingReversalExpired`  | 60 | Reversal window has already closed |
| `InvalidAmount`           |  5 | A pending claim already exists (call finalize first) |

## Vesting Rollover (Issue #541)

When enabled, any installments that were available but not claimed in a previous period roll over and accumulate. This ensures that a beneficiary doesn't lose out if they miss a claim window.

### `set_vesting_rollover`
```rust
fn set_vesting_rollover(env: Env, vault_id: u64, caller: Address, enabled: bool) -> Result<(), ContractError>
```

## Vesting Forfeiture (Issue #542)

If a beneficiary declines their role, all remaining unvested funds are automatically transferred to a designated forfeiture recipient instead of remaining in the vault or returning to the owner.

### `set_vesting_forfeiture`
```rust
fn set_vesting_forfeiture(env: Env, vault_id: u64, caller: Address, forfeiture_recipient: Address) -> Result<(), ContractError>
```

## Vesting Acceleration on Death (Issue #543)

Allows a designated oracle to immediately unlock all remaining vesting installments. This is typically used to handle the owner's passing.

### `set_vesting_acceleration`
```rust
fn set_vesting_acceleration(env: Env, vault_id: u64, caller: Address, oracle: Address) -> Result<(), ContractError>
```

### `accelerate_vesting`
```rust
fn accelerate_vesting(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError>
```
- `caller` must be the designated oracle.

## Vesting Staggering (Issue #544)

Stagger vesting across multiple beneficiaries with different schedules. Each beneficiary has their own start time, interval, and number of installments.

### `set_vesting_stagger`
```rust
fn set_vesting_stagger(env: Env, vault_id: u64, caller: Address, entries: Vec<VestingStaggerEntry>) -> Result<(), ContractError>
```

### `claim_staggered_vesting`
```rust
fn claim_staggered_vesting(env: Env, vault_id: u64, caller: Address) -> Result<i128, ContractError>
```

## Late-Claim Penalty (Issue #547)
...

The vault owner may attach a penalty config to a vesting schedule. If a beneficiary claims an installment more than `grace_period_seconds` after it unlocked, the payout for that installment is reduced by `penalty_bps` basis points.

### `set_vesting_penalty`

```rust
fn set_vesting_penalty(
    env: Env,
    vault_id: u64,
    caller: Address,           // must be vault owner
    penalty_bps: u32,          // 1–10000 (e.g. 500 = 5%)
    grace_period_seconds: u64, // seconds after unlock before penalty applies
) -> Result<(), ContractError>
```

### `get_vesting_penalty`

```rust
fn get_vesting_penalty(env: Env, vault_id: u64) -> Option<VestingPenaltyConfig>
```

### Penalty Calculation

For each claimable installment:
- If `now > unlock_time + grace_period_seconds`, the installment is "late".
- Late installment payout = `per_installment * (1 - penalty_bps / 10_000)`.
- On-time installments receive the full `per_installment` amount.

Example: 5% penalty, no grace period, 3 late installments of 100 each:
- Full payout would be 300.
- Penalized payout: 300 - 15 = 285.

## Events

| Topic | Data | Emitted when |
|-------|------|--------------|
| `set_vest` | `(start_time, interval, num_installments, total_amount, cliff_period)` | Schedule attached |
| `clm_vest` | `(beneficiary, amount, installments_unlocked)` | Installment claimed (one event per beneficiary) |
| `clif_rch` | `(timestamp,)` | First claim after cliff period is reached (emitted once per schedule) |
