# Beneficiary Auction System

## Overview - Issue #527

Allow beneficiaries to bid for larger allocations in a decentralized auction. Vault owners can open a time-limited auction where interested beneficiaries submit bids, and the highest bidder wins a designated allocation percentage.

## Key Features

- **Time-Limited Auctions**: Configurable start/end times with strict enforcement
- **Minimum Bid Requirements**: Set thresholds to filter frivolous bids
- **Transparent Bidding**: All bids are recorded on-chain with timestamps
- **Highest Bidder Wins**: Deterministic winner selection based on bid amount
- **Allocation Flexibility**: Auction-specific allocation percentages (0-100%)
- **Multiple Bids**: Same bidder can place multiple bids; highest replaces previous

## API Reference

### `create_beneficiary_auction`

Initialize a new auction for a vault.

```rust
pub fn create_beneficiary_auction(
    env: Env,
    vault_id: u64,
    caller: Address,                // must be vault owner
    start_time: u64,                // Unix timestamp when bidding starts
    end_time: u64,                  // Unix timestamp when bidding closes
    total_allocation_bps: u32,      // allocation being auctioned (0-10000 BPS)
    minimum_bid: i128,              // minimum bid amount in stroops
) -> Result<(), ContractError>
```

**Constraints:**
- Caller must be the vault owner
- `start_time` must be < `end_time`
- `total_allocation_bps` must be 1-10000 (1-100%)
- `minimum_bid` must be > 0
- Only one active auction per vault
- Vault must exist

**Errors:**
- `NotOwner` - Caller is not the vault owner
- `VaultNotFound` - Vault does not exist
- `AuctionAlreadyExists` - An auction already exists for this vault
- `InvalidAmount` - Minimum bid is <= 0

### `place_auction_bid`

Submit a bid during the auction window.

```rust
pub fn place_auction_bid(
    env: Env,
    vault_id: u64,
    bidder: Address,                // bidder's address (must authorize)
    bid_amount: i128,               // bid in stroops
    desired_allocation_bps: u32,    // desired allocation % (0-10000 BPS)
) -> Result<(), ContractError>
```

**Constraints:**
- Bidder must authorize this transaction
- Auction must exist for this vault
- Current time must be >= `start_time` and < `end_time`
- `bid_amount` must be >= `minimum_bid`
- `desired_allocation_bps` must be 1-10000
- Same bidder can place multiple bids (each recorded separately)

