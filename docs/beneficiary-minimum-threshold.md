# Beneficiary Minimum Threshold — Issue #512

## Overview

The Beneficiary Minimum Threshold feature allows vault owners to set a minimum amount each beneficiary must receive. If a beneficiary's calculated share falls below their minimum threshold, they receive nothing and their funds are redistributed to other qualifying beneficiaries.

This feature is useful for:
- Preventing dust amounts being sent to certain beneficiaries
- Ensuring meaningful minimum payments to beneficiaries
- Implementing tiered distribution strategies based on vault size

## How It Works

### 1. Setting Minimum Thresholds

Each beneficiary can have an individual minimum threshold (in stroops). When the vault is released:

1. The contract calculates each beneficiary's initial share based on their BPS allocation
2. For each beneficiary, it checks if their share >= their minimum_threshold
3. Beneficiaries below the threshold are skipped and their funds are redistributed
4. Qualifying beneficiaries receive their share proportionally from the total pool

### 2. Redistribution Logic

When a beneficiary doesn't meet the minimum threshold:

- They receive **nothing**
- Their calculated share is added to the redistribution pool
- The remaining funds are redistributed **only** among qualifying beneficiaries
- Redistribution is done proportionally based on each qualifying beneficiary's original BPS

### 3. Edge Cases

**All beneficiaries below threshold:**
If no beneficiary meets the minimum threshold, all funds are returned to the vault owner.

**Mixed thresholds:**
Beneficiaries with threshold = 0 will always qualify (threshold is disabled).

**Single qualifying beneficiary:**
If only one beneficiary qualifies, they receive 100% of the vault balance.

## API

### Setting Minimum Thresholds

#### `set_beneficiary_minimum_threshold`
Updates the minimum threshold for a specific beneficiary.

```rust
pub fn set_beneficiary_minimum_threshold(
    env: Env,
    vault_id: u64,
    caller: Address,
    beneficiary_address: Address,
    minimum_threshold: i128,  // in stroops
) -> Result<(), ContractError>
```

**Parameters:**
- `vault_id` - The vault ID
- `caller` - Must be the vault owner
- `beneficiary_address` - Address of the beneficiary to update
- `minimum_threshold` - Minimum amount in stroops (0 = disabled)

**Returns:**
- `Ok(())` on success
- `Err(ContractError::NotOwner)` if caller is not the vault owner
- `Err(ContractError::AlreadyReleased)` if vault is not in Locked status
- `Err(ContractError::InvalidBeneficiary)` if beneficiary is not found
- `Err(ContractError::InvalidAmount)` if threshold is negative

**Events:**
Emits `min_thr` (MIN_THRESHOLD_SET_TOPIC) with (beneficiary_address, minimum_threshold)

### Getting Minimum Thresholds

#### `get_beneficiary_minimum_threshold`
Retrieves the minimum threshold for a specific beneficiary.

```rust
pub fn get_beneficiary_minimum_threshold(
    env: Env,
    vault_id: u64,
    beneficiary_address: Address,
) -> Option<i128>
```

**Returns:**
- `Some(threshold)` if the beneficiary exists
- `None` if the beneficiary doesn't exist

#### `get_beneficiaries_with_thresholds`
Retrieves all beneficiaries and their minimum thresholds for a vault.

```rust
pub fn get_beneficiaries_with_thresholds(
    env: Env,
    vault_id: u64,
) -> Option<Vec<BeneficiaryEntry>>
```

**Returns:**
- `Some(Vec<BeneficiaryEntry>)` with all beneficiaries and their thresholds
- `None` if the vault doesn't exist

### Setting Beneficiaries

When creating or updating multi-beneficiary splits, include the `minimum_threshold` field in each BeneficiaryEntry:

```rust
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryEntry {
    pub address: Address,
    pub bps: u32,
    pub minimum_threshold: i128,  // New field
}
```

**Example:**
```rust
let entries = vec![
    BeneficiaryEntry {
        address: alice.clone(),
        bps: 5_000,              // 50%
        minimum_threshold: 1_000, // minimum 1000 stroops
    },
    BeneficiaryEntry {
        address: bob.clone(),
        bps: 5_000,              // 50%
        minimum_threshold: 2_000, // minimum 2000 stroops
    },
];
client.set_beneficiaries(&vault_id, &owner, &entries);
```

## Events

### Events Emitted

#### `min_thr` (MIN_THRESHOLD_SET_TOPIC)
Emitted when a beneficiary's minimum threshold is updated.

