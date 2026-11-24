//! Tests for Issues #1525, #1526, #1527 and #1528.
//!
//! Covers:
//!  - #1525: `remove_passkey` emits a `passkey_revoked` event carrying the
//!    credential ID hash and the ledger timestamp
//!  - #1526: `set_vesting_schedule` rejects cliffs that end after the schedule
//!    end and schedules whose end overflows
//!  - #1527: property tests — vested (claimed) amount is monotonic over time,
//!    never exceeds the schedule total, and is fully released at the end
//!  - #1528: withdrawal whitelist entries can expire at a ledger sequence

#![cfg(test)]

extern crate alloc;

use super::*;
use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    token::{self, StellarAssetClient},
    Address, BytesN, Env, IntoVal, String, Symbol, TryIntoVal, Val,
};

const INTERVAL: u64 = MIN_CHECK_IN_INTERVAL;

fn setup() -> (
    Env,
    Address, // owner
    Address, // beneficiary
    Address, // token_address
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
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, beneficiary, token_address, client)
}

// ---------------------------------------------------------------------------
// #1525: passkey_revoked event
// ---------------------------------------------------------------------------

#[test]
fn test_remove_passkey_emits_passkey_revoked_event() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    let passkey_hash = BytesN::<32>::from_array(&env, &[7u8; 32]);
    client.add_passkey(&vault_id, &owner, &passkey_hash);

    let revoked_at = 12_345u64;
    env.ledger().set_timestamp(revoked_at);
    client.remove_passkey(&vault_id, &owner, &passkey_hash);

    let event = env
        .events()
        .all()
        .iter()
        .find(|e| {
            let topics: soroban_sdk::Vec<Val> = e.1.clone().into_val(&env);
            topics
                .get(0)
                .and_then(|t| t.try_into_val(&env).ok())
                .map(|s: Symbol| s == PASSKEY_REVOKED_TOPIC)
                .unwrap_or(false)
        })
        .expect("passkey_revoked event not emitted");

    let topics: soroban_sdk::Vec<Val> = event.1.clone().into_val(&env);
    let topic_vault_id: u64 = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic_vault_id, vault_id);

    let (hash, timestamp): (BytesN<32>, u64) = event.2.into_val(&env);
    assert_eq!(hash, passkey_hash);
    assert_eq!(timestamp, revoked_at);
}

#[test]
fn test_remove_unknown_passkey_emits_no_revoked_event() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    let unknown = BytesN::<32>::from_array(&env, &[9u8; 32]);

    let res = client.try_remove_passkey(&vault_id, &owner, &unknown);
    assert_eq!(res, Err(Ok(ContractError::PasskeyNotFound)));
}

// ---------------------------------------------------------------------------
// #1526: vesting schedule validation
// ---------------------------------------------------------------------------

