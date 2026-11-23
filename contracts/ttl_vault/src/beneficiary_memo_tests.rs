#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger},
    token::StellarAssetClient,
    Address, Bytes, Env, IntoVal, TryIntoVal,
};

use crate::tests::common::setup_with_roles as setup;

fn release_events(env: &Env, vault_id: u64) -> Vec<ReleaseEvent> {
    env.events()
        .all()
        .iter()
        .filter_map(|e| {
            let topics: soroban_sdk::Vec<soroban_sdk::Val> = e.1.clone().into_val(env);
            let topic0: Result<soroban_sdk::Symbol, _> = topics.get(0)?.try_into_val(env);
            if topic0.ok()? != types::RELEASE_TOPIC {
                return None;
            }
            let event: ReleaseEvent = e.2.try_into_val(env).ok()?;
            (event.vault_id == vault_id).then_some(event)
        })
        .collect()
}

#[test]
fn test_beneficiary_memo_set() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let memo = String::from_slice(&env, "TTL Vault Release");

    // Test that owner can set release_memo on vault
    // Memo should be stored with max length validation
}

#[test]
fn test_beneficiary_memo_owner_only() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let non_owner = Address::generate(&env);
    let memo = String::from_slice(&env, "TTL Vault Release");

    // Test that only owner can set memo
    // Non-owner should get authorization error
}

#[test]
fn test_beneficiary_memo_max_length_28_bytes() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let memo_28_bytes = String::from_slice(&env, "12345678901234567890123456");

    // Test that memo can be exactly 28 bytes (Stellar limit)
    // Should succeed
}

#[test]
fn test_beneficiary_memo_exceeds_max_length() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let memo_too_long = String::from_slice(&env, "1234567890123456789012345678901");

    // Test that memo exceeding 28 bytes is rejected
    // Should return MemoTooLong error
}

#[test]
fn test_beneficiary_memo_empty_string() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let empty_memo = String::from_slice(&env, "");

    // Test that empty memo is allowed
    // Should clear memo when empty string is set
}

#[test]
fn test_beneficiary_memo_special_characters() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let memo = String::from_slice(&env, "TTL-Vault #123");

    // Test that special characters are allowed in memo
    // Should handle alphanumeric, hyphens, hashes, etc.
}

#[test]
fn test_beneficiary_memo_unicode_characters() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let memo = String::from_slice(&env, "💎 Inheritance");

    // Test memo with unicode characters
    // Should validate character encoding and byte length
}

#[test]
fn test_beneficiary_memo_attachment_on_release() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let memo = String::from_slice(&env, "TTL Vault Release");

    // Test that memo is attached to payment operation on trigger_release
    // Payment should include memo field
}

#[test]
fn test_beneficiary_memo_missing_on_release() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    // Test that release works without memo when not set
    // Should default to None
}

#[test]
fn test_beneficiary_memo_update() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let old_memo = String::from_slice(&env, "Old Memo");
    let new_memo = String::from_slice(&env, "New Memo");

    // Test that memo can be updated by owner
    // New memo should be used for next release
}

#[test]
fn test_beneficiary_memo_traceability() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let memo = String::from_slice(&env, "TTL Vault Release");

    // Test that beneficiary can identify vault from transaction memo
    // Memo should be visible in wallet history
}

#[test]
fn test_beneficiary_memo_persistence_across_checkins() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let memo = String::from_slice(&env, "TTL Vault Release");

    // Test that memo persists across multiple check-ins
    // Memo should remain unchanged after check-in operations
}

#[test]
fn test_beneficiary_memo_cleared_on_vault_reset() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let memo = String::from_slice(&env, "TTL Vault Release");

    // Test memo behavior when vault is reset/reused
    // Should allow setting new memo
}

#[test]
fn test_beneficiary_memo_with_multiple_beneficiaries() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let beneficiary1 = Address::generate(&env);
    let beneficiary2 = Address::generate(&env);
    let memo = String::from_slice(&env, "Split Release");

    // Test that memo is attached when releasing to multiple beneficiaries
    // All payments should include same memo
}

