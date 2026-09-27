# Token Management Features

This document describes the token management features implemented in TTL-Legacy, including token conversion, staking, yield distribution, lending, collateral, hedging, and rebalancing.

## Overview

TTL-Legacy now supports advanced token management capabilities that allow vault owners to:

1. **Convert tokens** before release (Issue #581)
2. **Validate token whitelisting** in batch operations (Issue #582)
3. **Stake tokens** for yield while locked (Issue #583)
4. **Distribute or reinvest yield** (Issue #584)
5. **Lend vault tokens** for interest income (Issue #585)
6. **Use tokens as collateral** for loans (Issue #586)
7. **Hedge token price risk** using derivatives (Issue #587)
8. **Rebalance multi-token portfolios** based on target weights (Issue #588)

## Fee Calculation

This section explains how various fees are calculated in TTL-Legacy token management operations, enabling users to predict transaction costs and understand fee distribution.

### Swap Fees

Swap fees are charged when converting one token to another.

**Formula:**
```
swap_fee = (amount * swap_fee_bps) / 10000
amount_after_fee = amount - swap_fee
```

**Parameters:**
- `amount`: Total input token amount
- `swap_fee_bps`: Basis points charged by the protocol (default: 30 bps = 0.3%)
- `amount_after_fee`: Actual amount used for conversion

**Example:**
```
Input amount: 1,000 USDC
Swap fee rate: 30 bps (0.3%)
Swap fee = (1,000 * 30) / 10000 = 3 USDC
Amount converted = 1,000 - 3 = 997 USDC
```

**Fee Distribution:**
- 70% goes to protocol treasury (2.1 USDC)
- 30% goes to the vault as a rebate (0.9 USDC)

### Conversion Fees

Conversion fees apply when transferring between different token types during withdrawal or release.

**Formula:**
```
conversion_fee = (amount * conversion_fee_bps) / 10000
amount_received = amount - conversion_fee
```

**Parameters:**
- `amount`: Total token amount to convert
- `conversion_fee_bps`: Basis points charged (default: 15 bps = 0.15%)
- `from_token`: Source token
- `to_token`: Destination token

**Example:**
```
Original vault amount: 500 XLM
Converting to USDC
Conversion fee rate: 15 bps (0.15%)
Conversion fee = (500 * 15) / 10000 = 0.75 XLM
Amount after fee = 500 - 0.75 = 499.25 XLM (converted)
```

**Fee Distribution:**
- 60% goes to protocol treasury (0.45 XLM)
- 40% goes to the originating token pool (0.30 XLM)

### Staking Fees

Staking fees are charged when enabling or managing staked positions.

**Formula:**
```
entry_fee = (amount * staking_entry_fee_bps) / 10000
management_fee = (staked_amount * annual_management_fee_bps / 365) / 10000
```

**Parameters:**
- `amount`: Amount being staked
- `staking_entry_fee_bps`: Entry fee in basis points (default: 25 bps = 0.25%)
- `annual_management_fee_bps`: Annual management fee (default: 50 bps = 0.5% per year)

**Example:**
```
Staking 10,000 XLM

Entry fee = (10,000 * 25) / 10000 = 2.5 XLM
Amount staked = 10,000 - 2.5 = 9,997.5 XLM

Daily management fee = (9,997.5 * 50 / 365) / 10000 = 0.137 XLM/day
Annual management fee = 50 XLM (approximately)
```

**Fee Distribution:**
- Entry fee: 100% to protocol treasury
- Management fee: 80% to protocol, 20% to vault operator

### Yield Distribution Fees

When yield is distributed or reinvested, fees may apply based on the distribution method.

**Formula:**
```
yield_accrued = (staked_amount * annual_yield_bps * days_elapsed) / (10000 * 365)
distribution_fee = (yield_accrued * yield_fee_bps) / 10000
yield_after_fee = yield_accrued - distribution_fee
```

**Parameters:**
- `annual_yield_bps`: Annual yield rate (varies by staking pool, e.g., 500 bps = 5% APY)
- `days_elapsed`: Number of days since staking or last distribution
- `yield_fee_bps`: Fee on distributed yield (default: 10 bps = 0.1%)

**Example:**
```
Staked amount: 10,000 XLM at 5% APY
Days elapsed: 365 (1 year)

Yield accrued = (10,000 * 500 * 365) / (10000 * 365) = 500 XLM
Yield fee = (500 * 10) / 10000 = 0.5 XLM
Yield distributed = 500 - 0.5 = 499.5 XLM

Options:
1. Distribute to beneficiary: 499.5 XLM sent
2. Reinvest: 499.5 XLM added to staked amount
3. Split (70/30): 349.65 XLM to beneficiary, 149.85 XLM reinvested
```

**Fee Distribution:**
- 100% of yield fees go to protocol treasury

### Lending Fees

Lending fees include entry fees and origination fees charged by the protocol.

**Formula:**
```
entry_fee = (loan_amount * lending_entry_fee_bps) / 10000
origination_fee = (loan_amount * origination_fee_bps) / 10000
total_fees = entry_fee + origination_fee
net_loan_amount = loan_amount - total_fees
```

**Parameters:**
- `loan_amount`: Amount being lent out
- `lending_entry_fee_bps`: Entry fee (default: 50 bps = 0.5%)
- `origination_fee_bps`: Origination fee (default: 25 bps = 0.25%)
- `interest_rate_bps`: Annual interest rate (e.g., 500 bps = 5%)

**Example:**
```
Lending 5,000 XLM at 5% annual interest for 180 days

Entry fee = (5,000 * 50) / 10000 = 2.5 XLM
Origination fee = (5,000 * 25) / 10000 = 1.25 XLM
Total fees = 3.75 XLM
Net loan to borrower = 5,000 - 3.75 = 4,996.25 XLM

Interest accrued = (5,000 * 500 * 180) / (10000 * 365) = 123.29 XLM
Total received after loan term = 5,000 + 123.29 = 5,123.29 XLM
```

**Fee Distribution:**
- Entry fee: 100% to protocol treasury
- Origination fee: 100% to protocol treasury
- Interest: 100% to vault owner

### Collateral Fees

Collateral fees apply when using vault tokens as collateral for external loans.

**Formula:**
```
collateral_fee = (collateral_amount * collateral_fee_bps) / 10000
```

**Parameters:**
- `collateral_amount`: Amount used as collateral
- `collateral_fee_bps`: Annual collateral management fee (default: 30 bps = 0.3%)

**Example:**
```
Using 2,000 XLM as collateral for a loan

Annual collateral fee = (2,000 * 30) / 10000 = 0.6 XLM per year
Monthly fee = 0.6 / 12 = 0.05 XLM per month
```

**Fee Distribution:**
- 100% to protocol treasury

### Hedging Fees

Hedging fees cover the cost of maintaining derivative positions.

**Formula:**
```
hedge_fee = (notional_amount * annual_hedge_fee_bps * days_active) / (10000 * 365)
```

**Parameters:**
- `notional_amount`: Notional value of the hedge
- `annual_hedge_fee_bps`: Annual fee rate (default: 100 bps = 1%)
- `days_active`: Number of days the hedge has been active

**Example:**
```
Hedging 1,000 XLM (notional: $200 at $0.20/XLM) for 90 days
Annual hedge fee rate: 1% (100 bps)

Hedge fee = (1,000 * 100 * 90) / (10000 * 365) = 2.47 XLM
```

**Fee Distribution:**
- 80% to protocol treasury (1.98 XLM)
- 20% to hedge provider/liquidity pool (0.49 XLM)

### Rebalancing Fees

Rebalancing fees apply when the portfolio is rebalanced to target weights.

**Formula:**
```
rebalance_fee = (total_portfolio_value * rebalance_fee_bps) / 10000
```

**Parameters:**
- `total_portfolio_value`: Sum of all token values in the vault
- `rebalance_fee_bps`: Fee per rebalance event (default: 40 bps = 0.4%)

**Example:**
```
Portfolio value: 
- 1,000 XLM ($200)
- 1,000 USDC ($1,000)
- Total: $1,200

Rebalancing fee = ($1,200 * 40) / 10000 = $4.80 (or ~24 XLM)
```

**Fee Distribution:**
- 100% to protocol treasury

### Fee Summary Table

| Operation | Typical Fee | Distribution | Notes |
|-----------|------------|--------------|-------|
| Swap | 30 bps | 70% treasury, 30% vault | Applied on conversion amount |
| Conversion | 15 bps | 60% treasury, 40% pool | Applied on output amount |
| Staking Entry | 25 bps | 100% treasury | One-time at staking start |
| Staking Management | 50 bps/year | 80% treasury, 20% operator | Daily deduction |
| Yield Distribution | 10 bps | 100% treasury | Applied to yield only |
| Lending Entry | 50 bps | 100% treasury | One-time upfront |
| Lending Origination | 25 bps | 100% treasury | One-time upfront |
| Collateral | 30 bps/year | 100% treasury | Monthly deduction |
| Hedging | 100 bps/year | 80% treasury, 20% provider | Daily deduction |
| Rebalancing | 40 bps | 100% treasury | Per rebalance event |

### Gas Fee Considerations

Smart contract operations incur Stellar network fees in addition to TTL-Legacy fees.

**Typical Gas Costs (XLM):**
- Simple transfer: 0.00001 XLM (1 stroops)
- Token contract interaction: 0.0001 - 0.001 XLM (10-100 stroops)
- Complex multi-step operation: 0.001 - 0.01 XLM (100-1000 stroops)

**Total Cost Example:**
```
Staking 10,000 XLM:
1. Staking entry fee: 2.5 XLM
2. Network gas: ~0.0005 XLM
3. Total: ~2.5005 XLM

Effective cost: 0.025005% (minimal additional impact)
```

### Fee Adjustment Strategy

Protocol fees may be adjusted by governance. Upcoming changes include:

- Dynamic fees based on network congestion
- Tiered fees for high-volume users
- Loyalty discounts for long-term vault holders
- Fee reductions during low-activity periods

**Monitoring Fee Changes:**
Subscribe to TTL-Legacy notifications for protocol updates affecting fees.

## Issue #581: Token Conversion

### Purpose

Allow vault owners to convert their vault tokens to different tokens before the vault is released to the beneficiary. This is useful for:

- Converting to stablecoins before release
- Exchanging to preferred tokens
- Hedging against price volatility

### API

#### Enable Token Conversion

```rust
pub fn enable_token_conversion(
    env: Env,
    vault_id: u64,
    from_token: Address,
    to_token: Address,
    conversion_rate: i128,
)
```

**Parameters:**
- `vault_id`: The vault ID
- `from_token`: Source token address (must be whitelisted)
- `to_token`: Target token address (must be whitelisted)
- `conversion_rate`: Conversion rate in basis points (10000 = 1:1)

**Requirements:**
- Caller must be the vault owner
- Both tokens must be whitelisted
- Conversion rate must be positive

**Events:**
- `TOKEN_CONVERSION_TOPIC`: Emitted when conversion is enabled

#### Get Token Conversion

```rust
pub fn get_token_conversion(env: Env, vault_id: u64) -> Option<TokenConversion>
```

Returns the token conversion configuration for a vault, or `None` if not configured.

### Example Usage

```rust
// Enable conversion from XLM to USDC at 1:1 rate
client.enable_token_conversion(
    &vault_id,
    &xlm_token,
    &usdc_token,
    &10000i128, // 1:1 rate
);

// Retrieve conversion config
if let Some(conversion) = client.get_token_conversion(&vault_id) {
    println!("Converting {} to {} at rate {}", 
        conversion.from_token, 
        conversion.to_token, 
        conversion.conversion_rate);
}
```

## Wrapped Token Support for Cross-Chain Compatibility

### Purpose

Allow vault owners to use wrapped tokens that represent a canonical token from another chain or bridge.
Wrapped tokens are accepted whenever their registered canonical token is whitelisted.

### API

#### Register Wrapped Token

```rust
pub fn register_wrapped_token(
    env: Env,
    wrapped_token_address: Address,
    canonical_token_address: Address,
)
```

### Example Usage

```rust
client.register_wrapped_token(&wrapped_token, &xlm_token);
```

Wrapped token registration makes it possible to create vaults and deposit with the wrapped asset while still enforcing the canonical token whitelist.

## Issue #582: Token Whitelisting Validation

### Purpose

Ensure that only whitelisted tokens can be deposited into vaults through batch operations. This prevents accidental or malicious use of non-approved tokens.

### Implementation

The `batch_deposit` function now validates that each vault's token is whitelisted before processing deposits:

```rust
pub fn batch_deposit(env: Env, from: Address, deposits: Vec<(u64, i128)>) {
    // ... validation ...
    
    // Issue #582: Validate token whitelist
    Self::assert_token_whitelisted(&env, &vault.token_address);
    
    // ... process deposit ...
    
    // Emit token whitelist validation event
    env.events().publish(
        (TOKEN_WHITELIST_VALIDATED_TOPIC, vault_id),
        (&vault.token_address, amount),
    );
}
```

### Validation Rules

1. Default XLM token is always whitelisted
2. Custom tokens must be explicitly whitelisted by admin
3. Validation occurs for each vault in the batch before any transfers
4. If any vault uses a non-whitelisted token, the entire batch is rejected

### Events

- `TOKEN_WHITELIST_VALIDATED_TOPIC`: Emitted for each successfully validated deposit

## Issue #583: Token Staking

### Purpose

Allow vault owners to stake their vault tokens in external staking pools to earn yield while the vault is locked. This enables passive income generation during the vault's active period.

### API

#### Enable Token Staking

```rust
pub fn enable_token_staking(
    env: Env,
    vault_id: u64,
    staking_pool: Address,
    annual_yield_bps: u32,
)
```

**Parameters:**
- `vault_id`: The vault ID
- `staking_pool`: Address of the staking pool contract
- `annual_yield_bps`: Annual yield in basis points (e.g., 500 = 5%)

**Requirements:**
- Caller must be the vault owner
- Annual yield must be between 0 and 10000 basis points

**Events:**
- `TOKEN_STAKING_TOPIC`: Emitted when staking is enabled

#### Disable Token Staking

```rust
pub fn disable_token_staking(env: Env, vault_id: u64)
```

Disables staking for a vault. The vault owner can call this to stop earning yield.

**Events:**
- `TOKEN_UNSTAKING_TOPIC`: Emitted when staking is disabled

#### Get Token Staking

```rust
pub fn get_token_staking(env: Env, vault_id: u64) -> Option<TokenStaking>
```

Returns the staking configuration for a vault.

### Example Usage

```rust
// Enable staking with 5% annual yield
client.enable_token_staking(
    &vault_id,
    &staking_pool_address,
    &500u32, // 5% APY
);

// Check staking status
if let Some(staking) = client.get_token_staking(&vault_id) {
    println!("Staking {} tokens at {}% APY",
        staking.staked_amount,
        staking.annual_yield_bps as f64 / 100.0);
}

// Disable staking
client.disable_token_staking(&vault_id);
```

## Issue #584: Token Yield Distribution

### Purpose

Configure how staking yield is distributed or reinvested. Vault owners can choose to:

1. **Distribute to Beneficiary**: Send all yield to the beneficiary
2. **Reinvest**: Automatically reinvest yield back into the vault
3. **Split**: Distribute a percentage to beneficiary and reinvest the rest

### API

#### Set Yield Distribution

```rust
pub fn set_yield_distribution(
    env: Env,
    vault_id: u64,
    mode: YieldDistributionMode,
)
```

**Parameters:**
- `vault_id`: The vault ID
- `mode`: The distribution mode (see below)

**Yield Distribution Modes:**

```rust
pub enum YieldDistributionMode {
    /// Distribute all yield to beneficiary
    DistributeToBeneficiary,
    
    /// Reinvest all yield back into vault
    Reinvest,
    
    /// Split yield: beneficiary_bps to beneficiary, rest reinvested
    Split(u32), // basis points for beneficiary
}
```

**Requirements:**
- Caller must be the vault owner
- Vault must have staking enabled

**Events:**
- `YIELD_DISTRIBUTED_TOPIC`: Emitted when yield is distributed

#### Get Yield Distribution

```rust
pub fn get_yield_distribution(env: Env, vault_id: u64) -> Option<YieldDistributionConfig>
```

Returns the yield distribution configuration for a vault.

#### Distribute Yield

```rust
pub fn distribute_yield(env: Env, vault_id: u64)
```

Calculates accumulated yield and distributes it according to the configured mode.

**Yield Calculation:**

```
yield = (staked_amount × annual_yield_bps × time_elapsed) / (10000 × 365 × 86400)
```

**Events:**
- `YIELD_DISTRIBUTED_TOPIC`: Emitted when yield is sent to beneficiary
- `YIELD_REINVESTED_TOPIC`: Emitted when yield is reinvested

### Example Usage

```rust
// Distribute all yield to beneficiary
client.set_yield_distribution(
    &vault_id,
    &YieldDistributionMode::DistributeToBeneficiary,
);

// Or reinvest all yield
client.set_yield_distribution(
    &vault_id,
    &YieldDistributionMode::Reinvest,
);

// Or split 70% to beneficiary, 30% reinvest
client.set_yield_distribution(
    &vault_id,
    &YieldDistributionMode::Split(7000u32),
);

// Distribute accumulated yield
client.distribute_yield(&vault_id);

// Check distribution stats
if let Some(config) = client.get_yield_distribution(&vault_id) {
    println!("Total distributed: {}", config.total_distributed);
    println!("Total reinvested: {}", config.total_reinvested);
}
```

## Integration Example

Here's a complete example showing how to use all token management features together:

```rust
// 1. Create a vault
let vault_id = client.create_vault(&owner, &beneficiary, &86400u64, &None);

// 2. Deposit funds
client.deposit(&vault_id, &owner, &1_000_000i128);

// 3. Enable staking with 5% APY
client.enable_token_staking(&vault_id, &staking_pool, &500u32);

// 4. Set yield distribution (70% to beneficiary, 30% reinvest)
client.set_yield_distribution(
    &vault_id,
    &YieldDistributionMode::Split(7000u32),
);

// 5. Enable token conversion (optional)
client.enable_token_conversion(
    &vault_id,
    &xlm_token,
    &usdc_token,
    &10000i128,
);

// 6. After some time, distribute yield
client.distribute_yield(&vault_id);

// 7. Check final state
let config = client.get_yield_distribution(&vault_id).unwrap();
println!("Distributed to beneficiary: {}", config.total_distributed);
println!("Reinvested: {}", config.total_reinvested);
```

## Issue #585: Token Lending

### Purpose

Allow vault owners to lend vault tokens to a borrower and earn interest income.

### API

#### Enable Token Lending

```rust
pub fn enable_token_lending(
    env: Env,
    vault_id: u64,
    caller: Address,
    borrower: Address,
    amount: i128,
    interest_rate_bps: u32,
    duration_seconds: u64,
) -> Result<(), ContractError>
```

**Parameters:**
- `vault_id`: The vault ID
- `caller`: Must be the vault owner
- `borrower`: Address of the borrower
- `amount`: Amount to lend (must be ≤ vault balance)
- `interest_rate_bps`: Annual interest rate in basis points (e.g., 500 = 5%)
- `duration_seconds`: Loan duration in seconds

**Events:**
- `TOKEN_LENDING_TOPIC`: Emitted when lending is enabled

#### Repay Token Loan

```rust
pub fn repay_token_loan(env: Env, vault_id: u64, caller: Address) -> Result<i128, ContractError>
```

Returns the accrued interest earned.

**Events:**
- `TOKEN_LEND_REPAY_TOPIC`: Emitted on repayment

#### Get Token Lending

```rust
pub fn get_token_lending(env: Env, vault_id: u64) -> Option<TokenLending>
```

## Issue #586: Token Collateral

### Purpose

Allow vault owners to use vault tokens as collateral for an external loan.

### API

#### Set Token Collateral

```rust
pub fn set_token_collateral(
    env: Env,
    vault_id: u64,
    caller: Address,
    collateral_amount: i128,
    loan_amount: i128,
    collateral_ratio_bps: u32,
) -> Result<(), ContractError>
```

**Parameters:**
- `collateral_ratio_bps`: Required collateral ratio ≥ 10000 (100%)

**Events:**
- `TOKEN_COLLATERAL_TOPIC`: Emitted when collateral is set

#### Release Token Collateral

```rust
pub fn release_token_collateral(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError>
```

**Events:**
- `TOKEN_COLLAT_RLSD_TOPIC`: Emitted when collateral is released

#### Get Token Collateral

```rust
pub fn get_token_collateral(env: Env, vault_id: u64) -> Option<TokenCollateral>
```

## Issue #587: Token Hedging

### Purpose

Allow vault owners to hedge token price risk using a derivative position.

### API

#### Enable Token Hedge

```rust
pub fn enable_token_hedge(
    env: Env,
    vault_id: u64,
    caller: Address,
    hedge_token: Address,
    notional_amount: i128,
    strike_price_bps: u32,
    expiry: u64,
) -> Result<(), ContractError>
```

**Events:**
- `TOKEN_HEDGE_TOPIC`: Emitted when hedge is enabled

#### Close Token Hedge

```rust
pub fn close_token_hedge(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError>
```

**Events:**
- `TOKEN_HEDGE_CLOSE_TOPIC`: Emitted when hedge is closed

#### Get Token Hedge

```rust
pub fn get_token_hedge(env: Env, vault_id: u64) -> Option<TokenHedge>
```

## Issue #588: Token Rebalancing

### Purpose

Automatically rebalance a multi-token vault portfolio based on configured target weights.
This feature allows vault owners to maintain a diversified portfolio that matches their intended asset allocation strategy.

### Target Weights and Rebalancing Thresholds

#### Target Weights

Target weights define the desired allocation percentage for each token in the portfolio.

**Definition:**
- Each token has a `target_bps` (basis points) value representing its desired allocation
- All `target_bps` values must sum to exactly 10000 (100%)
- Valid range per token: 0 to 10000 basis points

**Example Portfolio:**
```
Portfolio with 3 tokens:
- XLM: 5000 bps = 50%
- USDC: 3000 bps = 30%
- ETH: 2000 bps = 20%
Total: 10000 bps = 100% ✓

Portfolio value: $10,000
- XLM holdings: $5,000 (50%)
- USDC holdings: $3,000 (30%)
- ETH holdings: $2,000 (20%)
```

#### Rebalancing Threshold

The rebalance threshold defines when a portfolio is considered "out of balance" and needs rebalancing.

**Definition:**
- `rebalance_threshold_bps`: Drift tolerance in basis points
- Rebalancing is triggered when any token's actual allocation drifts beyond the threshold from its target
- Example: threshold of 500 bps (5%) means rebalance if actual allocation differs from target by 5% or more

**When to Rebalance:**
```
Portfolio with target allocations:
- XLM target: 50% ± 5%
- USDC target: 30% ± 5%
- ETH target: 20% ± 5%

Rebalance triggered when:
- XLM drops below 45% or rises above 55%
- USDC drops below 25% or rises above 35%
- ETH drops below 15% or rises above 25%
```

**Typical Thresholds:**
- Conservative (1-3%): Frequent rebalancing, lower drift, higher fees
- Moderate (3-5%): Balanced approach, suitable for most portfolios
- Aggressive (5-10%): Infrequent rebalancing, higher drift tolerance, lower fees

### API

#### Set Token Rebalance

```rust
pub fn set_token_rebalance(
    env: Env,
    vault_id: u64,
    caller: Address,
    target_weights: Vec<TokenWeight>,
    rebalance_threshold_bps: u32,
) -> Result<(), ContractError>
```

**Parameters:**
- `vault_id`: The vault ID to rebalance
- `caller`: Must be the vault owner
- `target_weights`: Vector of token weights with target allocations
  ```rust
  pub struct TokenWeight {
      pub token_address: Address,
      pub target_bps: u32,  // 0 to 10000 (basis points)
  }
  ```
- `rebalance_threshold_bps`: Drift tolerance in basis points (e.g., 500 = 5%)

**Validation Rules:**
- Sum of all `target_bps` must equal exactly 10000
- All `target_bps` values must be non-negative
- `rebalance_threshold_bps` must be between 0 and 10000
- Caller must be the vault owner
- Vault must support multi-token holdings

**Events:**
- `TOKEN_REBALANCE_TOPIC`: Emitted when rebalance config is set

**Errors:**
- `InvalidTargetWeights`: Weights don't sum to 10000
- `InvalidThreshold`: Threshold is out of range
- `Unauthorized`: Caller is not the vault owner
- `VaultNotFound`: Vault does not exist

#### Trigger Rebalance

```rust
pub fn trigger_rebalance(env: Env, vault_id: u64) -> Result<(), ContractError>
```

Manually triggers a rebalance of the vault portfolio to match target weights.

**When to Use:**
- After significant market movements
- When portfolio has drifted beyond threshold
- Periodically (e.g., quarterly or annually)
- When vault owner wants to manually align allocations

**Rebalancing Process:**
1. Calculate current allocation for each token (actual_bps)
2. Compare with target allocation
3. Swap tokens to bring allocations within threshold
4. Deduct rebalancing fees (typically 40 bps)
5. Emit `TOKEN_REBALANCED_TOPIC` event

**Events:**
- `TOKEN_REBALANCED_TOPIC`: Emitted after rebalance completes

**Fees:**
- Standard rebalancing fee: 40 bps (0.4%) of portfolio value

#### Get Token Rebalance

```rust
pub fn get_token_rebalance(env: Env, vault_id: u64) -> Option<TokenRebalanceConfig>
```

Retrieves the current rebalancing configuration and status.

**Returns:**
```rust
pub struct TokenRebalanceConfig {
    pub vault_id: u64,
    pub target_weights: Vec<TokenWeight>,
    pub rebalance_threshold_bps: u32,
    pub last_rebalanced_at: u64,  // Ledger sequence number
    pub total_rebalances: u32,     // Historical count
}
```

### Example Invocations

#### Example 1: Set Up a Balanced 60/40 Portfolio

```rust
// Create a 60/40 XLM/USDC portfolio with 3% rebalance threshold
let target_weights = vec![
    TokenWeight {
        token_address: xlm_token.clone(),
        target_bps: 6000,  // 60%
    },
    TokenWeight {
        token_address: usdc_token.clone(),
        target_bps: 4000,  // 40%
    },
];

client.set_token_rebalance(
    &vault_id,
    &owner,
    &target_weights,
    &300u32,  // 3% threshold
)?;

println!("Portfolio set up: 60% XLM, 40% USDC");
println!("Rebalance threshold: 3%");
```

#### Example 2: Diversified Multi-Asset Portfolio

```rust
// Create a diversified portfolio
let target_weights = vec![
    TokenWeight {
        token_address: xlm_token.clone(),
        target_bps: 5000,  // 50%
    },
    TokenWeight {
        token_address: usdc_token.clone(),
        target_bps: 2500,  // 25%
    },
    TokenWeight {
        token_address: eth_token.clone(),
        target_bps: 1500,  // 15%
    },
    TokenWeight {
        token_address: btc_token.clone(),
        target_bps: 1000,  // 10%
    },
];

client.set_token_rebalance(
    &vault_id,
    &owner,
    &target_weights,
    &500u32,  // 5% threshold
)?;

// Later, trigger a rebalance if markets have shifted
let rebalanced = client.trigger_rebalance(&vault_id)?;
println!("Portfolio rebalanced to target weights");

// Check current rebalance config
if let Some(config) = client.get_token_rebalance(&vault_id) {
    println!("Total rebalances performed: {}", config.total_rebalances);
    println!("Last rebalanced at ledger: {}", config.last_rebalanced_at);
}
```

#### Example 3: Conservative Portfolio with Low Drift Tolerance

```rust
// Conservative 80/20 portfolio with 1% rebalance threshold
// This maintains tight allocation control
let target_weights = vec![
    TokenWeight {
        token_address: usdc_token.clone(),
        target_bps: 8000,  // 80% stablecoins
    },
    TokenWeight {
        token_address: xlm_token.clone(),
        target_bps: 2000,  // 20% native asset
    },
];

client.set_token_rebalance(
    &vault_id,
    &owner,
    &target_weights,
    &100u32,  // 1% tight threshold
)?;

println!("Conservative portfolio: 80% USDC, 20% XLM");
println!("Rebalance threshold: 1% (tight control)");
```

### Cost Analysis

**Rebalancing Costs:**

```
Portfolio value: $10,000
Rebalance threshold: 5%
Rebalancing fee rate: 40 bps

Cost per rebalance: ($10,000 × 40) / 10000 = $4.00
Annual cost (monthly rebalance): $4.00 × 12 = $48.00
Annual cost (quarterly rebalance): $4.00 × 4 = $16.00
```

**Break-even Analysis:**
```
If portfolio rebalance saves 2% in value drift per year:
Value saved: $10,000 × 0.02 = $200/year
Cost for quarterly rebalance: $16/year
Net benefit: $200 - $16 = $184/year (92% ROI)
```

### Best Practices

1. **Set Realistic Thresholds**: Balance between maintaining allocations and minimizing fees
2. **Monitor Periodically**: Check drift and rebalance when approaching threshold
3. **Coordinate with Market Conditions**: Avoid rebalancing during high volatility
4. **Start Conservative**: Use higher thresholds (5-10%) to learn the feature
5. **Review Annually**: Adjust target weights and thresholds based on market changes

## Security Considerations

1. **Token Whitelisting**: Only whitelisted tokens can be used in vaults
2. **Owner Authorization**: Only vault owners can configure staking, lending, collateral, hedging, and rebalancing
3. **Yield Calculation**: Yield is calculated based on time elapsed and annual rate
4. **Atomic Operations**: Batch deposits validate all items before any transfers
5. **Event Tracking**: All operations emit events for on-chain audit trails
6. **Collateral Ratio**: Collateral ratio must be ≥ 100% to prevent under-collateralised loans
7. **Balance Checks**: Lending and collateral operations verify sufficient vault balance
