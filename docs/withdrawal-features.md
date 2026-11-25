# Withdrawal Features Documentation

This document describes the four new withdrawal features implemented in Issues #565-#568.

## Issue #565: Withdrawal Scheduling Validation

### Overview
Prevents overlapping or conflicting withdrawal schedules by validating that scheduled withdrawals don't occur within a 1-hour window of each other.

### Key Functions

#### `schedule_withdrawal(vault_id, caller, timestamp, amount) -> Result<(), ContractError>`
Schedules a withdrawal with conflict detection.

**Parameters:**
- `vault_id`: The vault ID
- `caller`: The vault owner (must be authenticated)
- `timestamp`: Unix timestamp for the scheduled withdrawal
- `amount`: Amount to withdraw in stroops

**Returns:**
- `Ok(())` on success
- `Err(ContractError::ConflictingWithdrawalSchedule)` if overlapping with existing schedule
- `Err(ContractError::NotOwner)` if caller is not the vault owner
- `Err(ContractError::AlreadyReleased)` if vault is not in Locked status

**Events:**
- `WITHDRAWAL_VALIDATION_TOPIC`: Emitted when a withdrawal is successfully scheduled

### Implementation Details
- Maintains a vector of `WithdrawalScheduleEntry` structs per vault
- Checks for conflicts within a 1-hour (3600 second) window
- Prevents scheduling withdrawals that would overlap with existing schedules
- Stores schedules in persistent storage with TTL management

### Error Codes
- `OverlappingWithdrawalSchedule = 64`: Withdrawal overlaps with existing schedule
- `ConflictingWithdrawalSchedule = 65`: Withdrawal conflicts with existing schedule

---

## Issue #566: Withdrawal Limits by Time

### Overview
Implements daily, weekly, and monthly withdrawal limits with automatic period resets. Limits are tracked per vault and reset automatically when their respective periods expire.

### Key Functions

#### `set_withdrawal_limits(vault_id, caller, daily_limit, weekly_limit, monthly_limit) -> Result<(), ContractError>`
Configures withdrawal limits for a vault.

**Parameters:**
- `vault_id`: The vault ID
- `caller`: The vault owner (must be authenticated)
- `daily_limit`: Maximum amount withdrawable per day (in stroops)
- `weekly_limit`: Maximum amount withdrawable per week (in stroops)
- `monthly_limit`: Maximum amount withdrawable per month (in stroops)

**Returns:**
- `Ok(())` on success
- `Err(ContractError::NotOwner)` if caller is not the vault owner

**Events:**
- `WITHDRAWAL_LIMIT_SET_TOPIC`: Emitted when limits are configured

#### `get_withdrawal_limits(vault_id) -> Option<WithdrawalLimit>`
Retrieves the current withdrawal limits for a vault.

**Returns:**
- `Some(WithdrawalLimit)` if limits are configured
- `None` if no limits are set

### Data Structures

#### `WithdrawalLimit`
```rust
pub struct WithdrawalLimit {
    pub daily_limit: i128,
    pub weekly_limit: i128,
    pub monthly_limit: i128,
}
```

#### `WithdrawalTracker`
```rust
pub struct WithdrawalTracker {
    pub daily_withdrawn: i128,
    pub daily_reset_at: u64,
    pub weekly_withdrawn: i128,
    pub weekly_reset_at: u64,
    pub monthly_withdrawn: i128,
    pub monthly_reset_at: u64,
}
```

### Implementation Details
- Limits are checked during every `withdraw()` call
- Trackers automatically reset when their period expires
- Daily period: 24 hours (86,400 seconds)
- Weekly period: 7 days (604,800 seconds)
- Monthly period: 30 days (2,592,000 seconds)
- Limits are optional; if not set, no restrictions apply

### Error Codes
- `DailyWithdrawalLimitExceeded = 66`: Daily limit would be exceeded
- `WeeklyWithdrawalLimitExceeded = 67`: Weekly limit would be exceeded
- `MonthlyWithdrawalLimitExceeded = 68`: Monthly limit would be exceeded

### Events
- `WITHDRAWAL_LIMIT_SET_TOPIC`: Emitted when limits are configured
- `WITHDRAWAL_LIMIT_EXCEEDED_TOPIC`: Emitted when a limit is exceeded

---

## Issue #567: Withdrawal Destination Whitelist

### Overview
Restricts withdrawals to only whitelisted addresses. Vault owners can add and remove addresses from the whitelist.

### Key Functions

#### `add_whitelist_address(vault_id, caller, address, label) -> Result<(), ContractError>`
Adds an address to the withdrawal whitelist.

