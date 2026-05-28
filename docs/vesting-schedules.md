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

// 2. Attach vesting schedule
//    start_time: Unix timestamp of first claimable installment
//    interval:   seconds between installments (1 year ≈ 31_536_000 s)
//    num_installments: 4
client.set_vesting_schedule(
    &vault_id,
    &owner,
    &start_time,          // e.g. env.ledger().timestamp() + 31_536_000
    &31_536_000u64,       // 1 year in seconds
    &4u32,
);

// 3. After vault expires, anyone triggers release (no funds move yet)
client.trigger_release(&vault_id);

// 4. Beneficiary claims each year
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
) -> Result<(), ContractError>
```

Constraints:
- Caller must be the vault owner.
- Vault must be `Locked` (not yet released or cancelled).
- `interval` and `num_installments` must both be > 0.
- Vault balance must be > 0.
- Replaces any previously set schedule (claimed_installments resets to 0).

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

## Late-Claim Penalty (Issue #547)

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
| `set_vest` | `(start_time, interval, num_installments, total_amount)` | Schedule attached |
| `clm_vest` | `(beneficiary, amount, installments_unlocked)` | Installment claimed (one event per beneficiary) |
| `vest_pen` | `(penalty_amount, net_payout)` | Late penalty applied during claim |
| `vest_rev` | `(vault_id, amount)` | Vesting claim reversed by owner within grace period |
| `vest_fin` | `(vault_id, amount)` | Pending vesting claim finalized after reversal window |
