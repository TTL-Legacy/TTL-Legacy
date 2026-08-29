//! Tests for Issue #1289: Vesting Schedule Support for Released Funds
//!
//! Covers:
//!  - `VestingSchedule` struct fields (cliff, duration, intervals)
//!  - `set_vesting_schedule` / `get_vesting_schedule_by_id` / `get_vesting_schedule_count`
//!  - `claim_vested`: linear vesting (no cliff)
//!  - `claim_vested`: cliff period enforcement
//!  - Multiple sequential installment claims
//!  - Batch claim when multiple installments become available at once
//!  - Final installment absorbs rounding remainder
//!  - Error paths: not released, cliff not reached, nothing to claim, already complete

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{self, StellarAssetClient},
    vec, Address, Env,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimum valid check-in interval (enforced by the contract).
const INTERVAL: u64 = MIN_CHECK_IN_INTERVAL; // 3600 s

/// Setup helper: creates a contract, vault, deposits `deposit_amount`, attaches a
/// vesting schedule with the supplied parameters, and returns everything the
/// tests need. The vault remains **Locked** (trigger_release not called).
fn setup(
    deposit_amount: i128,
    start_time: u64,
    vesting_interval: u64,
    num_installments: u32,
    cliff_period: u64,
) -> (
    Env,
    Address,      // owner
    Address,      // beneficiary
    Address,      // token_address
    u64,          // vault_id
    u32,          // schedule_id (0-indexed)
    TtlVaultContractClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &(deposit_amount * 4));

    let contract = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract);
    client.initialize(&token_address, &admin);
    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &deposit_amount);

    let schedule_id = client
        .set_vesting_schedule(
            &vault_id,
            &owner,
            &start_time,
            &vesting_interval,
            &num_installments,
            &deposit_amount,
            &cliff_period,
        )
        .unwrap();

    (env, owner, beneficiary, token_address, vault_id, schedule_id, client)
}

/// Expire the vault (advance time past check_in_interval) then trigger release
/// using the TTLExpiry condition. This matches the real on-chain flow.
fn expire_and_release(env: &Env, vault_id: u64, owner: &Address, client: &TtlVaultContractClient) {
    // Set TTLExpiry as the release condition
    client
        .set_release_conditions(
            &vault_id,
            owner,
            &vec![env, ReleaseCondition::TTLExpiry],
        )
        .unwrap();
    // Advance time past check_in_interval (INTERVAL seconds from vault creation at t=0)
    env.ledger().set_timestamp(INTERVAL + 1);
    client.trigger_release(&vault_id);
}

// ---------------------------------------------------------------------------
// 1. VestingSchedule struct field verification
// ---------------------------------------------------------------------------

#[test]
fn test_vesting_schedule_fields_stored_correctly() {
    let start_time     = 1_000u64;
    let v_interval     = 86_400u64;  // 1 day
    let installments   = 4u32;
    let cliff_period   = 172_800u64; // 2 days
    let deposit        = 4_000_000i128;

    let (_env, _owner, _ben, _token, vault_id, schedule_id, client) =
        setup(deposit, start_time, v_interval, installments, cliff_period);

    let sched = client
        .get_vesting_schedule_by_id(&vault_id, &schedule_id)
        .expect("schedule must exist after set_vesting_schedule");

    assert_eq!(sched.start_time,           start_time,   "start_time mismatch");
    assert_eq!(sched.interval,             v_interval,   "interval mismatch");
    assert_eq!(sched.num_installments,     installments, "num_installments mismatch");
    assert_eq!(sched.claimed_installments, 0u32,         "claimed_installments must start at 0");
    assert_eq!(sched.total_amount,         deposit,      "total_amount mismatch");
    assert_eq!(sched.cliff_period,         cliff_period, "cliff_period mismatch");
}

// ---------------------------------------------------------------------------
// 2. get_vesting_schedule_count
// ---------------------------------------------------------------------------

#[test]
fn test_get_vesting_schedule_count_zero_for_fresh_vault() {
    let env = Env::default();
    env.mock_all_auths();

    let owner       = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let admin       = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_addr  = env.register_stellar_asset_contract_v2(token_admin).address();
    StellarAssetClient::new(&env, &token_addr).mint(&owner, &1_000_000);

    let contract = env.register_contract(None, TtlVaultContract);
    let client   = TtlVaultContractClient::new(&env, &contract);
    client.initialize(&token_addr, &admin);
    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    assert_eq!(client.get_vesting_schedule_count(&vault_id), 0u32,
        "fresh vault must have 0 schedules");
}