**Parameters:**
- `vault_id`: The vault ID
- `caller`: The vault owner (must be authenticated)
- `address`: The address to whitelist
- `label`: A descriptive label for the address (e.g., "cold_wallet")

**Returns:**
- `Ok(())` on success
- `Err(ContractError::NotOwner)` if caller is not the vault owner

**Events:**
- `WHITELIST_ADDED_TOPIC`: Emitted when an address is added

#### `remove_whitelist_address(vault_id, caller, address) -> Result<(), ContractError>`
Removes an address from the withdrawal whitelist.

**Parameters:**
- `vault_id`: The vault ID
- `caller`: The vault owner (must be authenticated)
- `address`: The address to remove

**Returns:**
- `Ok(())` on success
- `Err(ContractError::NotOwner)` if caller is not the vault owner

**Events:**
- `WHITELIST_REMOVED_TOPIC`: Emitted when an address is removed

#### `get_whitelist(vault_id) -> Option<Vec<WhitelistEntry>>`
Retrieves the whitelist for a vault.

**Returns:**
- `Some(Vec<WhitelistEntry>)` if whitelist exists
- `None` if no whitelist is configured

### Data Structures

#### `WhitelistEntry`
```rust
pub struct WhitelistEntry {
    pub address: Address,
    pub added_at: u64,
    pub label: String,
}
```

### Implementation Details
- If no whitelist is configured, all addresses are allowed (backward compatible)
- If a whitelist exists, only whitelisted addresses can receive withdrawals
- Whitelist entries include timestamps for audit trails
- Whitelist is stored in persistent storage with TTL management

### Error Codes
- `WithdrawalDestinationNotWhitelisted = 69`: Destination address is not whitelisted

### Events
- `WHITELIST_ADDED_TOPIC`: Emitted when an address is added
- `WHITELIST_REMOVED_TOPIC`: Emitted when an address is removed
- `WHITELIST_VIOLATION_TOPIC`: Emitted when a withdrawal to non-whitelisted address is attempted

---

## Issue #568: Withdrawal Reversal

### Overview
Allows vault owners to reverse withdrawals within a grace period (24 hours by default). Reversed withdrawals restore funds to the vault.

### Key Functions

#### `reverse_withdrawal(vault_id, caller, withdrawal_id) -> Result<(), ContractError>`
Reverses a withdrawal within the grace period.

**Parameters:**
- `vault_id`: The vault ID
- `caller`: The vault owner (must be authenticated)
- `withdrawal_id`: The ID of the withdrawal to reverse

**Returns:**
- `Ok(())` on success
- `Err(ContractError::WithdrawalReversalGracePeriodExpired)` if grace period has expired
- `Err(ContractError::WithdrawalAlreadyReversed)` if already reversed
- `Err(ContractError::NotOwner)` if caller is not the vault owner

