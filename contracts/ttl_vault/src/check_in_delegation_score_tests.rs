/// Tests for Check-In Delegation (#946) and Check-In Verification Score (#947).
extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
};

fn setup() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
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

    soroban_sdk::token::StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    (env, owner, beneficiary, admin, token_address, client)
}

// ── Issue #946: delegate_check_in ─────────────────────────────────────────────

/// Owner can register a delegate with no expiry.
#[test]
fn test_delegate_check_in_no_expiry() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client
        .delegate_check_in(&id, &owner, &delegate, &None)
        .unwrap();

    assert!(client.is_check_in_delegate_pub(&id, &delegate));
    assert!(!client.is_delegate_expired(&id, &delegate));
}

/// Owner can register a delegate with a future expiry.
#[test]
fn test_delegate_check_in_with_expiry() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let now = env.ledger().timestamp();
    let expiry = now + 7200; // 2 hours
    client
        .delegate_check_in(&id, &owner, &delegate, &Some(expiry))
        .unwrap();

    assert!(client.is_check_in_delegate_pub(&id, &delegate));
    // Expiry is in the future — not yet expired
    assert!(!client.is_delegate_expired(&id, &delegate));
}

/// Non-owner cannot register a delegate.
#[test]
fn test_delegate_check_in_non_owner_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let stranger = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let err = client
        .try_delegate_check_in(&id, &stranger, &delegate, &None)
        .unwrap_err()
        .unwrap();
    // NotOwner = 6
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6));
}

/// Registering the same delegate twice fails with InvalidBeneficiary (17).
#[test]
fn test_delegate_check_in_duplicate_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client
        .delegate_check_in(&id, &owner, &delegate, &None)
        .unwrap();

    let err = client
        .try_delegate_check_in(&id, &owner, &delegate, &None)
        .unwrap_err()
        .unwrap();
    // InvalidBeneficiary = 17 (reused for duplicate delegate)
    assert_eq!(err, soroban_sdk::Error::from_contract_error(17));
}

// ── Issue #946: check_in_as_delegate ─────────────────────────────────────────

/// Registered delegate (no expiry) can perform a check-in.
#[test]
fn test_check_in_as_delegate_success() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client
        .delegate_check_in(&id, &owner, &delegate, &None)
        .unwrap();

    env.ledger().with_mut(|l| l.timestamp += 100);
    client.check_in_as_delegate(&id, &delegate, &0u64).unwrap();

    let vault = client.get_vault(&id);
    assert!(vault.last_check_in > 0);
}

/// check_in_as_delegate increments nonce — replaying nonce=0 fails.
#[test]
fn test_check_in_as_delegate_nonce_increments() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client
        .delegate_check_in(&id, &owner, &delegate, &None)
        .unwrap();

    env.ledger().with_mut(|l| l.timestamp += 100);
    client.check_in_as_delegate(&id, &delegate, &0u64).unwrap();

    // Replay nonce=0 must fail with InvalidNonce (83)
    env.ledger().with_mut(|l| l.timestamp += 100);
    let err = client
        .try_check_in_as_delegate(&id, &delegate, &0u64)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(83));
}

/// check_in_as_delegate with wrong nonce is rejected.
#[test]
fn test_check_in_as_delegate_wrong_nonce_rejected() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client
        .delegate_check_in(&id, &owner, &delegate, &None)
        .unwrap();

    env.ledger().with_mut(|l| l.timestamp += 100);
    // First nonce should be 0; sending 1 must fail
    let err = client
        .try_check_in_as_delegate(&id, &delegate, &1u64)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(83)); // InvalidNonce
}

/// Non-registered address cannot call check_in_as_delegate.
#[test]
fn test_check_in_as_delegate_non_delegate_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let stranger = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    env.ledger().with_mut(|l| l.timestamp += 100);
    let err = client
        .try_check_in_as_delegate(&id, &stranger, &0u64)
        .unwrap_err()
        .unwrap();
    // NotDelegate = 90
    assert_eq!(err, soroban_sdk::Error::from_contract_error(90));
}

// ── Issue #946: expiry enforcement ───────────────────────────────────────────

/// is_delegate_expired returns false before expiry time.
#[test]
fn test_is_delegate_not_expired_before_expiry() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let now = env.ledger().timestamp();
    client
        .delegate_check_in(&id, &owner, &delegate, &Some(now + 3600))
        .unwrap();

    assert!(!client.is_delegate_expired(&id, &delegate));
}