#[test]
fn test_get_vesting_schedule_count_one_after_creation() {
    let (_, _, _, _, vault_id, _, client) =
        setup(4_000i128, 0u64, 100u64, 2u32, 0u64);
    assert_eq!(client.get_vesting_schedule_count(&vault_id), 1u32);
}

#[test]
fn test_get_vesting_schedule_by_id_nonexistent_returns_none() {
    let (_, _, _, _, vault_id, _, client) =
        setup(4_000i128, 0u64, 100u64, 2u32, 0u64);
    let result = client.get_vesting_schedule_by_id(&vault_id, &99u32);
    assert!(result.is_none(), "non-existent schedule must return None");
}

// ---------------------------------------------------------------------------
// 3. Linear vesting — no cliff
// ---------------------------------------------------------------------------

/// First installment transferred after one interval elapses.
#[test]
fn test_claim_vested_first_installment_linear() {
    let v_interval   = 100u64;
    let installments = 4u32;
    let deposit      = 4_000i128;
    // Start vesting well after vault expiry (INTERVAL + 100)
    let start_time   = INTERVAL + 200;

    let (env, owner, beneficiary, token_address, vault_id, schedule_id, client) =
        setup(deposit, start_time, v_interval, installments, 0u64);

    expire_and_release(&env, vault_id, &owner, &client);

    // Advance to just after the first installment window
    env.ledger().set_timestamp(start_time + v_interval + 1);

    let token_client   = token::Client::new(&env, &token_address);
    let before         = token_client.balance(&beneficiary);
    let per_installment = deposit / installments as i128; // 1_000

    let claimed = client.claim_vested(&vault_id, &schedule_id, &beneficiary).unwrap();
    assert_eq!(claimed, per_installment, "first installment should equal 1/4 of total");
    assert_eq!(token_client.balance(&beneficiary), before + per_installment,
        "beneficiary token balance must increase by one installment");
    assert_eq!(client.get_vault(&vault_id).balance, deposit - per_installment,
        "vault balance must decrease by one installment");
}

/// All four installments claimable one after the other.
#[test]
fn test_claim_vested_all_installments_sequentially() {
    let v_interval   = 100u64;
    let installments = 4u32;
    let deposit      = 4_000i128;
    let start_time   = INTERVAL + 200;

    let (env, owner, beneficiary, token_address, vault_id, schedule_id, client) =
        setup(deposit, start_time, v_interval, installments, 0u64);

    expire_and_release(&env, vault_id, &owner, &client);

    let token_client    = token::Client::new(&env, &token_address);
    let per_installment = deposit / installments as i128;

    for i in 1..=installments {
        env.ledger().set_timestamp(start_time + v_interval * i as u64 + 1);
        let claimed  = client.claim_vested(&vault_id, &schedule_id, &beneficiary).unwrap();
        let expected = if i < installments {
            per_installment
        } else {
            // Last installment absorbs the rounding remainder
            deposit - per_installment * (installments as i128 - 1)
        };
        assert_eq!(claimed, expected, "installment {} amount wrong", i);
    }

    assert_eq!(token_client.balance(&beneficiary), deposit,
        "beneficiary must have received all funds");
    assert_eq!(client.get_vault(&vault_id).balance, 0,
        "vault balance must be zero after all installments");
}

/// Multiple installment windows elapse before first claim; all unlocked at once.
#[test]
fn test_claim_vested_batches_multiple_unlocked_installments() {
    let v_interval   = 100u64;
    let installments = 4u32;
    let deposit      = 4_000i128;
    let start_time   = INTERVAL + 200;

    let (env, owner, beneficiary, _token, vault_id, schedule_id, client) =
        setup(deposit, start_time, v_interval, installments, 0u64);

    expire_and_release(&env, vault_id, &owner, &client);

    // Advance past two installment windows without claiming
    env.ledger().set_timestamp(start_time + v_interval * 2 + 1);

    let claimed = client.claim_vested(&vault_id, &schedule_id, &beneficiary).unwrap();
    let per_installment = deposit / installments as i128;
    assert_eq!(claimed, per_installment * 2, "should claim 2 installments at once");

    let sched = client
        .get_vesting_schedule_by_id(&vault_id, &schedule_id)
        .unwrap();
    assert_eq!(sched.claimed_installments, 2u32,
        "claimed_installments must advance to 2");
}