**Events:**
- `WITHDRAWAL_REVERSED_TOPIC`: Emitted when a withdrawal is successfully reversed
- `WITHDRAWAL_CANCELLED_TOPIC`: Emitted when a withdrawal is cancelled (Issue #1134)

#### `get_withdrawal_reversal(vault_id, withdrawal_id) -> Option<WithdrawalReversal>`
Retrieves a withdrawal reversal record.

**Returns:**
- `Some(WithdrawalReversal)` if the record exists
- `None` if not found

### Data Structures

#### `WithdrawalReversal`
```rust
pub struct WithdrawalReversal {
    pub withdrawal_id: u64,
    pub amount: i128,
    pub withdrawn_at: u64,
    pub grace_period_until: u64,
    pub reversed: bool,
}
```

### Implementation Details
- Every withdrawal is automatically recorded for potential reversal
- Grace period is 24 hours (86,400 seconds) from withdrawal time
- Withdrawal IDs are auto-incremented per vault
- Reversals restore funds to the vault balance
- Once reversed, a withdrawal cannot be reversed again
- Reversal records are stored in persistent storage with TTL management

### Error Codes
- `WithdrawalReversalGracePeriodExpired = 70`: Grace period has expired
- `WithdrawalAlreadyReversed = 71`: Withdrawal has already been reversed

### Events
- `WITHDRAWAL_REVERSED_TOPIC`: Emitted when a withdrawal is reversed
- `WITHDRAWAL_CANCELLED_TOPIC`: Emitted when a withdrawal is cancelled (Issue #1134)
- `REVERSAL_GRACE_EXPIRED_TOPIC`: Emitted when a grace period expires

---

## Integration with Existing Withdrawal Function

All four features are integrated into the existing `withdraw()` function:

```rust
pub fn withdraw(env: Env, vault_id: u64, caller: Address, amount: i128) -> Result<(), ContractError>
```

The withdrawal process now:
1. Validates the caller is the vault owner
2. Checks withdrawal approval threshold (Issue #404)
3. **Checks withdrawal limits** (Issue #566)
4. **Validates whitelist** (Issue #567)
5. Transfers funds to the owner
6. **Records withdrawal for reversal** (Issue #568)
7. Emits withdrawal event

---

## Usage Examples

### Example 1: Setting Up Withdrawal Limits

```rust
// Set daily limit of 10 XLM, weekly of 50 XLM, monthly of 100 XLM
client.set_withdrawal_limits(
    &vault_id,
    &owner,
    &(10 * 10_000_000i128),      // 10 XLM in stroops
    &(50 * 10_000_000i128),      // 50 XLM in stroops
    &(100 * 10_000_000i128),     // 100 XLM in stroops
)?;
```

### Example 2: Whitelisting Addresses

```rust
// Add a cold wallet to the whitelist
client.add_whitelist_address(
    &vault_id,
    &owner,
    &cold_wallet_address,
    &String::from_str(&env, "cold_storage"),
)?;

// Withdrawals can now only go to whitelisted addresses
client.withdraw(&vault_id, &owner, &amount)?;
```

### Example 3: Reversing a Withdrawal

```rust
// Withdraw funds
client.withdraw(&vault_id, &owner, &amount)?;

// Within 24 hours, reverse the withdrawal
client.reverse_withdrawal(&vault_id, &owner, &0u64)?;

// Funds are restored to the vault
```

### Example 4: Scheduling Withdrawals

```rust
// Schedule a withdrawal for tomorrow
let tomorrow = env.ledger().timestamp() + 86_400u64;
client.schedule_withdrawal(
    &vault_id,
    &owner,
    &tomorrow,
    &amount,
)?;
```

---

## Security Considerations

1. **Withdrawal Limits**: Limits are per-vault and reset automatically. Owners should set appropriate limits based on their risk tolerance.

2. **Whitelist**: If a whitelist is configured, only whitelisted addresses can receive withdrawals. This prevents accidental transfers to wrong addresses.

3. **Reversal Grace Period**: The 24-hour grace period allows owners to recover from mistakes. After the grace period, reversals are no longer possible.

4. **Scheduling**: Scheduled withdrawals prevent overlapping transactions within a 1-hour window, reducing the risk of double-spending.

5. **Authorization**: All configuration functions require owner authentication via `require_auth()`.

---

## Storage Efficiency

- Withdrawal schedules are stored per-vault in a vector
- Withdrawal limits and trackers are stored per-vault
- Whitelist entries are stored per-vault in a vector
- Reversal records are stored with (vault_id, withdrawal_id) as key
- All storage uses TTL management to prevent bloat

---

## Future Enhancements

1. **Configurable Grace Period**: Allow owners to set custom reversal grace periods
2. **Withdrawal Notifications**: Emit events for monitoring systems
3. **Batch Reversals**: Reverse multiple withdrawals in a single transaction
4. **Limit Adjustments**: Allow dynamic limit adjustments without resetting trackers
5. **Whitelist Expiry**: Add expiration dates to whitelist entries

---

## Issue #1294: Withdrawal Dispute Window

### Overview
Allows vault owners to dispute unauthorized or suspicious withdrawals within a **24-hour grace period** after the transaction was recorded. Disputed withdrawals are flagged for admin review and resolution.

The dispute is linked directly to a specific entry in the vault's withdrawal audit log (identified by its zero-based index), ensuring traceability to the exact on-chain transaction.

### Key Functions

#### `dispute_withdrawal(vault_id, caller, audit_log_index, reason) -> Result<(), ContractError>`
Files a dispute against an existing successful withdrawal.

**Parameters:**
- `vault_id`: The vault ID
- `caller`: The vault owner (must be authenticated)
- `audit_log_index`: Zero-based index into `get_withdrawal_audit_log` identifying the disputed withdrawal
- `reason`: Human-readable description of the dispute

**Returns:**
- `Ok(())` on success
- `Err(ContractError::NotOwner)` if caller is not the vault owner
- `Err(ContractError::WithdrawalDisputeNotFound)` if the audit log index is invalid or references a failed (non-fund-moving) withdrawal
- `Err(ContractError::WithdrawalDisputeWindowExpired)` if more than 24 hours have elapsed since the withdrawal
- `Err(ContractError::DisputeFiled)` if an open dispute already exists for that withdrawal

**Events:**
- `WITHDRAWAL_DISPUTE_FILED_TOPIC` (`wd_disp`): Emitted on successful filing, with `(caller, withdrawal_timestamp, reason)`

#### `resolve_withdrawal_dispute(vault_id, dispute_index, approved) -> Result<(), ContractError>`
Resolves a pending dispute. **Admin-only.**

**Parameters:**
- `vault_id`: The vault ID
- `dispute_index`: Zero-based index into `get_withdrawal_disputes`
- `approved`: `true` = dispute upheld (`Resolved`), `false` = dispute dismissed (`None`)

**Returns:**
- `Ok(())` on success
- `Err(ContractError::NotAdmin)` if caller is not the contract admin
- `Err(ContractError::WithdrawalDisputeNotFound)` if `dispute_index` is out of range or no disputes exist
- `Err(ContractError::DisputeFiled)` if the dispute is not in `Filed` state (already resolved/dismissed)

**Events:**
- `WITHDRAWAL_DISPUTE_RESOLVED_TOPIC` (`wd_disp_r`): Emitted on resolution, with `(admin, dispute_index, approved)`

#### `get_withdrawal_disputes(vault_id) -> Vec<WithdrawalDispute>`
Returns all dispute entries (filed, resolved, and dismissed) for a vault.

#### `file_withdrawal_dispute(vault_id, caller, audit_log_index, reason) -> Result<(), ContractError>` *(legacy shim)*
Backwards-compatible wrapper; delegates to `dispute_withdrawal`. New integrations should call `dispute_withdrawal` directly.

### Data Structures

#### `WithdrawalDispute`
```rust
pub struct WithdrawalDispute {
    pub vault_id: u64,
    /// Timestamp of the withdrawal that is being disputed (from the audit log entry).
    pub withdrawal_timestamp: u64,
    /// Timestamp when the dispute was filed.
    pub dispute_filed_at: u64,
    /// Deadline by which the dispute window closes (withdrawal_timestamp + 24 h).
    pub dispute_expires_at: u64,
    pub status: DisputeStatus,
    pub reason: String,
    /// Set when the dispute is resolved or dismissed.
    pub resolved_at: Option<u64>,
}
```

#### `DisputeStatus`
```rust
pub enum DisputeStatus {
    None,     // Dismissed or not yet filed
    Filed,    // Active, awaiting admin resolution
    Resolved, // Upheld by admin
}
```

### Error Codes
- `WithdrawalDisputeWindowExpired = 101`: More than 24 hours since the withdrawal
- `WithdrawalDisputeNotFound = 102`: Invalid audit log index or no disputes at the given dispute index

### Implementation Details
- The 24-hour window is measured from `WithdrawalAuditEntry.timestamp` (the block time the withdrawal was recorded), not the time of filing.
- Only **successful** withdrawals (those that moved funds) can be disputed. Failed audit entries return `WithdrawalDisputeNotFound`.
- Duplicate open disputes for the same withdrawal timestamp are prevented; a dismissed dispute allows the window to be re-opened.
- Resolution is admin-only: vault owners cannot resolve their own disputes to prevent self-clearing.
- Disputes are stored per-vault in a persistent `Vec<WithdrawalDispute>` under `StorageKey::WithdrawalDisputes(vault_id)`.

### Usage Example

```rust
// 1. After a withdrawal occurs, query the audit log to find its index.
let audit_log = client.get_withdrawal_audit_log(&vault_id);
let idx = 0u32; // index of the suspicious withdrawal

// 2. File a dispute (must be called within 24 h of the withdrawal).
client.dispute_withdrawal(
    &vault_id,
    &owner,
    &idx,
    &String::from_str(&env, "I did not authorize this withdrawal"),
)?;

// 3. Admin reviews and resolves.
client.resolve_withdrawal_dispute(
    &vault_id,
    &0u32,  // dispute index in get_withdrawal_disputes
    &true,  // true = upheld, false = dismissed
)?;

// 4. Check final status.
let disputes = client.get_withdrawal_disputes(&vault_id);
assert_eq!(disputes.get(0).unwrap().status, DisputeStatus::Resolved);
```

### Security Considerations
- The 24-hour window limits the exposure window while giving owners reasonable time to detect anomalies.
- Linking disputes to audit log entries creates a tamper-evident chain between the on-chain transaction record and the dispute.
- Admin-only resolution prevents conflicts of interest and ensures neutral arbitration.