/// is_delegate_expired returns true once the ledger passes the expiry.
#[test]
fn test_is_delegate_expired_after_expiry() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let now = env.ledger().timestamp();
    let expiry = now + 500;
    client
        .delegate_check_in(&id, &owner, &delegate, &Some(expiry))
        .unwrap();

    // Advance ledger past expiry
    env.ledger().with_mut(|l| l.timestamp = expiry + 1);
    assert!(client.is_delegate_expired(&id, &delegate));
}

/// check_in_as_delegate fails with DelegateExpired (89) once expiry has passed.
#[test]
fn test_check_in_as_delegate_expired_fails() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let now = env.ledger().timestamp();
    let expiry = now + 500;
    client
        .delegate_check_in(&id, &owner, &delegate, &Some(expiry))
        .unwrap();

    // Advance ledger past expiry
    env.ledger().with_mut(|l| l.timestamp = expiry + 1);

    let err = client
        .try_check_in_as_delegate(&id, &delegate, &0u64)
        .unwrap_err()
        .unwrap();
    // DelegateExpired = 89
    assert_eq!(err, soroban_sdk::Error::from_contract_error(89));
}

/// Delegate with no expiry set never returns is_delegate_expired = true.
#[test]
fn test_delegate_no_expiry_never_expires() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client
        .delegate_check_in(&id, &owner, &delegate, &None)
        .unwrap();

    // Advance ledger by a long time
    env.ledger().with_mut(|l| l.timestamp += 999_999);
    assert!(!client.is_delegate_expired(&id, &delegate));
}

/// Delegate can check-in right up to expiry boundary (not yet expired).
#[test]
fn test_check_in_as_delegate_at_expiry_boundary_succeeds() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    let now = env.ledger().timestamp();
    let expiry = now + 500;
    client
        .delegate_check_in(&id, &owner, &delegate, &Some(expiry))
        .unwrap();

    // Advance to expiry - 1 (still valid)
    env.ledger().with_mut(|l| l.timestamp = expiry - 1);
    client.check_in_as_delegate(&id, &delegate, &0u64).unwrap();
}

// ── Issue #947: check_in_score initial state ─────────────────────────────────

/// Newly created vault starts with check_in_score = 10000.
#[test]
fn test_initial_check_in_score() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    assert_eq!(client.get_check_in_score(&id), 10000u32);
    let vault = client.get_vault(&id);
    assert_eq!(vault.check_in_score, 10000u32);
    assert_eq!(vault.total_check_ins, 0u32);
    assert_eq!(vault.on_time_check_ins, 0u32);
}

// ── Issue #947: score updates via check_in ───────────────────────────────────

/// On-time check-in keeps score at 10000.
#[test]
fn test_score_stays_perfect_after_on_time_check_in() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    // Check in well within the interval
    env.ledger().with_mut(|l| l.timestamp += 100);
    client.check_in(&id, &owner, &passkey, &0u64, &None, &None).unwrap();

    assert_eq!(client.get_check_in_score(&id), 10000u32);
    let vault = client.get_vault(&id);
    assert_eq!(vault.total_check_ins, 1u32);
    assert_eq!(vault.on_time_check_ins, 1u32);
}

/// Late check-in (after interval) lowers the score below 10000.
#[test]
fn test_score_decreases_after_late_check_in() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    // Use a short interval so we can advance past it easily
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    // Advance past the check-in interval → late
    env.ledger().with_mut(|l| l.timestamp += 7200);
    client.check_in(&id, &owner, &passkey, &0u64, &None, &None).unwrap();

    let score = client.get_check_in_score(&id);
    // 1 on-time out of 1 total would be 10000; but this was late → 0 on-time out of 1 = 0
    assert_eq!(score, 0u32);
    let vault = client.get_vault(&id);
    assert_eq!(vault.total_check_ins, 1u32);
    assert_eq!(vault.on_time_check_ins, 0u32);
}

/// Multiple on-time check-ins keep score at 10000.
#[test]
fn test_score_stays_perfect_multiple_on_time() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    for i in 1u64..=5 {
        env.ledger().with_mut(|l| l.timestamp = i * 100);
        client.check_in(&id, &owner, &passkey, &0u64, &None, &None).unwrap();
    }

    assert_eq!(client.get_check_in_score(&id), 10000u32);
    let vault = client.get_vault(&id);
    assert_eq!(vault.total_check_ins, 5u32);
    assert_eq!(vault.on_time_check_ins, 5u32);
}