**Errors:**
- `AuctionNotFound` - No auction for this vault
- `AuctionEnded` - Auction window has closed
- `InvalidAmount` - Bid is below minimum_bid
- (Implicit: bidder doesn't authorize = auth failure)

### `finalize_beneficiary_auction`

Close the auction and determine the winner.

```rust
pub fn finalize_beneficiary_auction(env: Env, vault_id: u64) -> Result<(), ContractError>
```

**Constraints:**
- Auction must exist
- Current time must be >= `end_time`
- Can only be called once per auction (idempotent after first call)

**Behavior:**
- If bids exist: Highest bidder (by `bid_amount`) becomes the winner
- If no bids: Auction finalizes with no winner
- Sets auction `finalized = true` and stores winner address

**Errors:**
- `AuctionNotFound` - No auction for this vault
- `AuctionNotEnded` - Auction window is still open

### `get_beneficiary_auction`

Retrieve complete auction details.

```rust
pub fn get_beneficiary_auction(env: Env, vault_id: u64) -> Option<BeneficiaryAuction>
```

Returns auction struct with all bids, timing, and winner info, or `None` if no auction exists.

### `get_beneficiary_auction_bids`

Retrieve all bids for an auction.

```rust
pub fn get_beneficiary_auction_bids(env: Env, vault_id: u64) -> Vec<BeneficiaryAuctionBid>
```

Returns a vector of all bids placed on this auction, or empty vector if no auction.

## Example Usage

### Simple Auction: 50% Allocation To Highest Bidder

```rust
// Owner creates vault and deposits 10,000 XLM
let vault_id = client.create_vault(&owner, &primary_ben, &100u64, &None);
client.deposit(&vault_id, &owner, &10_000_000);

let now = env.ledger().timestamp() as u64;
let start = now + 86_400;         // Auction starts tomorrow
let end = start + 604_800u64;     // Runs for 1 week

// Owner opens auction for 50% allocation (5000 BPS)
client.create_beneficiary_auction(
    &vault_id,
    &owner,
    &start,
    &end,
    &5000u32,        // 50%
    &100_000i128,    // minimum 0.1 XLM
);

// Move into auction window
env.ledger().set_timestamp((start + 1_000) as u32);

// Bidder A bids 0.5 XLM
let ben_a = Address::generate(&env);
client.place_auction_bid(&vault_id, &ben_a, &500_000i128, &5000u32);

// Bidder B bids 1.0 XLM (higher)
let ben_b = Address::generate(&env);
client.place_auction_bid(&vault_id, &ben_b, &1_000_000i128, &5000u32);

// Auction ends
env.ledger().set_timestamp((end + 1) as u32);

// Finalize - Ben B wins with highest bid
client.finalize_beneficiary_auction(&vault_id)?;

let auction = client.get_beneficiary_auction(&vault_id).unwrap();
assert_eq!(auction.winner, Some(ben_b));
assert_eq!(auction.finalized, true);
```

### Multi-Tier Auction: Progressive Allocations

```rust
// Scenario: Owner wants to allocate 80% total, split by bid levels

// First auction: Top 40% for highest bidder
client.create_beneficiary_auction(&vault_id, &owner, &start1, &end1, &4000u32, &500_000i128);

// ... bidding occurs ...
// Winner: Alice (4000 BPS = 40%)

// Later: Second auction for next 40% allocation
client.create_beneficiary_auction(&vault_id, &owner, &start2, &end2, &4000u32, &300_000i128);

// ... new bidding round ...
// Winner: Bob (4000 BPS = 40%)
// Remaining: Charlie gets default primary allocation (20%)
```

### Auction With Dynamic Minimum Bid

```rust
// Start conservative: low minimum to attract bids
client.create_beneficiary_auction(
    &vault_id, &owner, &start, &end,
    &10_000u32,    // 100% (unreserved)
    &10_000i128,   // very low minimum
);

// Bidders can bid aggressively to ensure winning
```

## Data Structures

### BeneficiaryAuction

```rust
pub struct BeneficiaryAuction {
    pub auction_id: u64,              // global auction counter
    pub vault_id: u64,                // associated vault
    pub start_time: u64,              // Unix timestamp: bidding starts
    pub end_time: u64,                // Unix timestamp: bidding closes
    pub total_allocation_bps: u32,    // allocation in BPS (0-10000)
    pub minimum_bid: i128,            // minimum bid in stroops
    pub bids: Vec<BeneficiaryAuctionBid>,  // all bids placed
    pub finalized: bool,              // true after finalization
    pub winner: Option<Address>,      // winning bidder address
}
```

### BeneficiaryAuctionBid

```rust
pub struct BeneficiaryAuctionBid {
    pub auction_id: u64,              // parent auction ID
    pub bidder: Address,              // who placed the bid
    pub bid_amount: i128,             // bid in stroops
    pub desired_allocation_bps: u32,  // requested allocation %
    pub bid_timestamp: u64,           // when bid was placed
    pub accepted: bool,               // reserved for future use
}
```

## Events

| Topic | Data | Emitted when |
|-------|------|--------------|
| `auc_crt` | `(auction_id, start_time, end_time)` | Auction is created |
| `auc_bid` | `(bidder, bid_amount)` | Bid is placed |
| `auc_fin` | `(winner_address)` | Auction is finalized |

## Winner Selection Algorithm

The auction uses a **simple highest-bid mechanism**:

1. After `end_time` passes, any account can call `finalize_beneficiary_auction`
2. If bids exist:
   - Iterate through all bids
   - Select bid with maximum `bid_amount`
   - Store bidder as `winner`
3. If no bids:
   - Auction finalizes with `winner = None`
4. Mark auction as finalized (idempotent)

**Tie-Breaking:** In the unlikely case of equal bids, the first bid placed wins (earlier index in bids vector).

## Auction Timing Guarantees

- **Before start_time**: Bidding is impossible (rejected)
- **At/After start_time**: Bidding is allowed
- **At end_time (exclusive)**: Bidding still allowed (end is exclusive boundary)
- **After end_time**: No new bids accepted; finalization allowed
- **Multiple finalizations**: Calling finalize multiple times is safe (idempotent)

**Example Timing:**
```
start_time = 1000
end_time = 2000

Timestamp 999: ❌ Bid rejected (auction not started)
Timestamp 1000: ✅ Bid accepted (auction started)
Timestamp 1999: ✅ Bid accepted (still < end_time)
Timestamp 2000: ❌ Bid rejected (≥ end_time)
Timestamp 2000: ✅ Finalize allowed (≥ end_time)
```

## Security Considerations

- **No Fund Transfer**: Bidding does NOT transfer funds (bids are just commitments)
- **On-Chain Transparent**: All bids recorded with timestamp audit trail
- **Immutable Record**: Once placed, bids cannot be cancelled or modified
- **Authorization Required**: Each bid must be authorized by the bidder
- **Owner Control**: Only vault owner can create auctions

## Common Patterns

### Sealed Auction Approximation

Since all bids are public, Soroban doesn't support true sealed bids. However, you can:

1. Use off-chain coordination (signal via external server)
2. Place bids with obfuscated allocations
3. Rely on finalization happening after all bids are public

### All-Pay Auction (Modified)

Convert to all-pay by having the contract collect bid amounts:

```
// After finalization:
// - Winner receives allocation
// - All bidders' amounts remain in vault or are donated
// (Requires additional contract logic)
```

### Reverse Auction (Lowest Bid Wins)

Modify winner selection to find minimum `bid_amount`:

```rust
// Find lowest bidder instead of highest
let winner_idx = bids.iter().min_by_key(|bid| bid.bid_amount);
```

## Limitations

- Single auction per vault (must finalize before creating new one)
- No automatic winner distribution (winner must claim separately via vesting)
- No auction cancellation mid-stream
- No bid cancellation or modification
- No proxy bidding or automated increments
- 64-bit auction_id limits auctions to ~18 billion total

## Integration with Vesting

After auction finalization, the vault owner can:

1. Set the winner as a new beneficiary with custom vesting:
   ```rust
   client.set_beneficiary_vesting(
       &vault_id, &owner, &winner,
       &start, &interval, &4u32, &0u64
   );
   ```

2. Combine with main beneficiary in multi-beneficiary splits:
   ```rust
   client.set_beneficiaries(&vault_id, &owner, &vec![
       BeneficiaryEntry { address: winner, bps: 5000, ... },
       BeneficiaryEntry { address: primary, bps: 5000, ... },
   ]);
   ```

## Future Enhancements

- Auction types: Dutch auction, sealed bid, descending price
- Bidding guarantees: escrow deposits, minimum winning bid enforcement
- Partial fills: Support multiple winners based on tier allocations
- Auction extensions: Automatic extension if bid activity near deadline
- Bid modifiers: Reduce bid requirements for qualified beneficiaries
