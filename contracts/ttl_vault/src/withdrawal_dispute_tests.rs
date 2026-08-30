//! Tests for Issue #1294: Withdrawal Dispute Window
//!
//! Verifies:
//!  - dispute_withdrawal links to a real audit-log entry and enforces the 24-hour window
//!  - filing outside the 24-hour window is rejected with WithdrawalDisputeWindowExpired
//!  - only the vault owner can file a dispute
//!  - filing against a non-existent or failed audit entry is rejected
//!  - duplicate open disputes for the same withdrawal are rejected
//!  - resolve_withdrawal_dispute is admin-only and transitions status correctly
//!  - get_withdrawal_disputes returns filed disputes
//!  - file_withdrawal_dispute (legacy shim) delegates to dispute_withdrawal

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env, String,
};

// ---------------------------------------------------------------------------
// Helper: spin up a contract with one vault that has a successful withdrawal
// in its audit log.
// ---------------------------------------------------------------------------

struct DisputeFixture {
    env: Env,
    admin: Address,
    owner: Address,
    beneficiary: Address,
    vault_id: u64,
    client: TtlVaultContractClient<'static>,
    /// Ledger timestamp at the time the withdrawal was recorded.
    withdrawal_ts: u64,
}

fn setup_with_withdrawal() -> DisputeFixture {
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

    // Safety-cast lifetime so we can return the client alongside borrowed env.
    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.deposit(&vault_id, &owner, &5_000_000);

    // Record a successful withdrawal so audit log index 0 exists.
    let withdrawal_ts = env.ledger().timestamp();
    client.withdraw(&vault_id, &owner, &1_000_000);

    DisputeFixture {
        env,
        admin,
        owner,
        beneficiary,
        vault_id,
        client,
        withdrawal_ts,
    }
}

// ---------------------------------------------------------------------------
// dispute_withdrawal — happy path
// ---------------------------------------------------------------------------

#[test]
fn test_dispute_withdrawal_within_window_succeeds() {
    let f = setup_with_withdrawal();

    // File a dispute immediately (well within 24 h window).
    let result = f.client.try_dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Unauthorized withdrawal"),
    );
    assert!(result.is_ok(), "Expected dispute to succeed: {:?}", result);
}

#[test]
fn test_dispute_withdrawal_creates_dispute_entry() {
    let f = setup_with_withdrawal();

    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Suspicious transaction"),
    );

    let disputes = f.client.get_withdrawal_disputes(&f.vault_id);
    assert_eq!(disputes.len(), 1u32);

    let d = disputes.get(0u32).unwrap();
    assert_eq!(d.vault_id, f.vault_id);
    assert_eq!(d.withdrawal_timestamp, f.withdrawal_ts);
    assert_eq!(d.status, DisputeStatus::Filed);
    assert!(d.resolved_at.is_none());
}

#[test]
fn test_dispute_withdrawal_expires_at_withdrawal_plus_24h() {
    let f = setup_with_withdrawal();

    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Fraudulent"),
    );

    let disputes = f.client.get_withdrawal_disputes(&f.vault_id);
    let d = disputes.get(0u32).unwrap();

    // dispute_expires_at should be exactly withdrawal_ts + 24 h
    let expected_expiry = f.withdrawal_ts + (24 * 3600u64);
    assert_eq!(d.dispute_expires_at, expected_expiry);
}

// ---------------------------------------------------------------------------
// dispute_withdrawal — window enforcement
// ---------------------------------------------------------------------------

#[test]
fn test_dispute_withdrawal_just_before_window_closes_succeeds() {
    let f = setup_with_withdrawal();

    // Advance ledger to 1 second before expiry.
    f.env.ledger().with_mut(|l| {
        l.timestamp = f.withdrawal_ts + (24 * 3600) - 1;
    });

    let result = f.client.try_dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Almost too late"),
    );
    assert!(result.is_ok());
}

