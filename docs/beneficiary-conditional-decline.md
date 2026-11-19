# Beneficiary Conditional Decline with Threshold

**Issue**: #503  
**Status**: Implemented  
**Last Updated**: 2026-08-29

## Overview

Beneficiary Conditional Decline allows a beneficiary to decline their role in a vault **conditionally**, with a maximum balance threshold. This feature enables beneficiaries to signal their unwillingness to accept an inheritance if the vault balance falls below their expectations, avoiding low-value inheritance obligations.

The decline is a **signal** rather than a block—the vault owner retains ultimate control over the release mechanism, but beneficiaries can express their concerns clearly.

## Use Cases

1. **Low-Value Inheritance Rejection**: A beneficiary declines to accept a vault containing only 100 XLM when they expect at least 1,000 XLM.
2. **Administrative Burden Avoidance**: A beneficiary wants to avoid the administrative overhead of managing a small inheritance.
3. **Conditional Acceptance**: Working alongside `accept_with_threshold`, beneficiaries can both accept high-value vaults and decline low-value ones.
4. **Risk Management**: Beneficiaries can pre-set thresholds to avoid inheriting assets below a certain value, which may not justify managing them.

## API

### `decline_with_threshold(vault_id: u64, max_balance_threshold: i128, reason: String) -> Result<(), ContractError>`

**Caller**: Beneficiary only  
**Auth**: Required

Beneficiary declines the vault role with a maximum balance threshold and optional reason.

**Parameters**:
- `vault_id`: The vault ID
- `max_balance_threshold`: Maximum balance (in stroops) acceptable for the decline. Must be > 0.
- `reason`: Human-readable reason for the decline (max 256 characters)

**Returns**: `Ok(())` on success, or `ContractError` on failure.

**Errors**:
- `InvalidAmount`: If `max_balance_threshold <= 0` or reason exceeds 256 characters
- `NotBeneficiary`: If caller is not the beneficiary
- `VaultNotFound`: If vault does not exist
- `VaultPaused`: If the vault is paused

**Events**: Emits `BENEFICIARY_DECLINED_TOPIC` with:
- `vault_id`
- `beneficiary` address
- `max_balance_threshold`

**Example**:
```rust
// Beneficiary declines if balance is less than 1,000,000 stroops
client.decline_with_threshold(
    &vault_id, 
    &1_000_000i128,
    &String::from_str(&env, "Vault balance too low for my obligations")
)?;
```

### `get_beneficiary_conditional_decline(vault_id: u64) -> Option<BeneficiaryConditionalDecline>`

**Caller**: Anyone  
**Auth**: Not required

Retrieves the beneficiary's conditional decline entry if it exists.

**Returns**: 
- `Some(BeneficiaryConditionalDecline)` if a conditional decline exists
- `None` if no conditional decline has been set

**Structure**:
```rust
pub struct BeneficiaryConditionalDecline {
    pub max_balance_threshold: i128,
    pub declined_at: u64,  // Ledger timestamp
    pub reason: String,    // Human-readable reason (max 256 chars)
}
```

**Example**:
```rust
if let Some(decline) = client.get_beneficiary_conditional_decline(&vault_id) {
    println!("Max threshold: {}", decline.max_balance_threshold);
    println!("Reason: {}", decline.reason);
    println!("Declined at: {}", decline.declined_at);
}
```

## Behavior

### Threshold Semantics

- **Max Balance Threshold**: The beneficiary's signal applies if the vault balance is **below** this threshold
- If `vault.balance < max_balance_threshold`, the decline is "active"
- If `vault.balance >= max_balance_threshold`, the decline is "inactive"
- The threshold is inclusive: `balance == threshold` does NOT trigger the decline

### Reason Storage

- The `reason` field stores the beneficiary's explanation for the decline
- Used for off-chain communication (e.g., UIs can display the reason to the vault owner)
- Maximum length: 256 characters
- Can be empty or contain any UTF-8 string

### Relationship to Release

Unlike `accept_with_threshold`, which **prevents** release if conditions aren't met:
- `decline_with_threshold` is informational—it doesn't block release
- The vault owner retains full control over `trigger_release`
- The decline serves as a signal to the owner about the beneficiary's preferences
- Monitoring systems can use this to alert owners: "Beneficiary has declined due to low balance"

### Overwriting Declines

- If a beneficiary calls `decline_with_threshold` multiple times, the most recent call overwrites previous ones
- The new `max_balance_threshold` and `reason` replace the old values
- This allows beneficiaries to adjust their thresholds as circumstances change

## Events

### `BENEFICIARY_DECLINED_TOPIC` (ben_dec)

Emitted when a beneficiary declines with a threshold condition.

**Data**:
```
(vault_id: u64, beneficiary: Address, max_balance_threshold: i128)
```

