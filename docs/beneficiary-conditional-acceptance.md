# Beneficiary Conditional Acceptance with Threshold

**Issue**: #503  
**Status**: Implemented  
**Last Updated**: 2026-05-28

## Overview

Beneficiary Conditional Acceptance allows a beneficiary to accept their role in a vault **conditionally**, with a minimum balance threshold. The beneficiary will only receive funds if the vault balance meets or exceeds the specified threshold at the time of release.

This feature enables beneficiaries to:
- Accept inheritance only if the estate is substantial enough
- Avoid accepting vaults with insufficient funds
- Set clear expectations about minimum inheritance amounts

## Use Cases

1. **Minimum Inheritance Threshold**: A beneficiary accepts only if the vault contains at least $10,000 worth of XLM.
2. **Conditional Acceptance**: A beneficiary wants to ensure the vault has grown to a certain level before accepting.
3. **Risk Management**: Beneficiaries can decline acceptance if the vault balance falls below expectations.

## API

### `accept_with_threshold(vault_id: u64, min_balance_threshold: i128) -> Result<(), ContractError>`

**Caller**: Beneficiary only  
**Auth**: Required

Beneficiary accepts the vault role with a minimum balance threshold condition.

**Parameters**:
- `vault_id`: The vault ID
- `min_balance_threshold`: Minimum balance (in stroops) required for release. Must be > 0.

**Returns**: `Ok(())` on success, or `ContractError` on failure.

**Errors**:
- `InvalidAmount`: If `min_balance_threshold <= 0`
- `NotBeneficiary`: If caller is not the beneficiary
- `VaultNotFound`: If vault does not exist

**Events**: Emits `BENEFICIARY_CONDITION_ACCEPTED_TOPIC` with:
- `vault_id`
- `beneficiary` address
- `min_balance_threshold`

**Example**:
```rust
// Beneficiary accepts vault only if balance >= 500,000 stroops
client.accept_with_threshold(&vault_id, &500_000i128)?;
```

### `get_beneficiary_conditional_acceptance(vault_id: u64) -> Option<BeneficiaryConditionalAcceptance>`

**Caller**: Anyone  
**Auth**: Not required

Retrieves the beneficiary's conditional acceptance entry if it exists.

**Returns**: 
- `Some(BeneficiaryConditionalAcceptance)` if a conditional acceptance exists
- `None` if no conditional acceptance has been set

**Structure**:
```rust
pub struct BeneficiaryConditionalAcceptance {
    pub min_balance_threshold: i128,
    pub accepted_at: u64,  // Ledger timestamp
}
```

**Example**:
```rust
if let Some(acceptance) = client.get_beneficiary_conditional_acceptance(&vault_id) {
    println!("Threshold: {}", acceptance.min_balance_threshold);
    println!("Accepted at: {}", acceptance.accepted_at);
}
```

## Behavior

### Release Validation

When `trigger_release()` is called on an expired vault:

1. **Check Beneficiary Status**: Ensure beneficiary hasn't declined
2. **Check Conditional Acceptance Threshold** (NEW):
   - If a conditional acceptance exists, verify `vault.balance >= min_balance_threshold`
   - If threshold is not met, release fails with `InsufficientBalance` error
   - If no conditional acceptance exists, proceed normally
3. **Check Other Conditions**: Proof of life, voting, etc.
4. **Execute Release**: Transfer funds to beneficiary

### Threshold Validation

- Threshold must be **positive** (> 0)
- Threshold is checked against the **current vault balance** at release time
- If balance decreases after acceptance (e.g., via withdrawal), the threshold may no longer be met
- Threshold is **inclusive**: balance == threshold is acceptable

## Events

### `BENEFICIARY_CONDITION_ACCEPTED_TOPIC` (ben_cond)

Emitted when a beneficiary accepts with a threshold condition.

**Data**:
```
(vault_id: u64, beneficiary: Address, min_balance_threshold: i128)
```

## Error Handling

| Error | Cause | Resolution |
|-------|-------|-----------|
| `InvalidAmount` | Threshold <= 0 | Set a positive threshold |
| `NotBeneficiary` | Caller is not beneficiary | Call as the beneficiary |
| `InsufficientBalance` | Balance < threshold at release | Wait for more deposits or lower threshold |
| `VaultNotFound` | Vault does not exist | Verify vault ID |

## Interaction with Other Features

### Vesting Schedules
- Conditional acceptance threshold is checked **before** vesting schedule logic
- If threshold is not met, release fails regardless of vesting schedule

### Multi-Beneficiary Splits
- Conditional acceptance applies to the **primary beneficiary** only
- Multi-beneficiary splits are not affected by this feature