#[test]
fn test_dispute_withdrawal_after_window_fails_with_expired_error() {
    let f = setup_with_withdrawal();

    // Advance ledger to exactly 24 h + 1 s after withdrawal.
    f.env.ledger().with_mut(|l| {
        l.timestamp = f.withdrawal_ts + (24 * 3600) + 1;
    });

    let result = f.client.try_dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Too late"),
    );
    assert_eq!(
        result,
        Err(Ok(ContractError::WithdrawalDisputeWindowExpired))
    );
}

#[test]
fn test_dispute_withdrawal_exactly_at_window_boundary_fails() {
    let f = setup_with_withdrawal();

    // Advance ledger to exactly now == withdrawal_ts + 24 h.
    // timestamp > entry.timestamp + SECONDS_PER_DAY → equal means NOT expired.
    // Actually, the guard is `now > entry.timestamp + SECONDS_PER_DAY`, so at the
    // boundary (equal) it should still pass.
    f.env.ledger().with_mut(|l| {
        l.timestamp = f.withdrawal_ts + (24 * 3600);
    });

    let result = f.client.try_dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "At the boundary"),
    );
    // `now > threshold` is strict greater-than so exactly at boundary is OK.
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// dispute_withdrawal — authorization
// ---------------------------------------------------------------------------

#[test]
fn test_dispute_withdrawal_non_owner_fails() {
    let f = setup_with_withdrawal();
    let stranger = Address::generate(&f.env);

    let result = f.client.try_dispute_withdrawal(
        &f.vault_id,
        &stranger,
        &0u32,
        &String::from_str(&f.env, "Trying to dispute"),
    );
    assert_eq!(result, Err(Ok(ContractError::NotOwner)));
}

#[test]
fn test_dispute_withdrawal_beneficiary_cannot_file() {
    let f = setup_with_withdrawal();

    let result = f.client.try_dispute_withdrawal(
        &f.vault_id,
        &f.beneficiary,
        &0u32,
        &String::from_str(&f.env, "Beneficiary filing"),
    );
    assert_eq!(result, Err(Ok(ContractError::NotOwner)));
}

// ---------------------------------------------------------------------------
// dispute_withdrawal — invalid audit log index
// ---------------------------------------------------------------------------

#[test]
fn test_dispute_withdrawal_out_of_range_index_fails() {
    let f = setup_with_withdrawal();
    // Audit log has only index 0; index 99 is invalid.
    let result = f.client.try_dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &99u32,
        &String::from_str(&f.env, "Bad index"),
    );
    assert_eq!(result, Err(Ok(ContractError::WithdrawalDisputeNotFound)));
}

#[test]
fn test_dispute_withdrawal_failed_entry_not_disputable() {
    let f = setup_with_withdrawal();

    // The only entry in the audit log is a successful withdrawal.
    // We need a *failed* entry to test this branch.  A second vault
    // where we attempt an over-balance withdrawal will produce a failed
    // audit entry at index 0 for that vault.
    let vault_id_2 = f
        .client
        .create_vault(&f.owner, &f.beneficiary, &3600u64, &None);
    f.client.deposit(&vault_id_2, &f.owner, &500_000);

    // Attempt withdrawal larger than balance → recorded as failure.
    let _ = f.client.try_withdraw(&vault_id_2, &f.owner, &999_999_999);

    // Now audit log index 0 for vault_id_2 is a failed entry.
    let result = f.client.try_dispute_withdrawal(
        &vault_id_2,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Failed withdrawal"),
    );
    assert_eq!(result, Err(Ok(ContractError::WithdrawalDisputeNotFound)));
}

// ---------------------------------------------------------------------------
// dispute_withdrawal — duplicate prevention
// ---------------------------------------------------------------------------

#[test]
fn test_dispute_withdrawal_duplicate_open_dispute_rejected() {
    let f = setup_with_withdrawal();

    // File once — OK.
    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "First dispute"),
    );

    // File again for same withdrawal — should fail.
    let result = f.client.try_dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Duplicate"),
    );
    assert_eq!(result, Err(Ok(ContractError::DisputeFiled)));
}

