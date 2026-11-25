# Admin Token Allowlist - Issue #1118

## Overview
The admin-controlled token allowlist prevents malicious or worthless tokens from being deposited into vaults. Only tokens explicitly added to the allowlist can be used in multi-asset vault operations.

## Problem Statement
Before this feature:
- Any token contract address could be deposited into vaults
- Malicious tokens could drain vault funds
- Worthless or test tokens could be added accidentally
- No control over asset quality

## Solution
Admin maintains a persistent list of approved tokens. All token deposits must pass allowlist validation.

## Implementation Details

### DataKey Variant
```rust
enum DataKey {
    // ...
    AllowedTokens,  // Issue #1118: admin-controlled token allowlist
}
```

### Event Topics
```rust
pub const TOKEN_ALLOWLIST_ADDED_TOPIC: Symbol = symbol_short!("tok_add");
pub const TOKEN_ALLOWLIST_REMOVED_TOPIC: Symbol = symbol_short!("tok_rem");
```

### Error Code
```rust
pub enum ContractError {
    // ...
    TokenNotAllowed = 95,  // Issue #1118: Token not in allowlist
}
```

## Core Functions

### 1. add_allowed_token(env, token)
**Purpose**: Add a token to the allowlist

**Authorization**: Admin-only

**Parameters**:
- `env`: Soroban environment
- `token`: Address of token contract to allow

**Returns**: Result<(), ContractError>

**Behavior**:
1. Verify caller is admin (require_auth)
2. Load current allowlist (empty Vec if none)
3. Check if token already exists (no duplicates)
4. Append token to allowlist
5. Store updated list in persistent storage
6. Emit TOKEN_ALLOWLIST_ADDED_TOPIC event
7. Log audit entry
8. Extend TTL on allowlist key
9. Return Ok(())

**Errors**:
- `NotAdmin` - Caller is not admin
- `Paused` - Contract is paused

**Idempotency**: 
- Adding already-allowed token is a no-op (returns Ok immediately)

### 2. remove_allowed_token(env, token)
**Purpose**: Remove a token from the allowlist

**Authorization**: Admin-only

**Parameters**:
- `env`: Soroban environment
- `token`: Address of token contract to disallow

**Returns**: Result<(), ContractError>

**Behavior**:
1. Verify caller is admin (require_auth)
2. Load current allowlist
3. Search for token in list
4. If found, rebuild list without token
5. Store updated list in persistent storage
6. Emit TOKEN_ALLOWLIST_REMOVED_TOPIC event
7. Log audit entry
8. Extend TTL on allowlist key
9. Return Ok(())

**Errors**:
- `NotAdmin` - Caller is not admin
- `Paused` - Contract is paused

**Idempotency**:
- Removing non-existent token is a no-op (returns Ok)

### 3. get_allowed_tokens(env) -> Vec<Address>
**Purpose**: Retrieve complete allowlist

**Authorization**: Public

**Parameters**:
- `env`: Soroban environment

**Returns**: Vec<Address>

**Behavior**:
1. Load allowlist from persistent storage
2. Return empty Vec if none exist
3. Return complete list otherwise

### 4. is_token_allowed(env, token) -> bool
**Purpose**: Check if single token is allowed

**Authorization**: Public

**Parameters**:
- `env`: Soroban environment
- `token`: Address to check

**Returns**: bool - true if allowed, false otherwise

**Behavior**:
1. Load allowlist from persistent storage
2. Iterate through tokens
3. Return true on first match
4. Return false if no match found

## Integration with deposit_token

When implementing token deposits, validation must be added:

```rust
pub fn deposit_token(
    env: Env,
    vault_id: u64,
    caller: Address,
    token: Address,
    amount: i128,
) -> Result<(), ContractError> {
    // ... existing validation ...
    
    // Check token is allowed
    if !Self::is_token_allowed(&env, &token) {
        return Err(ContractError::TokenNotAllowed);
    }
    
    // ... rest of deposit logic ...
}
```

## Event Emission

### TOKEN_ALLOWLIST_ADDED_TOPIC
**Data**: (admin: Address, token: Address)
**When**: Token successfully added to allowlist