**Data:** `(vault_id, beneficiary_address, minimum_threshold)`

#### `thr_skip` (MIN_THRESHOLD_SKIP_TOPIC)
Emitted when a beneficiary's share is below their minimum threshold and they are skipped.

**Data:** `(vault_id, beneficiary_address, calculated_share, minimum_threshold)`

#### `thr_redis` (MIN_THRESHOLD_REDISTRIBUTE_TOPIC)
Emitted when funds are redistributed to a qualifying beneficiary.

**Data:** `(vault_id, beneficiary_address, redistributed_amount)`

## Examples

### Example 1: Basic Minimum Threshold

**Scenario:**
- Vault balance: 10,000 stroops
- 2 beneficiaries, 50% each
- Alice (50%): minimum threshold = 3,000
- Bob (50%): minimum threshold = 6,000

**Expected distribution:**
- Alice's initial share: 5,000 stroops ✓ (meets 3,000 threshold)
- Bob's initial share: 5,000 stroops ✗ (below 6,000 threshold)
- Alice receives all: 10,000 stroops
- Bob receives: 0 stroops

### Example 2: Redistribution Among Qualifying Beneficiaries

**Scenario:**
- Vault balance: 12,000 stroops
- 3 beneficiaries: 40%, 40%, 20%
- Alice (40%): minimum = 2,000
- Bob (40%): minimum = 6,000
- Charlie (20%): minimum = 1,000

**Expected distribution:**
- Alice's initial: 4,800 ✓ (meets 2,000)
- Bob's initial: 4,800 ✗ (below 6,000)
- Charlie's initial: 2,400 ✓ (meets 1,000)
- Qualifying beneficiaries: Alice + Charlie (80% of BPS)
- Alice receives: 12,000 × 40% / 80% = 6,000
- Charlie receives: 12,000 × 20% / 80% = 3,000
- Bob receives: 0

### Example 3: Return to Owner

**Scenario:**
- Vault balance: 1,000 stroops
- 2 beneficiaries, 50% each
- Both have minimum threshold = 2,000

**Expected distribution:**
- No beneficiary qualifies
- Owner receives: 1,000 stroops (refund)
- Both beneficiaries receive: 0

## Integration Notes

### Backwards Compatibility

The `minimum_threshold` field is stored in each `BeneficiaryEntry`. When setting beneficiaries via `set_beneficiaries`, callers must now provide this field. Setting it to `0` disables the minimum threshold for that beneficiary (they will always qualify).

### When Vesting Schedules Are Present

When a vault has a vesting schedule attached, `trigger_release` marks the vault as Released but doesn't distribute funds immediately. The minimum threshold logic applies when the vesting schedule is claimed via `claim_vesting_installment`.

### Interaction with Spending Limits

The spending limit (Issue #382) applies **before** the minimum threshold logic. If a spending limit is set, the release amount is first capped at the spending limit, then the minimum threshold redistribution logic is applied to that capped amount.

## Error Handling

| Error | Condition |
|-------|-----------|
| `ContractError::NotOwner` | Caller is not the vault owner |
| `ContractError::AlreadyReleased` | Vault is not in Locked status |
| `ContractError::InvalidBeneficiary` | Beneficiary not found in the vault |
| `ContractError::InvalidAmount` | Minimum threshold is negative |

## Testing

Comprehensive tests are included in `contracts/ttl_vault/src/test.rs`:

- `test_minimum_threshold_skips_beneficiary_below_threshold` - Basic threshold enforcement
- `test_minimum_threshold_redistributes_to_qualifying_beneficiaries` - Redistribution logic
- `test_minimum_threshold_all_below_threshold_returns_to_owner` - Owner fallback
- `test_set_beneficiary_minimum_threshold` - Threshold setting
- `test_get_beneficiary_minimum_threshold_not_found` - Query non-existent beneficiary
- `test_get_beneficiaries_with_thresholds` - Batch retrieval
- `test_minimum_threshold_zero_disables_threshold` - Disabled thresholds
- `test_minimum_threshold_update_existing` - Updating existing thresholds

## Performance Considerations

The minimum threshold logic requires two passes through the beneficiary list:

1. **First pass:** Calculate shares and identify qualifying beneficiaries (O(n))
2. **Second pass:** Distribute funds to qualifying beneficiaries (O(n))

Total complexity: **O(n)** where n = number of beneficiaries. Storage operations remain O(1).