### Spending Limits
- Spending limit is applied **after** threshold validation
- If threshold is met but spending limit is lower, the spending limit applies

### Conditional Acceptance (Issue #400)
- Conditional acceptance (string-based conditions) and threshold-based acceptance are **independent**
- Both can be set on the same vault
- Both are checked during release

## Examples

### Example 1: Simple Threshold Acceptance

```rust
// Owner creates vault with 100-second check-in interval
let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None)?;

// Owner deposits 1,000,000 stroops
client.deposit(&vault_id, &owner, &1_000_000i128)?;

// Beneficiary accepts only if balance >= 500,000
client.accept_with_threshold(&vault_id, &500_000i128)?;

// After vault expires, release succeeds because 1,000,000 >= 500,000
client.trigger_release(&vault_id)?;
```

### Example 2: Threshold Not Met

```rust
// Owner creates vault
let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None)?;

// Owner deposits only 100,000 stroops
client.deposit(&vault_id, &owner, &100_000i128)?;

// Beneficiary accepts with threshold of 500,000
client.accept_with_threshold(&vault_id, &500_000i128)?;

// After vault expires, release fails because 100,000 < 500,000
let result = client.trigger_release(&vault_id);
assert!(result.is_err());  // InsufficientBalance
```

### Example 3: Checking Acceptance Status

```rust
// Check if beneficiary has set a conditional acceptance
if let Some(acceptance) = client.get_beneficiary_conditional_acceptance(&vault_id) {
    println!("Threshold: {} stroops", acceptance.min_balance_threshold);
    println!("Accepted at ledger timestamp: {}", acceptance.accepted_at);
} else {
    println!("No conditional acceptance set");
}
```

## Testing

Comprehensive tests are included in `contracts/ttl_vault/src/test.rs`:

- `test_accept_with_threshold_beneficiary_only`: Validates beneficiary-only access
- `test_accept_with_threshold_invalid_amount`: Tests threshold validation
- `test_trigger_release_with_threshold_met`: Verifies successful release when threshold is met
- `test_trigger_release_with_threshold_not_met`: Verifies release failure when threshold is not met
- `test_trigger_release_without_threshold_condition`: Ensures normal release when no threshold is set
- `test_accept_with_threshold_exact_match`: Tests exact threshold match
- `test_accept_with_threshold_emits_event`: Validates event emission

## Security Considerations

1. **Threshold Immutability**: Once set, the threshold cannot be changed. Beneficiary must accept again with a new threshold.
2. **Balance Dependency**: Threshold is checked against current balance, which can change due to deposits/withdrawals.
3. **No Bypass**: Threshold is enforced at release time and cannot be bypassed.
4. **Auth Required**: Only the beneficiary can set a conditional acceptance.

## Owner-Beneficiary Validation

`create_vault` rejects configurations where the owner and beneficiary addresses are identical. An `InvalidBeneficiary` error is returned (panic in contract terms) in this case.

This prevents degenerate vaults where the release mechanism has no practical effect and blocks potential gaming of reward/streak mechanics in future features.

```rust
client.create_vault(&owner, &beneficiary, &interval, &None);
```

If `owner == beneficiary`, the contract returns `ContractError::InvalidBeneficiary`.

## Interacting with Decline Threshold (Issue #503)

Beneficiaries can both **accept** and **decline** with thresholds on the same vault:

- `accept_with_threshold(min)`: Sets minimum balance required for release. If `balance < min`, release **fails**.
- `decline_with_threshold(max, reason)`: Signals unwillingness to accept if `balance < max`. Release proceeds regardless.

These are **independent** mechanisms:
- Acceptance threshold **blocks** release if not met
- Decline threshold is purely **informational**

**Example Workflow**:
```rust
// Beneficiary accepts if balance >= 100,000
client.accept_with_threshold(&vault_id, &100_000i128)?;

// Beneficiary signals they'd prefer >= 500,000
client.decline_with_threshold(
    &vault_id,
    &500_000i128,
    &String::from_str(&env, "Vault is smaller than expected")
)?;

// After expiry:
// - If balance < 100,000: release fails (acceptance not met)
// - If 100,000 <= balance < 500,000: release succeeds, but decline signal is active (owner notified)
// - If balance >= 500,000: release succeeds, decline signal is inactive
```

See [Beneficiary Conditional Decline with Threshold](beneficiary-conditional-decline.md) for details.

## Future Enhancements

- Allow beneficiary to update threshold before release
- Support multiple threshold conditions (e.g., min AND max balance)
- Time-based threshold adjustments (e.g., threshold decreases over time)
- Conditional acceptance with custom logic (e.g., threshold based on token price)
- Integrated decline escalation (auto-route to alternative recipient if beneficiary declines)