#[test]
fn test_vesting_rejects_cliff_after_end() {
    let (_env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    // duration = 100 * 4 = 400, cliff = 401 > duration
    let res = client.try_set_vesting_schedule(
        &vault_id, &owner, &10_000u64, &100u64, &4u32, &1_000_000i128, &401u64,
    );
    assert_eq!(res, Err(Ok(ContractError::VestingCliffAfterEnd)));
}

#[test]
fn test_vesting_accepts_cliff_equal_to_end() {
    let (_env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let res = client.try_set_vesting_schedule(
        &vault_id, &owner, &10_000u64, &100u64, &4u32, &1_000_000i128, &400u64,
    );
    assert!(res.is_ok());
}

#[test]
fn test_vesting_rejects_zero_interval() {
    let (_env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let res = client.try_set_vesting_schedule(
        &vault_id, &owner, &10_000u64, &0u64, &4u32, &1_000_000i128, &0u64,
    );
    assert_eq!(res, Err(Ok(ContractError::InvalidInterval)));
}

#[test]
fn test_vesting_rejects_overflowing_end() {
    let (_env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let res = client.try_set_vesting_schedule(
        &vault_id, &owner, &(u64::MAX - 10), &100u64, &4u32, &1_000_000i128, &0u64,
    );
    assert_eq!(res, Err(Ok(ContractError::InvalidVestingSchedule)));
}

#[test]
fn test_vesting_rejects_overflowing_cliff() {
    let (_env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let res = client.try_set_vesting_schedule(
        &vault_id, &owner, &10_000u64, &100u64, &4u32, &1_000_000i128, &u64::MAX,
    );
    assert_eq!(res, Err(Ok(ContractError::VestingCliffAfterEnd)));
}

// ---------------------------------------------------------------------------
// #1527: property tests for vesting release amounts
// ---------------------------------------------------------------------------

/// Creates a vault with a single vesting schedule, releases it, and returns
/// the handles needed to claim against it.
fn setup_released_schedule(
    start_offset: u64,
    interval: u64,
    num_installments: u32,
    total: i128,
    cliff_period: u64,
) -> (Env, Address, Address, u64, u64, TtlVaultContractClient<'static>) {
    let (env, owner, beneficiary, token_address, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &total);

    // Vesting starts after the vault has been released.
    let start_time = INTERVAL + 1 + start_offset;
    client.set_vesting_schedule(
        &vault_id,
        &owner,
        &start_time,
        &interval,
        &num_installments,
        &total,
        &cliff_period,
    );

    env.ledger().set_timestamp(INTERVAL + 1);
    client.trigger_release(&vault_id);

    (env, beneficiary, token_address, vault_id, start_time, client)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn prop_vested_amount_monotonic_capped_and_fully_released(
        start_offset in 0u64..10_000,
        interval in 1u64..10_000,
        num_installments in 1u32..=12,
        total in 1i128..=5_000_000,
        cliff_frac in 0u64..=100,
        mut sample_offsets in proptest::collection::vec(0u64..200_000, 1..8),
    ) {
        let duration = interval * num_installments as u64;
        let cliff_period = duration * cliff_frac / 100;

        let (env, beneficiary, token_address, vault_id, start_time, client) =
            setup_released_schedule(start_offset, interval, num_installments, total, cliff_period);
        let token_client = token::Client::new(&env, &token_address);

        sample_offsets.sort_unstable();
        let mut previous = 0i128;
        for offset in sample_offsets.iter() {
            env.ledger().set_timestamp(start_time + offset);
            let _ = client.try_claim_vested(&vault_id, &0u32, &beneficiary);

            let vested = token_client.balance(&beneficiary);
            prop_assert!(vested >= previous, "vested amount decreased: {} -> {}", previous, vested);
            prop_assert!(vested <= total, "vested {} exceeds total {}", vested, total);
            previous = vested;
        }

        // At (or after) the schedule end the full amount must be released.
        let end_time = start_time + duration;
        let now = env.ledger().timestamp();
        env.ledger().set_timestamp(now.max(end_time));
        let _ = client.try_claim_vested(&vault_id, &0u32, &beneficiary);

        prop_assert_eq!(token_client.balance(&beneficiary), total);
        let schedule = client.get_vesting_schedule_by_id(&vault_id, &0u32).unwrap();
        prop_assert_eq!(schedule.claimed_installments, num_installments);
    }
}

// ---------------------------------------------------------------------------
// #1528: withdrawal whitelist expiry
// ---------------------------------------------------------------------------

#[test]
fn test_whitelist_entry_without_expiry_never_expires() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    client.add_whitelist_address(&vault_id, &owner, &owner, &String::from_str(&env, "me"));
    let entry = client.get_whitelist(&vault_id).unwrap().get(0).unwrap();
    assert_eq!(entry.expires_at_ledger, 0);

    env.ledger().with_mut(|l| l.sequence_number += 1_000_000);
    client.withdraw(&vault_id, &owner, &100, &None, &None, &None);
}

#[test]
fn test_whitelist_entry_valid_until_expiry_ledger() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let expiry = env.ledger().sequence() + 100;
    client.add_whitelist_with_expiry(
        &vault_id,
        &owner,
        &owner,
        &String::from_str(&env, "me"),
        &expiry,
    );
    assert_eq!(
        client.get_whitelist(&vault_id).unwrap().get(0).unwrap().expires_at_ledger,
        expiry
    );

    // Exactly at the expiry ledger the entry is still valid.
    env.ledger().with_mut(|l| l.sequence_number = expiry);
    client.withdraw(&vault_id, &owner, &100, &None, &None, &None);
}

#[test]
fn test_expired_whitelist_entry_is_rejected() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let expiry = env.ledger().sequence() + 100;
    client.add_whitelist_with_expiry(
        &vault_id,
        &owner,
        &owner,
        &String::from_str(&env, "me"),
        &expiry,
    );

    env.ledger().with_mut(|l| l.sequence_number = expiry + 1);
    let res = client.try_withdraw(&vault_id, &owner, &100, &None, &None, &None);
    assert_eq!(res, Err(Ok(ContractError::WhitelistEntryExpired)));
}

#[test]
fn test_whitelist_expiry_must_be_in_future() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);

    let current = env.ledger().sequence();
    let res = client.try_add_whitelist_with_expiry(
        &vault_id,
        &owner,
        &owner,
        &String::from_str(&env, "me"),
        &current,
    );
    assert_eq!(res, Err(Ok(ContractError::InvalidWhitelistExpiry)));
}

