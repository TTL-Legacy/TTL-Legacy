#![cfg(test)]

//! Tests for vault-level pause/resume (Issue #790).
//!
//! Issue #2031: in-flight check-in transactions that land in the same ledger
//! as the pause must be blocked before any state mutation.  The pause gate is
//! evaluated at the top of every check-in path so that `CHECK_IN_RECORDED_TOPIC`
//! is never emitted and `last_check_in` is never updated for a paused vault.

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

use crate::tests::common::setup_with_roles as setup;

// ─── pause / resume basic behaviour ─────────────────────────────────────────

#[test]
fn test_pause_vault_owner_only() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let non_owner = Address::generate(&env);
    let result = client.try_pause_vault(&vault_id, &non_owner);
    assert!(result.is_err(), "non-owner must not be allowed to pause vault");
}

#[test]
fn test_pause_vault_sets_flag() {
    let (_env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    client.pause_vault(&vault_id, &owner);
    assert!(
        client.is_vault_paused(&vault_id),
        "vault must be paused after pause_vault"
    );
}

#[test]
fn test_pause_vault_active_vault() {
    let (_env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Should succeed on an active vault.
    let result = client.try_pause_vault(&vault_id, &owner);
    assert!(result.is_ok(), "pausing an active vault must succeed");
}

#[test]
fn test_pause_vault_already_paused() {
    let (_env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    client.pause_vault(&vault_id, &owner);
    // Idempotent: pausing again should not panic.
    let result = client.try_pause_vault(&vault_id, &owner);
    assert!(
        result.is_ok(),
        "pausing an already-paused vault must be idempotent"
    );
    assert!(client.is_vault_paused(&vault_id));
}

#[test]
fn test_pause_vault_released_vault() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&owner, &vault_id, &100_000i128);

    // Check in then expire.
    env.ledger().set_timestamp(10);
    client.check_in(&vault_id, &owner, &None, &None, &None);
    env.ledger().set_timestamp(10 + interval + 1);
    client.trigger_release(&vault_id);

    // Pausing a released vault should be rejected.
    let result = client.try_pause_vault(&vault_id, &owner);
    assert!(
        result.is_err(),
        "pausing a released vault must be rejected"
    );
}

// ─── resume ──────────────────────────────────────────────────────────────────

#[test]
fn test_resume_vault_owner_only() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.pause_vault(&vault_id, &owner);

    let non_owner = Address::generate(&env);
    let result = client.try_resume_vault(&vault_id, &non_owner);
    assert!(result.is_err(), "non-owner must not be allowed to resume vault");
}

#[test]
fn test_resume_vault_clears_flag() {
    let (_env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    client.pause_vault(&vault_id, &owner);
    assert!(client.is_vault_paused(&vault_id));

    client.resume_vault(&vault_id, &owner);
    assert!(
        !client.is_vault_paused(&vault_id),
        "vault must not be paused after resume_vault"
    );
}

#[test]
fn test_resume_vault_paused_vault() {
    let (_env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.pause_vault(&vault_id, &owner);

    let result = client.try_resume_vault(&vault_id, &owner);
    assert!(result.is_ok(), "resuming a paused vault must succeed");
    assert!(!client.is_vault_paused(&vault_id));
}

#[test]
fn test_resume_vault_not_paused() {
    let (_env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Resume on a non-paused vault is idempotent.
    let result = client.try_resume_vault(&vault_id, &owner);
    assert!(
        result.is_ok(),
        "resuming a non-paused vault must be idempotent"
    );
    assert!(!client.is_vault_paused(&vault_id));
}

// ─── trigger_release interaction ─────────────────────────────────────────────

#[test]
fn test_trigger_release_blocked_while_paused() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&owner, &vault_id, &100_000i128);

    // Check in and expire.
    env.ledger().set_timestamp(10);
    client.check_in(&vault_id, &owner, &None, &None, &None);
    env.ledger().set_timestamp(10 + interval + 1);

    // Pause before release.
    client.pause_vault(&vault_id, &owner);

    let result = client.try_trigger_release(&vault_id);
    assert!(
        result.is_err(),
        "trigger_release must be blocked while vault is paused"
    );
    assert_eq!(
        client.get_release_status(&vault_id),
        ReleaseStatus::Locked,
        "status must remain Locked when trigger_release is blocked by pause"
    );
}

#[test]
fn test_trigger_release_allowed_after_resume() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&owner, &vault_id, &100_000i128);

    // Check in and expire.
    env.ledger().set_timestamp(10);
    client.check_in(&vault_id, &owner, &None, &None, &None);
    env.ledger().set_timestamp(10 + interval + 1);

    // Pause then resume.
    client.pause_vault(&vault_id, &owner);
    client.resume_vault(&vault_id, &owner);

    // Release should now succeed.
    client.trigger_release(&vault_id);
    assert_eq!(
        client.get_vault(&vault_id).status,
        ReleaseStatus::Released,
        "vault must be Released after trigger_release following resume"
    );
}

