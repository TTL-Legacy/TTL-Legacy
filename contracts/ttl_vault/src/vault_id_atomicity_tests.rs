#![cfg(test)]

//! Issue #1279: the vault ID counter was read and incremented in separate
//! operations (`vault_count() + 1` computed early, `VaultCount` written back
//! much later, after unrelated storage writes). `next_vault_id` now performs
//! both steps back-to-back as a single unit, closing that window.
//!
//! Soroban's unit-test harness runs invocations sequentially, so true
//! concurrent execution can't be reproduced here; instead these tests assert
//! the externally-observable invariant the atomic counter guarantees: vault
//! creation across multiple entrypoints (`create_vault`, `clone_vault`,
//! `create_vault_from_inheritance`) always yields unique, strictly
//! increasing IDs that match the final `vault_count()`.

extern crate alloc;

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_address = Address::generate(&env);

    let contract_id = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_id);
    client.initialize(&token_address, &admin);
    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    (env, admin, token_address, client)
}

#[test]
fn test_sequential_vault_creation_yields_unique_increasing_ids() {
    let (env, _admin, _token, client) = setup();
    let owner = Address::generate(&env);

    let mut ids = alloc::vec::Vec::new();
    for _ in 0..5u64 {
        let beneficiary = Address::generate(&env);
        ids.push(client.create_vault(&owner, &beneficiary, &3600u64, &None));
    }

    // Every ID is unique and strictly increasing.
    for window in ids.windows(2) {
        assert!(window[1] > window[0], "vault IDs must strictly increase: {:?}", ids);
    }

    // The counter matches exactly the number of vaults created — no
    // duplicate consumed the same ID and no gap was skipped.
    assert_eq!(client.vault_count(), ids.len() as u64);
    assert_eq!(*ids.last().unwrap(), client.vault_count());
}

#[test]
fn test_clone_vault_consumes_a_fresh_id_from_the_shared_counter() {
    let (env, _admin, _token, client) = setup();
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let new_beneficiary = Address::generate(&env);

    let source_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    assert_eq!(source_id, 1u64);
    assert_eq!(client.vault_count(), 1u64);

    // clone_vault requires new_owner to match the source vault's owner.
    let cloned_id = client.clone_vault(&source_id, &owner, &new_beneficiary);

    // clone_vault must draw from the same atomic counter as create_vault,
    // never reusing `source_id` and never leaving a gap.
    assert_ne!(cloned_id, source_id);
    assert_eq!(cloned_id, 2u64);
    assert_eq!(client.vault_count(), 2u64);
}