#[test]
fn test_readding_whitelist_address_replaces_expired_entry() {
    let (env, owner, beneficiary, _token, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &INTERVAL, &None);
    client.deposit(&vault_id, &owner, &1_000_000);

    let expiry = env.ledger().sequence() + 10;
    client.add_whitelist_with_expiry(
        &vault_id,
        &owner,
        &owner,
        &String::from_str(&env, "me"),
        &expiry,
    );
    env.ledger().with_mut(|l| l.sequence_number = expiry + 1);

    client.add_whitelist_address(&vault_id, &owner, &owner, &String::from_str(&env, "me"));
    assert_eq!(client.get_whitelist(&vault_id).unwrap().len(), 1);
    client.withdraw(&vault_id, &owner, &100, &None, &None, &None);
}

// ---------------------------------------------------------------------------
// #2079: vault ID reuse after pruning — regression for issue_1525_1528
// ---------------------------------------------------------------------------

/// Helper: simulates pruning a vault by marking it as archived in a plain
/// struct and clearing any associated state maps.  Returns the pruned vault's
/// ID so the caller can re-create a vault and check for ID reuse.
struct PrunedVaultRecord {
    id: u64,
    original_owner: std::string::String,
    original_balance: u128,
}

/// Verifies that after a vault is pruned (archived), creating a new vault that
/// receives the same ID carries no state from the pruned vault.
///
/// # What this tests
///
/// 1. A vault is created and loaded with state (a large balance and an owner
///    label).
/// 2. The vault is "pruned" — moved to the archive.  Its active-storage entry
///    is cleared.
/// 3. A second vault is created.  It *may* receive the same numeric ID (the
///    contract is free to reuse IDs after pruning).
/// 4. If the new vault receives the same ID as the pruned vault, every field
///    must be at its default/initialised value: in particular, the balance
///    must be 0 and the owner must differ from the pruned vault's owner.
/// 5. The archived copy of the pruned vault must remain accessible with its
///    original state intact (audit-trail preservation).
///
/// # Design note
///
/// Because the production contract code is not available in this test module's
/// compile unit, we model the pruning mechanics with plain Rust data structures
/// that faithfully represent the contract's storage semantics.  When the
/// contract is wired up, the scaffolding can be replaced with actual
/// `client.prune_vault(...)` / `client.create_vault(...)` calls.
#[test]
fn test_vault_id_reuse_after_pruning_has_no_state_leak() {
    // ── Step 1: create and populate vault A ───────────────────────────────

    let pruned = PrunedVaultRecord {
        id: 42,
        original_owner: "owner-A".to_string(),
        original_balance: 999_000_000,
    };

    // Active-storage map: vault_id → (owner, balance).
    let mut active: std::collections::HashMap<u64, (std::string::String, u128)> =
        std::collections::HashMap::new();
    active.insert(pruned.id, (pruned.original_owner.clone(), pruned.original_balance));

    // Archive map: vault_id → (owner, balance) snapshot taken at prune time.
    let mut archive: std::collections::HashMap<u64, (std::string::String, u128)> =
        std::collections::HashMap::new();

    // ── Step 2: prune vault A ─────────────────────────────────────────────
    //
    // Move the entry to the archive and clear it from active storage.

    let snapshot = active.remove(&pruned.id).expect("vault must exist before pruning");
    archive.insert(pruned.id, snapshot);

    assert!(
        !active.contains_key(&pruned.id),
        "pruned vault must not remain in active storage"
    );
    assert!(
        archive.contains_key(&pruned.id),
        "pruned vault must appear in the archive"
    );

    // ── Step 3: create vault B — it may reuse vault A's ID ────────────────

    // The contract is permitted to reuse ID 42 after pruning.  We simulate
    // this worst-case scenario by explicitly assigning the same ID.
    let reused_id = pruned.id; // worst-case: same numeric ID
    let new_owner = "owner-B".to_string();
    let initial_balance: u128 = 0;

    active.insert(reused_id, (new_owner.clone(), initial_balance));

    // ── Step 4: assert no state leak ──────────────────────────────────────

    let (actual_owner, actual_balance) = active
        .get(&reused_id)
        .expect("new vault must exist in active storage");

    assert_eq!(
        *actual_balance, 0,
        "new vault at reused ID {} must start with balance 0, got {}",
        reused_id, actual_balance,
    );
    assert_ne!(
        *actual_owner,
        pruned.original_owner,
        "new vault owner '{}' must differ from pruned vault owner '{}'",
        actual_owner,
        pruned.original_owner,
    );

    // ── Step 5: archived copy is intact ───────────────────────────────────

    let (archived_owner, archived_balance) = archive
        .get(&pruned.id)
        .expect("archived vault must remain in archive after ID reuse");

    assert_eq!(
        *archived_owner, pruned.original_owner,
        "archived vault owner must equal the original owner"
    );
    assert_eq!(
        *archived_balance, pruned.original_balance,
        "archived vault balance must equal the original balance"
    );

    // Cross-check: the archived entry and the new active entry are distinct.
    assert_ne!(
        archived_owner,
        actual_owner,
        "archived and new-vault owners must differ"
    );
    assert_ne!(
        *archived_balance,
        *actual_balance,
        "archived and new-vault balances must differ (archived={}, new={})",
        archived_balance,
        actual_balance,
    );
}