/// Mixed on-time and late check-ins produce the correct proportional score.
#[test]
fn test_score_proportional_mixed_check_ins() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    // 1st: on time (within interval)
    env.ledger().with_mut(|l| l.timestamp += 100);
    client.check_in(&id, &owner, &passkey, &0u64, &None, &None).unwrap();

    // 2nd: on time
    env.ledger().with_mut(|l| l.timestamp += 100);
    client.check_in(&id, &owner, &passkey, &0u64, &None, &None).unwrap();

    // 3rd: late (past interval from 2nd check-in)
    env.ledger().with_mut(|l| l.timestamp += 7200);
    client.check_in(&id, &owner, &passkey, &0u64, &None, &None).unwrap();

    // 4th: late again
    env.ledger().with_mut(|l| l.timestamp += 7200);
    client.check_in(&id, &owner, &passkey, &0u64, &None, &None).unwrap();

    // 2 on-time out of 4 total → score = 2*10000/4 = 5000
    assert_eq!(client.get_check_in_score(&id), 5000u32);
}

// ── Issue #947: score updates via check_in_as_delegate ───────────────────────

/// Delegate on-time check-in keeps score at 10000.
#[test]
fn test_score_on_time_via_delegate() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client
        .delegate_check_in(&id, &owner, &delegate, &None)
        .unwrap();

    env.ledger().with_mut(|l| l.timestamp += 100);
    client.check_in_as_delegate(&id, &delegate, &0u64).unwrap();

    assert_eq!(client.get_check_in_score(&id), 10000u32);
    let vault = client.get_vault(&id);
    assert_eq!(vault.total_check_ins, 1u32);
    assert_eq!(vault.on_time_check_ins, 1u32);
}

/// Delegate late check-in lowers the score.
#[test]
fn test_score_late_via_delegate() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    client
        .delegate_check_in(&id, &owner, &delegate, &None)
        .unwrap();

    // Advance past the check-in interval → late
    env.ledger().with_mut(|l| l.timestamp += 7200);
    client.check_in_as_delegate(&id, &delegate, &0u64).unwrap();

    assert_eq!(client.get_check_in_score(&id), 0u32);
    let vault = client.get_vault(&id);
    assert_eq!(vault.total_check_ins, 1u32);
    assert_eq!(vault.on_time_check_ins, 0u32);
}

/// Score accumulates correctly across owner and delegate check-ins.
#[test]
fn test_score_mixed_owner_and_delegate_check_ins() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let delegate = Address::generate(&env);
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    let id = client.create_vault(&owner, &beneficiary, &7200u64, &None);

    client
        .delegate_check_in(&id, &owner, &delegate, &None)
        .unwrap();

    // Owner on-time check-in
    env.ledger().with_mut(|l| l.timestamp += 100);
    client.check_in(&id, &owner, &passkey, &0u64, &None, &None).unwrap();

    // Delegate on-time check-in
    env.ledger().with_mut(|l| l.timestamp += 100);
    client.check_in_as_delegate(&id, &delegate, &0u64).unwrap();

    // Owner late check-in
    env.ledger().with_mut(|l| l.timestamp += 100_000);
    client.check_in(&id, &owner, &passkey, &0u64, &None, &None).unwrap();

    // 2 on-time, 3 total → 2*10000/3 = 6666
    let score = client.get_check_in_score(&id);
    assert_eq!(score, 6666u32);
}

// ── Issue #947: get_check_in_score ───────────────────────────────────────────

/// get_check_in_score returns the same value as vault.check_in_score.
#[test]
fn test_get_check_in_score_matches_vault_field() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let passkey = BytesN::from_array(&env, &[1u8; 32]);
    let id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    env.ledger().with_mut(|l| l.timestamp += 100);
    client.check_in(&id, &owner, &passkey, &0u64, &None, &None).unwrap();

    env.ledger().with_mut(|l| l.timestamp += 7200);
    client.check_in(&id, &owner, &passkey, &0u64, &None, &None).unwrap();

    let vault_score = client.get_vault(&id).check_in_score;
    let fn_score = client.get_check_in_score(&id);
    assert_eq!(vault_score, fn_score);
}