// ---------------------------------------------------------------------------
// 4. Cliff vesting
// ---------------------------------------------------------------------------

/// No installments claimable before cliff + start_time has elapsed.
#[test]
fn test_claim_vested_cliff_blocks_early_claim() {
    let v_interval   = 86_400u64;           // 1 day
    let cliff_period = 7 * 86_400u64;       // 7-day cliff
    let installments = 12u32;
    let deposit      = 12_000_000i128;
    let start_time   = INTERVAL + 100;

    let (env, owner, beneficiary, _token, vault_id, schedule_id, client) =
        setup(deposit, start_time, v_interval, installments, cliff_period);

    expire_and_release(&env, vault_id, &owner, &client);

    // 1 day in — cliff is 7 days, should fail
    env.ledger().set_timestamp(start_time + v_interval);
    let result = client.try_claim_vested(&vault_id, &schedule_id, &beneficiary);
    assert!(result.is_err(), "must not claim before cliff expires");

    // 1 second before cliff — should fail
    env.ledger().set_timestamp(start_time + cliff_period - 1);
    let result = client.try_claim_vested(&vault_id, &schedule_id, &beneficiary);
    assert!(result.is_err(), "must not claim 1 second before cliff");
}

/// Once cliff has elapsed, claim succeeds and returns a positive amount.
#[test]
fn test_claim_vested_cliff_allows_claim_after_expiry() {
    let v_interval   = 86_400u64;
    let cliff_period = 7 * 86_400u64;
    let installments = 12u32;
    let deposit      = 12_000_000i128;
    let start_time   = INTERVAL + 100;

    let (env, owner, beneficiary, token_address, vault_id, schedule_id, client) =
        setup(deposit, start_time, v_interval, installments, cliff_period);

    expire_and_release(&env, vault_id, &owner, &client);

    // Advance past cliff and one additional installment window
    env.ledger().set_timestamp(start_time + cliff_period + v_interval + 1);

    let token_client = token::Client::new(&env, &token_address);
    let before       = token_client.balance(&beneficiary);

    let claimed = client.claim_vested(&vault_id, &schedule_id, &beneficiary).unwrap();
    assert!(claimed > 0, "claimed amount must be positive after cliff: got {}", claimed);
    assert!(token_client.balance(&beneficiary) > before,
        "beneficiary balance must increase after successful claim");
}

/// A zero cliff period means the first installment is claimable immediately at start_time.
#[test]
fn test_claim_vested_zero_cliff_claimable_at_start() {
    let v_interval   = 100u64;
    let installments = 4u32;
    let deposit      = 4_000i128;
    let start_time   = INTERVAL + 200;

    let (env, owner, beneficiary, _token, vault_id, schedule_id, client) =
        setup(deposit, start_time, v_interval, installments, 0u64);

    expire_and_release(&env, vault_id, &owner, &client);

    // Advance to just after start_time (first window, no cliff)
    env.ledger().set_timestamp(start_time + 1);
    let result = client.try_claim_vested(&vault_id, &schedule_id, &beneficiary);
    assert!(result.is_ok(), "zero-cliff: first installment must be claimable at start_time");
}

// ---------------------------------------------------------------------------
// 5. Error paths
// ---------------------------------------------------------------------------

/// Claiming from a vault that is still Locked must fail.
#[test]
fn test_claim_vested_requires_released_vault() {
    let v_interval   = 100u64;
    let start_time   = INTERVAL + 200;

    let (env, _owner, beneficiary, _token, vault_id, schedule_id, client) =
        setup(4_000i128, start_time, v_interval, 4u32, 0u64);

    // Do NOT release the vault
    env.ledger().set_timestamp(start_time + v_interval + 1);
    let result = client.try_claim_vested(&vault_id, &schedule_id, &beneficiary);
    assert!(result.is_err(), "claiming from locked vault must fail");
}