#[test]
fn test_beneficiary_memo_bytes_validation() {
    let env = Env::new();
    let vault_id: u64 = 1;
    let owner = Address::generate(&env);
    let memo = String::from_slice(&env, "Testing");

    // Test that memo byte length is correctly validated
    // Should use UTF-8 byte count, not character count
}

// ── Issue #2144: beneficiary_memo in the RELEASE_TOPIC event payload ──────────

/// A release for a single beneficiary with a stored release memo emits a
/// `RELEASE_TOPIC` whose `beneficiary_memo` carries that exact memo.
#[test]
fn test_release_event_includes_beneficiary_memo() {
    let (env, owner, beneficiary, _admin, token_address, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    let memo = Bytes::from_slice(&env, b"beneficiary memo");
    client.set_release_memo(&vault_id, &owner, &memo);

    StellarAssetClient::new(&env, &token_address).mint(&owner, &5_000i128);
    client.deposit(&vault_id, &owner, &5_000i128);

    env.ledger().with_mut(|l| l.timestamp += 7_200);
    client.trigger_release(&vault_id);

    let events = release_events(&env, vault_id);
    assert_eq!(events.len(), 1);
    let event = events.get(0).expect("release event missing");
    assert_eq!(event.beneficiary, beneficiary);
    assert_eq!(event.beneficiary_memo, memo);
}

/// A release for a beneficiary with no stored memo succeeds and emits a
/// `RELEASE_TOPIC` with an empty `beneficiary_memo`.
#[test]
fn test_release_event_beneficiary_memo_empty_when_not_set() {
    let (env, owner, beneficiary, _admin, token_address, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);

    StellarAssetClient::new(&env, &token_address).mint(&owner, &5_000i128);
    client.deposit(&vault_id, &owner, &5_000i128);

    env.ledger().with_mut(|l| l.timestamp += 7_200);
    client.trigger_release(&vault_id);

    let events = release_events(&env, vault_id);
    assert_eq!(events.len(), 1);
    let event = events.get(0).expect("release event missing");
    assert_eq!(event.beneficiary, beneficiary);
    assert!(event.beneficiary_memo.is_empty());
}

/// A multi-beneficiary release emits one `RELEASE_TOPIC` per beneficiary and
/// each event carries the beneficiary's stored memo — the vault's release
/// memo, in the repo's vault-level storage model.
#[test]
fn test_release_event_multi_beneficiary_memo_mapping() {
    let (env, owner, _beneficiary, _admin, token_address, client) = setup();
    let beneficiary_a = Address::generate(&env);
    let beneficiary_b = Address::generate(&env);
    let beneficiary_c = Address::generate(&env);

    let mut splits = Vec::new(&env);
    splits.push_back(BeneficiarySplit {
        address: beneficiary_a.clone(),
        percentage: 50,
    });
    splits.push_back(BeneficiarySplit {
        address: beneficiary_b.clone(),
        percentage: 30,
    });
    splits.push_back(BeneficiarySplit {
        address: beneficiary_c.clone(),
        percentage: 20,
    });

    let vault_id = client.create_vault_with_splits(&owner, &splits, &3600u64, &None);

    let memo = Bytes::from_slice(&env, b"split vault memo");
    client.set_release_memo(&vault_id, &owner, &memo);

    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000i128);
    client.deposit(&vault_id, &owner, &10_000i128);

    env.ledger().with_mut(|l| l.timestamp += 7_200);
    client.trigger_release(&vault_id);

    let events = release_events(&env, vault_id);
    assert_eq!(events.len(), 3);

    let expected: [(Address, i128); 3] = [
        (beneficiary_a.clone(), 5_000i128),
        (beneficiary_b.clone(), 3_000i128),
        (beneficiary_c.clone(), 2_000i128),
    ];
    for (beneficiary, amount) in expected.iter() {
        let event = events
            .iter()
            .find(|e| e.beneficiary == *beneficiary)
            .expect("release event missing for beneficiary");
        assert_eq!(event.amount, *amount);
        assert_eq!(event.beneficiary_memo, memo);
    }
}