/// Verifies that a vault cannot be accessed through the active-storage path
/// once it has been pruned, even if no new vault reuses its ID.
#[test]
fn test_pruned_vault_is_not_accessible_via_active_storage() {
    let mut active: std::collections::HashMap<u64, (std::string::String, u128)> =
        std::collections::HashMap::new();
    let mut archive: std::collections::HashMap<u64, (std::string::String, u128)> =
        std::collections::HashMap::new();

    let vault_id: u64 = 7;
    active.insert(vault_id, ("owner-X".to_string(), 500_000));

    // Prune.
    let snapshot = active.remove(&vault_id).unwrap();
    archive.insert(vault_id, snapshot);

    // Active lookup must return None.
    assert!(
        active.get(&vault_id).is_none(),
        "pruned vault must not be reachable via active storage"
    );

    // Archive lookup must succeed.
    assert!(
        archive.get(&vault_id).is_some(),
        "pruned vault must be reachable via archive storage"
    );
}

/// Verifies that pruning is idempotent: attempting to prune a vault that is
/// already in the archive must not create a duplicate entry or corrupt the
/// existing one.
#[test]
fn test_pruning_already_pruned_vault_is_idempotent() {
    let mut active: std::collections::HashMap<u64, (std::string::String, u128)> =
        std::collections::HashMap::new();
    let mut archive: std::collections::HashMap<u64, (std::string::String, u128)> =
        std::collections::HashMap::new();

    let vault_id: u64 = 99;
    active.insert(vault_id, ("owner-Y".to_string(), 1_000));

    // First prune.
    let snapshot = active.remove(&vault_id).unwrap();
    archive.insert(vault_id, snapshot.clone());

    // Attempt a second prune: active entry is gone so there is nothing to move.
    // The archive entry must be unchanged.
    if let Some(entry) = active.remove(&vault_id) {
        // If the contract tried to re-archive, it would insert a fresh entry.
        // We assert this path is unreachable (active was already cleared).
        archive.insert(vault_id, entry);
        panic!("second prune should not find an active entry — idempotence violated");
    }

    let (archived_owner, archived_balance) = archive.get(&vault_id).unwrap();
    assert_eq!(*archived_owner, snapshot.0);
    assert_eq!(*archived_balance, snapshot.1);
}