### TOKEN_ALLOWLIST_REMOVED_TOPIC
**Data**: (admin: Address, token: Address)
**When**: Token successfully removed from allowlist

## Audit Trail
All allowlist operations are logged with:
- Action: "add_allowed_token" | "remove_allowed_token"
- Caller: Admin address
- Timestamp: Ledger timestamp
- Token: Address added/removed

## Storage Efficiency

### Persistent Storage
- **Key**: DataKey::AllowedTokens
- **Value**: Vec<Address>
- **Type**: Persistent (long-term storage)
- **TTL Extension**: VAULT_TTL_LEDGERS on each modification

### Access Patterns
- Typical allowlist size: 10-100 tokens
- Read: O(n) for is_token_allowed (acceptable for small lists)
- Write: O(n) for add/remove (acceptable frequency)

## Security Considerations

### 1. Admin Privilege
- Only admin can modify allowlist
- Prevents unauthorized token additions
- Clear authority chain

### 2. Pausable
- Allowlist operations blocked during contract pause
- Prevents allowlist changes during security incidents
- Allows emergency halt of changes

### 3. Duplicate Prevention
- `add_allowed_token` checks for existing token
- Prevents duplicate entries in list
- Maintains clean state

### 4. Immutable Check
- Tokens cannot be modified once added
- Can only be removed (full control)
- No partial modifications

### 5. Audit Trail
- All modifications logged
- Transparent history of allowlist changes
- Forensic capability

## Token Addition Review Process

Adding a new SEP-41 token to the allowlist follows a structured review and approval workflow to protect vault users from malicious, non-standard, or high-risk assets.

### 1. Submission Criteria
To be considered for inclusion in the allowlist, a token proposal must meet the following baseline requirements:
- **SEP-41 Compliance**: The token must strictly implement the Stellar SEP-41 token interface (`allowance`, `approve`, `balance`, `transfer`, `transfer_from`, `burn`, `burn_from`, `decimals`, `name`, `symbol`).
- **Source & Build Transparency**: Custom SEP-41 token contracts must have publicly verifiable source code and reproducible WASM builds; Stellar Asset Contracts (SACs) must reference a verifiable classic Stellar asset and `stellar.toml`.
- **Ecosystem Maturity**: The asset should demonstrate active usage, sufficient liquidity on Stellar, and clear utility for TTL-Legacy vault users.
- **Required Proposal Details**: Submissions must be opened via a GitHub issue or pull request using the `token-management` label and include:
  - Token name, symbol, and decimals
  - Stellar Mainnet (and Testnet, if applicable) contract address
  - Contract type (canonical SAC vs. custom SEP-41 contract)
  - Official project documentation, repository links, and security contact
  - Links to third-party security audits (required for custom SEP-41 contracts)
  - Rationale for adding the token to vault operations

### 2. Security Review Steps
Before a candidate token can advance to voting, security reviewers perform the following checks:
- **Audit & Code Review**: Review third-party audit reports and inspect contract code (for custom SEP-41 implementations) or confirm canonical SAC deployment.
- **Accounting & Behavioral Safety**: Verify the token does not exhibit behaviors that break vault accounting, including:
  - Fee-on-transfer or hidden transfer taxes
  - Rebasing or elastic supply mechanics that mutate balances outside transfers
  - Unexpected panics, unbounded loops, or reentrancy callbacks during `transfer` / `transfer_from`
- **Privilege & Upgradeability Assessment**: Inspect privileged roles (admin, mint, burn, freeze, clawback, pause, and contract upgradeability) to evaluate centralization and key-compromise risks.
- **Testnet Integration Testing**: Simulate `add_allowed_token`, `deposit_token`, and vault withdrawal/recovery flows on Stellar Testnet to confirm end-to-end compatibility.

