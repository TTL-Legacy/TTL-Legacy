# Beneficiary-Specific Vesting Schedules

## Overview - Issue #525

Each beneficiary can have a unique vesting schedule with different timelines, cliff periods, and installment frequencies. This allows fine-grained control over how funds are released to individual beneficiaries.

## Key Features

- **Per-Beneficiary Schedules**: Set independent vesting timelines for each beneficiary
- **Cliff Periods**: Enforce lock-up durations before any funds become claimable
- **Flexible Intervals**: Different installment frequencies per beneficiary
- **Installment Tracking**: Track claimed vs. available installments per beneficiary
- **Schedule Updates**: Modify schedules (claimed installments reset)

## API Reference

### `set_beneficiary_vesting`

Attach a vesting schedule to a specific beneficiary for a vault.

```rust
pub fn set_beneficiary_vesting(
    env: Env,
    vault_id: u64,
    caller: Address,         // must be vault owner
    beneficiary: Address,    // target beneficiary
    start_time: u64,         // Unix timestamp when vesting begins
    interval: u64,           // seconds between installments
    num_installments: u32,   // total number of tranches
    cliff_period: u64,       // seconds to lock-up (0 = no cliff)
) -> Result<(), ContractError>
```

**Constraints:**
- Caller must be the vault owner
- `interval` and `num_installments` must be > 0
- `start_time` must be a valid future timestamp
- Vault can have multiple schedules (one per beneficiary)
- Replaces any existing schedule for this beneficiary (resets `claimed_installments` to 0)

**Errors:**
- `NotOwner` - Caller is not the vault owner
- `InvalidAmount` - `interval` or `num_installments` is 0
- `VaultNotFound` - Vault does not exist

### `get_beneficiary_vesting`

Retrieve the vesting schedule for a specific beneficiary.

```rust
pub fn get_beneficiary_vesting(
    env: Env,
    vault_id: u64,
    beneficiary: Address,
) -> Option<BeneficiaryVestingSchedule>
```

Returns the schedule or `None` if no schedule is set for this beneficiary.

### `claim_beneficiary_vesting`

Claim available vesting installments for a beneficiary.

```rust
pub fn claim_beneficiary_vesting(
    env: Env,
    vault_id: u64,
    beneficiary: Address,
) -> Result<i128, ContractError>
```

**Constraints:**
- Vault must be in `Released` status
- A vesting schedule must be attached to this beneficiary
- Current time must be past `start_time + cliff_period` (if cliff is set)
- At least one new installment window must have elapsed since `start_time`
- Returns the total amount transferred

**Errors:**
- `VestingNotFound` - No schedule attached for this beneficiary
- `CliffNotReached` - Current time is before `start_time + cliff_period`
- `NothingToClaimYet` - No new installments available
- `AlreadyReleased` - Vault is not in Released status
- `InvalidAmount` - Calculated amount is invalid (internal error)

## Example Usage

### Basic Vesting: 4 Equal Installments Over 1 Year

```rust
// Owner creates vault and deposits 4000 XLM
let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
client.deposit(&vault_id, &owner, &4_000_000); // stroops

// Set beneficiary to receive 1000 XLM per quarter (90 days = 7,776,000 seconds)
let start_time = env.ledger().timestamp() as u64 + 86_400; // tomorrow
client.set_beneficiary_vesting(
    &vault_id,
    &owner,
    &beneficiary,
    &start_time,
    &7_776_000u64,   // 90 days
    &4u32,           // 4 installments
    &0u64,           // no cliff
);

// After vault expires, trigger release
client.trigger_release(&vault_id);

// Beneficiary claims first installment (90 days after start_time)
let amount_1 = client.claim_beneficiary_vesting(&vault_id, &beneficiary)?;
// amount_1 = 1_000_000 stroops (1000 XLM)
```

### With Cliff Period: 1-Year Lockup, Then Quarterly Vesting

```rust
let start_time = env.ledger().timestamp() as u64 + 86_400;
let one_year = 31_536_000u64;
let quarter = 7_884_000u64;

client.set_beneficiary_vesting(
    &vault_id,
    &owner,
    &beneficiary,
    &start_time,
    &quarter,
    &4u32,
    &one_year,  // cliff: no funds available until after 1 year
);

// Claiming before start_time + cliff_period fails
env.ledger().set_timestamp((start_time + 86_400) as u32);
let result = client.try_claim_beneficiary_vesting(&vault_id, &beneficiary);
// Err(CliffNotReached)

// After cliff period, claiming succeeds
env.ledger().set_timestamp((start_time + one_year + 86_400) as u32);
let amount = client.claim_beneficiary_vesting(&vault_id, &beneficiary)?;
// amount = 1_000_000 stroops
```