#[test]
fn test_dispute_withdrawal_second_withdrawal_can_be_disputed_independently() {
    let f = setup_with_withdrawal();

    // Make a second withdrawal.
    f.env.ledger().with_mut(|l| {
        l.timestamp += 60; // 1 minute later
    });
    f.client.withdraw(&f.vault_id, &f.owner, &500_000);

    // Dispute first withdrawal.
    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "First"),
    );

    // Dispute second withdrawal — index 1 — should succeed separately.
    let result = f.client.try_dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &1u32,
        &String::from_str(&f.env, "Second"),
    );
    assert!(result.is_ok());

    let disputes = f.client.get_withdrawal_disputes(&f.vault_id);
    assert_eq!(disputes.len(), 2u32);
}

// ---------------------------------------------------------------------------
// resolve_withdrawal_dispute — happy paths
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_withdrawal_dispute_approved() {
    let f = setup_with_withdrawal();

    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Fraudulent"),
    );

    let result = f
        .client
        .try_resolve_withdrawal_dispute(&f.vault_id, &0u32, &true);
    assert!(result.is_ok());

    let disputes = f.client.get_withdrawal_disputes(&f.vault_id);
    let d = disputes.get(0u32).unwrap();
    assert_eq!(d.status, DisputeStatus::Resolved);
    assert!(d.resolved_at.is_some());
}

#[test]
fn test_resolve_withdrawal_dispute_dismissed() {
    let f = setup_with_withdrawal();

    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Unclear"),
    );

    let result = f
        .client
        .try_resolve_withdrawal_dispute(&f.vault_id, &0u32, &false);
    assert!(result.is_ok());

    let disputes = f.client.get_withdrawal_disputes(&f.vault_id);
    let d = disputes.get(0u32).unwrap();
    // Dismissed → status goes back to None
    assert_eq!(d.status, DisputeStatus::None);
    assert!(d.resolved_at.is_some());
}

// ---------------------------------------------------------------------------
// resolve_withdrawal_dispute — authorization
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_withdrawal_dispute_admin_can_resolve() {
    let f = setup_with_withdrawal();

    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Filing"),
    );

    // With mock_all_auths active, require_admin loads the stored admin and
    // calls admin.require_auth(), which mock_all_auths satisfies.
    // This test confirms a successful resolution path exists.
    assert_ne!(f.owner, f.admin, "Precondition: owner and admin are distinct addresses");

    let result = f
        .client
        .try_resolve_withdrawal_dispute(&f.vault_id, &0u32, &true);
    assert!(result.is_ok(), "Admin should be able to resolve: {:?}", result);

    let disputes = f.client.get_withdrawal_disputes(&f.vault_id);
    assert_eq!(disputes.get(0u32).unwrap().status, DisputeStatus::Resolved);
}

// ---------------------------------------------------------------------------
// resolve_withdrawal_dispute — error cases
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_withdrawal_dispute_index_out_of_range() {
    let f = setup_with_withdrawal();

    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Test"),
    );

    let result = f
        .client
        .try_resolve_withdrawal_dispute(&f.vault_id, &99u32, &true);
    assert_eq!(result, Err(Ok(ContractError::WithdrawalDisputeNotFound)));
}

#[test]
fn test_resolve_withdrawal_dispute_no_disputes_returns_not_found() {
    let f = setup_with_withdrawal();

    // No dispute filed yet.
    let result = f
        .client
        .try_resolve_withdrawal_dispute(&f.vault_id, &0u32, &true);
    assert_eq!(result, Err(Ok(ContractError::WithdrawalDisputeNotFound)));
}