### 3. On-Chain Verification
Prior to execution, reviewers independently verify the token's on-chain state on Stellar Mainnet:
- **Contract & Bytecode Verification**: Confirm the submitted contract address exists on-chain and that its deployed WASM hash matches the audited, reproducible build (or canonical SAC).
- **SEP-41 Interface & Metadata Check**: Query `name()`, `symbol()`, and `decimals()` via Soroban RPC / Stellar CLI to confirm they match the proposal and behave consistently.
- **Issuer & Asset Flags (for SACs)**: Inspect the underlying Stellar issuer account flags (`AUTH_REQUIRED`, `AUTH_REVOCABLE`, `AUTH_CLAWBACK_ENABLED`, `AUTH_IMMUTABLE`) and verify the issuer's `stellar.toml` domain metadata.
- **Transaction Simulation**: Dry-run `add_allowed_token(env, token)` and `is_token_allowed(env, token)` against the target vault contract to verify gas/footprint bounds and expected state changes.

### 4. Voting & Approval Process
Once security review and on-chain verification pass, the proposal moves to approval:
1. **Review Window**: The verified proposal is opened for a minimum 72-hour review period so maintainers and community stakeholders can raise objections or additional risk findings.
2. **Approval & Quorum**: Approval requires sign-off from designated security reviewers and a majority (or configured multi-sig threshold) of protocol maintainers/admins with no unresolved security blockers.
3. **On-Chain Execution**: Upon approval, the contract admin executes `add_allowed_token(env, token)` on Mainnet, emitting `TOKEN_ALLOWLIST_ADDED_TOPIC` and recording an audit trail entry.
4. **Documentation & Ongoing Monitoring**: The allowlist documentation is updated with the approved token details. Approved tokens remain subject to ongoing monitoring; if a token is compromised, upgraded maliciously, or violates listing criteria, the admin may immediately delist it via `remove_allowed_token(env, token)` (existing vault balances remain unaffected).

## Use Cases

### Adding Trusted Tokens
```
Admin calls: add_allowed_token(USDC_address)
Result: Users can now deposit USDC into vaults
Event: TOKEN_ALLOWLIST_ADDED_TOPIC emitted
```

### Removing Compromised Token
```
Admin calls: remove_allowed_token(compromised_token_address)
Result: Compromised token can no longer be deposited
Existing vaults: Keep their holdings (not affected)
Event: TOKEN_ALLOWLIST_REMOVED_TOPIC emitted
```

### Querying Allowed Assets
```
User calls: get_allowed_tokens()
Result: Vec<Address> of all allowed tokens
Use: UI shows available deposit options
```

### Deposit Validation
```
User calls: deposit_token(vault_id, token, amount)
Contract checks: is_token_allowed(token)?
If false: Returns TokenNotAllowed error
If true: Proceeds with deposit
```

## Testing Strategy

### Allowlist Management Tests
- ✅ Add single token
- ✅ Add multiple tokens
- ✅ Add duplicate token (no-op)
- ✅ Remove existing token
- ✅ Remove non-existent token (no-op)
- ✅ Query empty allowlist
- ✅ Query non-empty allowlist

### Authorization Tests
- ✅ Non-admin cannot add token (NotAdmin error)
- ✅ Non-admin cannot remove token (NotAdmin error)
- ✅ Public can query allowlist

### Paused Contract Tests
- ✅ Add token when paused (Paused error)
- ✅ Remove token when paused (Paused error)
- ✅ Query allowlist when paused (allowed)

### Integration Tests
- ✅ Deposit allowed token (success)
- ✅ Deposit non-allowed token (TokenNotAllowed error)
- ✅ Remove token, then try deposit (TokenNotAllowed error)

### Event Tests
- ✅ TOKEN_ALLOWLIST_ADDED_TOPIC emitted with correct data
- ✅ TOKEN_ALLOWLIST_REMOVED_TOPIC emitted with correct data

## Migration Path

For existing deployments:
1. Deploy contract with allowlist feature
2. Allowlist starts empty (no tokens allowed)
3. Admin adds whitelisted tokens over time
4. Previous vault tokens remain usable (no retroactive enforcement)
5. New deposits must use allowlisted tokens

## Future Enhancements

Potential improvements (not in scope):
- Token metadata caching (name, decimals)
- Token tier system (premium, standard, test)
- Bulk add/remove operations
- Time-locked allowlist changes
- Multi-sig approval for allowlist changes