/// Claiming before start_time returns an error (NothingToClaimYet).
#[test]
fn test_claim_vested_before_start_time_returns_error() {
    let v_interval = 100u64;
    let start_time = INTERVAL + 10_000; // start far in the future

    let (env, owner, beneficiary, _token, vault_id, schedule_id, client) =
        setup(4_000i128, start_time, v_interval, 4u32, 0u64);

    expire_and_release(&env, vault_id, &owner, &client);
    // Time is at INTERVAL + 1 (from expire_and_release) — before start_time
    let result = client.try_claim_vested(&vault_id, &schedule_id, &beneficiary);
    assert!(result.is_err(), "claiming before start_time must fail");
}

/// Claiming the same installment window twice returns an error.
#[test]
fn test_claim_vested_double_claim_same_window_fails() {
    let v_interval = 100u64;
    let start_time = INTERVAL + 200;

    let (env, owner, beneficiary, _token, vault_id, schedule_id, client) =
        setup(4_000i128, start_time, v_interval, 4u32, 0u64);

    expire_and_release(&env, vault_id, &owner, &client);
    env.ledger().set_timestamp(start_time + v_interval + 1);

    // First claim succeeds
    client.claim_vested(&vault_id, &schedule_id, &beneficiary).unwrap();
    // Second claim in same window must fail
    let result = client.try_claim_vested(&vault_id, &schedule_id, &beneficiary);
    assert!(result.is_err(), "double claim in same window must fail");
}

/// After all installments are claimed, VestingAlreadyComplete is returned.
#[test]
fn test_claim_vested_all_claimed_returns_complete_error() {
    let v_interval   = 100u64;
    let installments = 2u32;
    let deposit      = 2_000i128;
    let start_time   = INTERVAL + 200;

    let (env, owner, beneficiary, _token, vault_id, schedule_id, client) =
        setup(deposit, start_time, v_interval, installments, 0u64);

    expire_and_release(&env, vault_id, &owner, &client);

    // Advance past all installments and claim everything
    env.ledger().set_timestamp(start_time + v_interval * installments as u64 + 1);
    client.claim_vested(&vault_id, &schedule_id, &beneficiary).unwrap();

    // Any further claim must fail
    let result = client.try_claim_vested(&vault_id, &schedule_id, &beneficiary);
    assert!(result.is_err(), "post-completion claim must fail");
}

// ---------------------------------------------------------------------------
// 6. Rounding: last installment absorbs remainder
// ---------------------------------------------------------------------------

/// 1_000_001 stroops / 3 installments = 333_333 * 2 + 333_335 (last).
#[test]
fn test_claim_vested_last_installment_absorbs_remainder() {
    let v_interval   = 100u64;
    let installments = 3u32;
    let deposit      = 1_000_001i128; // not evenly divisible by 3
    let start_time   = INTERVAL + 200;

    let (env, owner, beneficiary, token_address, vault_id, schedule_id, client) =
        setup(deposit, start_time, v_interval, installments, 0u64);

    expire_and_release(&env, vault_id, &owner, &client);

    let token_client    = token::Client::new(&env, &token_address);
    let per_installment = deposit / installments as i128; // 333_333

    // Installment 1
    env.ledger().set_timestamp(start_time + v_interval + 1);
    let c1 = client.claim_vested(&vault_id, &schedule_id, &beneficiary).unwrap();
    assert_eq!(c1, per_installment, "installment 1 must equal floor(deposit/n)");

    // Installment 2
    env.ledger().set_timestamp(start_time + v_interval * 2 + 1);
    let c2 = client.claim_vested(&vault_id, &schedule_id, &beneficiary).unwrap();
    assert_eq!(c2, per_installment, "installment 2 must equal floor(deposit/n)");

    // Installment 3 (last) — must absorb the 2-stroop remainder
    env.ledger().set_timestamp(start_time + v_interval * 3 + 1);
    let c3 = client.claim_vested(&vault_id, &schedule_id, &beneficiary).unwrap();
    let expected_last = deposit - per_installment * 2; // 1_000_001 - 666_666 = 333_335
    assert_eq!(c3, expected_last, "last installment must absorb the remainder");

    // Total must equal full deposit
    assert_eq!(c1 + c2 + c3, deposit, "sum of all installments must equal deposit");
    assert_eq!(token_client.balance(&beneficiary), deposit);
    assert_eq!(client.get_vault(&vault_id).balance, 0);
}