**Off-Chain Indexing**: 
- Use this event to notify owners that a beneficiary has expressed concerns about the vault balance
- Display the `max_balance_threshold` and stored reason in user interfaces

## Error Handling

| Error | Cause | Resolution |
|-------|-------|-----------|
| `InvalidAmount` | Threshold <= 0 or reason > 256 chars | Set positive threshold; shorten reason |
| `NotBeneficiary` | Caller is not beneficiary | Call as the beneficiary |
| `VaultNotFound` | Vault does not exist | Verify vault ID |
| `VaultPaused` | Vault is paused | Wait for vault to resume or contact owner |

## Interaction with Other Features

### Accept with Threshold (Issue #503)
- `accept_with_threshold` and `decline_with_threshold` are **independent**
- A beneficiary can set both on the same vault
- `accept_with_threshold` enforces a minimum balance requirement at release
- `decline_with_threshold` is a signal about preferences

### Multi-Beneficiary Splits
- Conditional decline applies to the **primary beneficiary** only
- Multi-beneficiary splits are not affected by this feature
- Each beneficiary in a split can independently set their own decline threshold

### Release Conditions
- Release succeeds even if a beneficiary has declined
- The decline does not affect the release logic
- External systems should monitor decline events and notify owners

### Vesting Schedules
- Decline is independent of vesting logic
- Both can be applied to the same vault
- Decline does not affect vesting schedule execution

## Examples

### Example 1: Simple Decline

```rust
// Owner creates vault with 100-second check-in interval
let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None)?;

// Owner deposits 100,000 stroops
client.deposit(&vault_id, &owner, &100_000i128)?;

// Beneficiary signals they'd prefer at least 500,000
client.decline_with_threshold(
    &vault_id, 
    &500_000i128,
    &String::from_str(&env, "Not enough to justify accepting")
)?;

// Vault still releases normally after expiry
client.trigger_release(&vault_id)?;
// Beneficiary receives 100_000, despite the decline signal
```

### Example 2: Monitoring Decline Signals

```rust
// Off-chain system checks for decline signals
if let Some(decline) = client.get_beneficiary_conditional_decline(&vault_id) {
    let vault = client.get_vault(&vault_id);
    
    if vault.balance < decline.max_balance_threshold {
        // Alert owner
        println!("⚠️  Beneficiary has declined due to low balance");
        println!("   Current: {} stroops", vault.balance);
        println!("   Expected: {} stroops", decline.max_balance_threshold);
        println!("   Reason: {}", decline.reason);
    }
}
```

### Example 3: Updating Decline Threshold

```rust
// Beneficiary initially declines if < 500,000
client.decline_with_threshold(&vault_id, &500_000i128, &reason1)?;

// Later decides 250,000 is acceptable
client.decline_with_threshold(&vault_id, &250_000i128, &reason2)?;

// The new threshold replaces the old one
let decline = client.get_beneficiary_conditional_decline(&vault_id);
assert_eq!(decline.unwrap().max_balance_threshold, 250_000i128);
```

## Testing

Comprehensive tests are included in `contracts/ttl_vault/src/test.rs`:

- `test_decline_with_threshold_beneficiary_only`: Validates beneficiary-only access
- `test_decline_with_threshold_owner_fails`: Ensures owner cannot decline
- `test_decline_with_threshold_invalid_amount`: Tests threshold validation
- `test_decline_with_threshold_reason_too_long`: Validates reason length limit
- `test_decline_with_threshold_stores_timestamp`: Verifies timestamp recording
- `test_decline_with_threshold_emits_event`: Validates event emission
- `test_decline_with_threshold_multiple_declines`: Tests threshold overwriting
- `test_trigger_release_with_decline_threshold_below_max`: Ensures release proceeds despite decline
- `test_decline_with_threshold_with_reason_stored`: Verifies reason storage and retrieval

## Security Considerations

1. **Decline Does Not Block Release**: Unlike acceptance thresholds, declines are purely informational. They cannot prevent a release.
2. **Auth Required**: Only the beneficiary can set a decline. The owner cannot force a decline or prevent one.
3. **Immutability Window**: A decline is active until overwritten. The beneficiary controls this entirely.
4. **Reason Storage**: Reasons are stored on-chain and visible to all. Do not include sensitive information.

## Future Enhancements

- Support multiple decline conditions (e.g., time-based escalation)
- Integration with estate distribution systems to auto-route declined vaults
- Decline expiry (auto-clear after a certain time)
- Beneficiary groups with shared decline policies
- Conditional routing to alternative recipients if primary beneficiary declines

## Related Features

- [Beneficiary Conditional Acceptance with Threshold](beneficiary-conditional-acceptance.md)
- [Beneficiary Conflict Resolution](beneficiary-conflict-resolution.md)
- [Multi-Beneficiary Splits](beneficiary-advanced-features.md)