### Multiple Beneficiaries With Different Schedules

```rust
let ben_alice = Address::generate(&env);
let ben_bob = Address::generate(&env);

let start = env.ledger().timestamp() as u64 + 86_400;

// Alice: 4 monthly installments (30 days each)
client.set_beneficiary_vesting(
    &vault_id, &owner, &ben_alice,
    &start, &2_592_000u64, &4u32, &0u64
);

// Bob: 2 bi-annual installments (180 days each), with 1-year cliff
client.set_beneficiary_vesting(
    &vault_id, &owner, &ben_bob,
    &start, &15_552_000u64, &2u32, &31_536_000u64
);

// Each beneficiary can claim independently on their own schedule
```

## Installment Calculation

Each installment amount is calculated as:

```
per_installment = total_amount / num_installments
```

The **last installment** absorbs any remainder to ensure all funds are distributed.

**Example:** 1,000,001 stroops over 3 installments:
- Installment 1: 333,333
- Installment 2: 333,333
- Installment 3: 333,335 (absorbs remainder)

## Schedule Updates

Calling `set_beneficiary_vesting` on an existing beneficiary replaces their schedule:

```rust
// Initial schedule: 4 installments
client.set_beneficiary_vesting(&vault_id, &owner, &beneficiary, &start, &interval, &4u32, &0u64);

// Update to 6 installments (claimed_installments resets to 0)
client.set_beneficiary_vesting(&vault_id, &owner, &beneficiary, &start, &interval, &6u32, &0u64);
```

## Data Structures

### BeneficiaryVestingSchedule

```rust
pub struct BeneficiaryVestingSchedule {
    pub beneficiary: Address,
    pub vault_id: u64,
    pub start_time: u64,             // Unix timestamp of first unlock
    pub interval: u64,               // seconds between installments
    pub num_installments: u32,       // total tranches
    pub claimed_installments: u32,   // already claimed
    pub total_amount: i128,          // amount allocated at creation
    pub cliff_period: u64,           // lock-up duration (0 = no cliff)
}
```

## Events

| Topic | Data | Emitted when |
|-------|------|--------------|
| `set_bvst` | `(beneficiary, start_time, interval, num_installments)` | Schedule is set |
| `clm_bvst` | `(beneficiary, amount)` | Installment is claimed |

## Security Considerations

- **No Claim Authority**: Only the beneficiary themselves can claim (they have the funds once vault is released)
- **Time-Based Release**: Relies on ledger timestamps for all timing
- **Immutable Amounts**: Once set, `total_amount` is fixed to vault balance at that moment
- **No Cliff Bypass**: Cliff periods are strictly enforced on every claim attempt

## Common Patterns

### On-Time Incentive (with Vesting Bonus)

Combine with vesting bonus feature to reward timely claims:

```rust
// Set vesting bonus: 1% for claiming within 7 days
client.set_vesting_bonus(&vault_id, &owner, &100u32, &604_800u64);

// Set beneficiary vesting
client.set_beneficiary_vesting(&vault_id, &owner, &ben, &start, &interval, &4u32, &0u64);

// Beneficiary claims on time and gets bonus
let amount = client.claim_with_bonus(&vault_id)?;
// Receives: installment_amount * 1.01
```

### Graduated Vesting (Progressive Access)

Stagger vesting start times to create graduated access:

```rust
let base_start = env.ledger().timestamp() as u64;
let quarter = 86_400u64 * 90;

// Beneficiary 1: Immediate access to first tranche
client.set_beneficiary_vesting(&vault_id, &owner, &ben1, &base_start, &quarter, &2u32, &0u64);

// Beneficiary 2: 6-month delay before access
client.set_beneficiary_vesting(&vault_id, &owner, &ben2, &(base_start + quarter * 2), &quarter, &2u32, &0u64);
```

### Cliff Lock (Minimum Hold Period)

Use cliff period to enforce a minimum hold duration:

```rust
let cliff_6_months = 15_552_000u64;

client.set_beneficiary_vesting(
    &vault_id, &owner, &beneficiary,
    &start_time,
    &7_884_000u64,   // quarterly unlocks
    &4u32,           // after cliff expires
    &cliff_6_months, // lock for 6 months first
);
```

## Limitations

- Maximum 32-bit installment count (~4.2 billion per schedule)
- Start times in future only (no backdated vesting)
- Cliff period is fixed once set (cannot adjust after creation)
- No vesting acceleration or cancellation (see Vesting Acceleration feature for acceleration)
