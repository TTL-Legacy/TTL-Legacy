//! Issue #2143 – beneficiary waitlist promotion.
//!
//! A vault may keep an ordered waitlist of up to three candidate beneficiaries.
//! When a confirmed beneficiary is removed, the head of the waitlist is
//! promoted into the vacated slot (inheriting its BPS allocation) and a
//! `BeneficiaryPromotedFromWaitlist` event is emitted. Remaining entries keep
//! their relative order.

#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    vec, Address, Env, IntoVal, Symbol, TryIntoVal,
};

use crate::tests::common::setup;

const INTERVAL: u64 = 3600;

fn make_split_vault(
    env: &Env,
    owner: &Address,
    client: &TtlVaultContractClient<'static>,
) -> (u64, Address, Address) {
    let b1 = Address::generate(env);
    let b2 = Address::generate(env);
    let splits = vec![
        env,
        BeneficiarySplit {
            address: b1.clone(),
            percentage: 50,
        },
        BeneficiarySplit {
            address: b2.clone(),
            percentage: 50,
        },
    ];
    let vault_id = client.create_vault_with_splits(owner, &splits, &INTERVAL, &None);
    (vault_id, b1, b2)
}

/// Collect the addresses emitted via `BeneficiaryPromotedFromWaitlist`.
fn promoted_addresses(env: &Env) -> alloc::vec::Vec<Address> {
    env.events()
        .all()
        .iter()
        .filter_map(|event| {
            let topics: soroban_sdk::Vec<soroban_sdk::Val> = event.1.clone().into_val(env);
            let topic: Option<Symbol> = topics.get(0).and_then(|value| value.try_into_val(env).ok());
            if topic == Some(BENEFICIARY_PROMOTED_FROM_WAITLIST_TOPIC) {
                event.2.try_into_val(env).ok()
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn test_beneficiary_waitlist_add_entry() {
    let (env, owner, _beneficiary, client) = setup();
    let (vault_id, _b1, _b2) = make_split_vault(&env, &owner, &client);
    let candidate = Address::generate(&env);

    client.add_waitlist_beneficiary(&vault_id, &owner, &candidate);

    let waitlist = client.get_beneficiary_waitlist(&vault_id);
    assert_eq!(waitlist.len(), 1);
    assert_eq!(waitlist.get(0).unwrap(), candidate);
}

#[test]
fn test_beneficiary_waitlist_max_three_entries() {
    let (env, owner, _beneficiary, client) = setup();
    let (vault_id, _b1, _b2) = make_split_vault(&env, &owner, &client);

    for _ in 0..MAX_WAITLIST_ENTRIES {
        let candidate = Address::generate(&env);
        client.add_waitlist_beneficiary(&vault_id, &owner, &candidate);
    }
    assert_eq!(client.get_beneficiary_waitlist(&vault_id).len(), MAX_WAITLIST_ENTRIES);

    let overflow = Address::generate(&env);
    let err = client
        .try_add_waitlist_beneficiary(&vault_id, &owner, &overflow)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::BeneficiaryCapacityExceeded);
    assert_eq!(client.get_beneficiary_waitlist(&vault_id).len(), MAX_WAITLIST_ENTRIES);
}

#[test]
fn test_primary_beneficiary_promotion_on_rejection() {
    let (env, owner, _beneficiary, client) = setup();
    let (vault_id, b1, _b2) = make_split_vault(&env, &owner, &client);
    let candidate = Address::generate(&env);
    client.add_waitlist_beneficiary(&vault_id, &owner, &candidate);

    // The owner removes the first beneficiary, vacating a confirmed slot.
    client.remove_beneficiary(&vault_id, &owner, &b1);

    let vault = client.get_vault(&vault_id);
    let promoted = vault
        .beneficiaries
        .iter()
        .find(|entry| entry.address == candidate)
        .expect("waitlist candidate should have been promoted");
    assert_eq!(promoted.bps, 5000);
    assert_eq!(client.get_beneficiary_waitlist(&vault_id).len(), 0);
}

#[test]
fn test_primary_beneficiary_promotion_on_timeout() {
    let (env, owner, _beneficiary, client) = setup();
    let (vault_id, b1, _b2) = make_split_vault(&env, &owner, &client);
    let candidates: alloc::vec::Vec<Address> =
        (0..3).map(|_| Address::generate(&env)).collect();
    for candidate in &candidates {
        client.add_waitlist_beneficiary(&vault_id, &owner, candidate);
    }

    // A vacancy promotes exactly the head of the queue and shifts the rest up.
    client.remove_beneficiary(&vault_id, &owner, &b1);

    let waitlist = client.get_beneficiary_waitlist(&vault_id);
    assert_eq!(waitlist.len(), 2);
    assert_eq!(waitlist.get(0).unwrap(), candidates[1]);
    assert_eq!(waitlist.get(1).unwrap(), candidates[2]);
}

#[test]
fn test_beneficiary_promoted_from_waitlist_event() {
    let (env, owner, _beneficiary, client) = setup();
    let (vault_id, b1, _b2) = make_split_vault(&env, &owner, &client);
    let candidate = Address::generate(&env);
    client.add_waitlist_beneficiary(&vault_id, &owner, &candidate);

    client.remove_beneficiary(&vault_id, &owner, &b1);

    let promotions = promoted_addresses(&env);
    assert_eq!(promotions.len(), 1);
    assert_eq!(promotions[0], candidate);
}

#[test]
fn test_beneficiary_waitlist_empty_promotion() {
    let (env, owner, _beneficiary, client) = setup();
    let (vault_id, b1, _b2) = make_split_vault(&env, &owner, &client);

    // Removing a beneficiary with an empty waitlist simply shrinks the list.
    client.remove_beneficiary(&vault_id, &owner, &b1);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.beneficiaries.len(), 1);
    assert_eq!(client.get_beneficiary_waitlist(&vault_id).len(), 0);
    assert!(promoted_addresses(&env).is_empty());
}

#[test]
fn test_beneficiary_waitlist_duplicate_prevention() {
    let (env, owner, _beneficiary, client) = setup();
    let (vault_id, _b1, _b2) = make_split_vault(&env, &owner, &client);
    let candidate = Address::generate(&env);

    client.add_waitlist_beneficiary(&vault_id, &owner, &candidate);
    let err = client
        .try_add_waitlist_beneficiary(&vault_id, &owner, &candidate)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::InvalidBeneficiary);
    assert_eq!(client.get_beneficiary_waitlist(&vault_id).len(), 1);
}

#[test]
fn test_beneficiary_waitlist_remove_entry() {
    let (env, owner, _beneficiary, client) = setup();
    let (vault_id, _b1, _b2) = make_split_vault(&env, &owner, &client);
    let first = Address::generate(&env);
    let second = Address::generate(&env);
    let third = Address::generate(&env);
    for candidate in [first.clone(), second.clone(), third.clone()] {
        client.add_waitlist_beneficiary(&vault_id, &owner, &candidate);
    }

    client.remove_waitlist_beneficiary(&vault_id, &owner, &second);

    let waitlist = client.get_beneficiary_waitlist(&vault_id);
    assert_eq!(waitlist.len(), 2);
    assert_eq!(waitlist.get(0).unwrap(), first);
    assert_eq!(waitlist.get(1).unwrap(), third);
}

#[test]
fn test_beneficiary_waitlist_sequential_promotions() {
    let (env, owner, _beneficiary, client) = setup();
    let (vault_id, b1, b2) = make_split_vault(&env, &owner, &client);
    let first = Address::generate(&env);
    let second = Address::generate(&env);
    client.add_waitlist_beneficiary(&vault_id, &owner, &first);
    client.add_waitlist_beneficiary(&vault_id, &owner, &second);

    client.remove_beneficiary(&vault_id, &owner, &b1);
    assert_eq!(client.get_beneficiary_waitlist(&vault_id).len(), 1);
    assert_eq!(
        client.get_beneficiary_waitlist(&vault_id).get(0).unwrap(),
        second
    );

    client.remove_beneficiary(&vault_id, &owner, &b2);
    assert_eq!(client.get_beneficiary_waitlist(&vault_id).len(), 0);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.beneficiaries.len(), 2);
    assert!(vault.beneficiaries.iter().any(|entry| entry.address == first));
    assert!(vault.beneficiaries.iter().any(|entry| entry.address == second));
    assert_eq!(promoted_addresses(&env), alloc::vec![first, second]);
}

#[test]
fn test_beneficiary_waitlist_persistence() {
    let (env, owner, _beneficiary, client) = setup();
    let (vault_id, _b1, _b2) = make_split_vault(&env, &owner, &client);
    let candidate = Address::generate(&env);
    client.add_waitlist_beneficiary(&vault_id, &owner, &candidate);

    // An unrelated state mutation must not drop the queue.
    client.deposit(&owner, &vault_id, &1_000);

    let waitlist = client.get_beneficiary_waitlist(&vault_id);
    assert_eq!(waitlist.len(), 1);
    assert_eq!(waitlist.get(0).unwrap(), candidate);
}