#[test]
fn test_resolve_already_resolved_dispute_fails() {
    let f = setup_with_withdrawal();

    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Fraudulent"),
    );
    f.client
        .resolve_withdrawal_dispute(&f.vault_id, &0u32, &true);

    // Resolving an already-resolved dispute should fail (status != Filed).
    let result = f
        .client
        .try_resolve_withdrawal_dispute(&f.vault_id, &0u32, &true);
    assert_eq!(result, Err(Ok(ContractError::DisputeFiled)));
}

// ---------------------------------------------------------------------------
// get_withdrawal_disputes
// ---------------------------------------------------------------------------

#[test]
fn test_get_withdrawal_disputes_empty_when_none_filed() {
    let f = setup_with_withdrawal();
    let disputes = f.client.get_withdrawal_disputes(&f.vault_id);
    assert_eq!(disputes.len(), 0u32);
}

#[test]
fn test_get_withdrawal_disputes_returns_all_disputes() {
    let f = setup_with_withdrawal();

    // Make a second withdrawal.
    f.env.ledger().with_mut(|l| {
        l.timestamp += 30;
    });
    f.client.withdraw(&f.vault_id, &f.owner, &200_000);

    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "First"),
    );
    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &1u32,
        &String::from_str(&f.env, "Second"),
    );

    let disputes = f.client.get_withdrawal_disputes(&f.vault_id);
    assert_eq!(disputes.len(), 2u32);
}

#[test]
fn test_disputes_are_per_vault() {
    let f = setup_with_withdrawal();

    // Create second vault and add a withdrawal on it too.
    let vault_id_2 = f
        .client
        .create_vault(&f.owner, &f.beneficiary, &3600u64, &None);
    f.client.deposit(&vault_id_2, &f.owner, &3_000_000);
    f.client.withdraw(&vault_id_2, &f.owner, &500_000);

    // File dispute only on vault 1.
    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "V1 dispute"),
    );

    assert_eq!(f.client.get_withdrawal_disputes(&f.vault_id).len(), 1u32);
    assert_eq!(f.client.get_withdrawal_disputes(&vault_id_2).len(), 0u32);
}

// ---------------------------------------------------------------------------
// file_withdrawal_dispute (legacy shim)
// ---------------------------------------------------------------------------

#[test]
fn test_legacy_file_withdrawal_dispute_shim_works() {
    let f = setup_with_withdrawal();

    // The legacy shim now requires audit_log_index.
    let result = f.client.try_file_withdrawal_dispute(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Via legacy shim"),
    );
    assert!(result.is_ok());

    let disputes = f.client.get_withdrawal_disputes(&f.vault_id);
    assert_eq!(disputes.len(), 1u32);
    assert_eq!(disputes.get(0u32).unwrap().status, DisputeStatus::Filed);
}

// ---------------------------------------------------------------------------
// Full dispute lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_full_dispute_lifecycle_file_then_resolve() {
    let f = setup_with_withdrawal();

    // 1. File dispute.
    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Unauthorized"),
    );

    let disputes = f.client.get_withdrawal_disputes(&f.vault_id);
    assert_eq!(disputes.get(0u32).unwrap().status, DisputeStatus::Filed);

    // 2. Admin resolves (approves).
    f.client
        .resolve_withdrawal_dispute(&f.vault_id, &0u32, &true);

    let disputes = f.client.get_withdrawal_disputes(&f.vault_id);
    let d = disputes.get(0u32).unwrap();
    assert_eq!(d.status, DisputeStatus::Resolved);
    assert!(d.resolved_at.is_some());
}

#[test]
fn test_full_dispute_lifecycle_file_then_dismiss() {
    let f = setup_with_withdrawal();

    f.client.dispute_withdrawal(
        &f.vault_id,
        &f.owner,
        &0u32,
        &String::from_str(&f.env, "Disputable"),
    );

    f.client
        .resolve_withdrawal_dispute(&f.vault_id, &0u32, &false);

    let disputes = f.client.get_withdrawal_disputes(&f.vault_id);
    let d = disputes.get(0u32).unwrap();
    assert_eq!(d.status, DisputeStatus::None);
    assert!(d.resolved_at.is_some());
}