// ─── Issue #2031: pause gate evaluated before any state mutation ──────────────

/// When a vault is paused and a check-in is attempted, the check-in must be
/// fully rejected: `last_check_in` must NOT be updated and
/// `CHECK_IN_RECORDED_TOPIC` must NOT be emitted.
///
/// This test verifies that the pause gate sits before all state mutations in
/// the check-in path so that "in-flight" transactions hitting the same ledger
/// as a pause are silently dropped without altering vault state.
#[test]
fn test_paused_vault_check_in_does_not_mutate_state() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Perform an initial check-in to establish a known last_check_in baseline.
    env.ledger().set_timestamp(1_000);
    client.check_in(&vault_id, &owner, &None, &None, &None);
    let baseline_last_check_in = client.get_vault(&vault_id).last_check_in;
    assert_eq!(
        baseline_last_check_in, 1_000,
        "baseline last_check_in must equal the initial check-in timestamp"
    );

    // Pause the vault.
    client.pause_vault(&vault_id, &owner);
    assert!(client.is_vault_paused(&vault_id));

    // Advance time and attempt a check-in while paused.
    env.ledger().set_timestamp(2_000);
    let result = client.try_check_in(&vault_id, &owner, &None, &None, &None);
    assert!(
        result.is_err(),
        "check_in must fail when vault is paused"
    );

    // last_check_in must remain at the pre-pause value — no state mutation.
    let vault_after = client.get_vault(&vault_id);
    assert_eq!(
        vault_after.last_check_in,
        baseline_last_check_in,
        "last_check_in must not be updated when check_in is blocked by pause \
         (Issue #2031: pause gate must precede all state mutations)"
    );
}

/// Resuming the vault after a pause allows the next check-in to proceed and
/// updates last_check_in normally.
#[test]
fn test_check_in_resumes_after_vault_unpause() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    env.ledger().set_timestamp(1_000);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    client.pause_vault(&vault_id, &owner);
    env.ledger().set_timestamp(2_000);
    // Blocked while paused.
    let _ = client.try_check_in(&vault_id, &owner, &None, &None, &None);

    // Resume and check in again.
    client.resume_vault(&vault_id, &owner);
    env.ledger().set_timestamp(3_000);
    client.check_in(&vault_id, &owner, &None, &None, &None);

    let vault = client.get_vault(&vault_id);
    assert_eq!(
        vault.last_check_in, 3_000,
        "last_check_in must be updated after check-in following resume"
    );
}

// ─── pause / resume cycle ────────────────────────────────────────────────────

#[test]
fn test_pause_resume_cycle() {
    let (_env, owner, beneficiary, _admin, _token_address, client) = setup();
    let interval = 3_600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    for _ in 0..3 {
        client.pause_vault(&vault_id, &owner);
        assert!(client.is_vault_paused(&vault_id));
        client.resume_vault(&vault_id, &owner);
        assert!(!client.is_vault_paused(&vault_id));
    }
}

// ─── nonexistent vault ───────────────────────────────────────────────────────

#[test]
fn test_pause_vault_nonexistent() {
    let (env, _owner, _beneficiary, _admin, _token_address, client) = setup();
    let fake_caller = Address::generate(&env);
    let result = client.try_pause_vault(&99999u64, &fake_caller);
    assert!(result.is_err(), "pausing a nonexistent vault must fail");
}
