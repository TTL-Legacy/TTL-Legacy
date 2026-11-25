#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env, String,
};

fn setup_tags_vault_env() -> (
    Env,
    Address,
    Address,
    Address,
    u64,
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

    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    (env, owner, beneficiary, admin, vault_id, client)
}

// ========== Test: set_vault_tags_adds_single_tag ==========

#[test]
fn test_set_vault_tags_adds_single_tag() {
    let (env, owner, _, _, vault_id, client) = setup_tags_vault_env();
    let tags = vec![&env, String::from_slice(&env, "important")];

    client.set_vault_tags(&vault_id, &owner, &tags);

    let retrieved_tags = client.get_vault_tags(&vault_id);

    assert_eq!(retrieved_tags.len(), 1);
    assert_eq!(retrieved_tags.get(0).unwrap(), String::from_slice(&env, "important"));
}

// ========== Test: set_vault_tags_adds_multiple_tags ==========

#[test]
fn test_set_vault_tags_adds_multiple_tags() {
    let (env, owner, _, _, vault_id, client) = setup_tags_vault_env();
    let tags = vec![
        &env,
        String::from_slice(&env, "personal"),
        String::from_slice(&env, "legacy"),
        String::from_slice(&env, "urgent"),
    ];

    client.set_vault_tags(&vault_id, &owner, &tags);

    let retrieved_tags = client.get_vault_tags(&vault_id);

    assert_eq!(retrieved_tags.len(), 3);
}

// ========== Test: set_vault_tags_respects_max_tag_count ==========

#[test]
#[should_panic]
fn test_set_vault_tags_respects_max_tag_count() {
    let (env, owner, _, _, vault_id, client) = setup_tags_vault_env();
    let mut tags_vec = Vec::new();

    // Try to add more than 5 tags (exceeding limit)
    for i in 0..6 {
        tags_vec.push(format!("tag{}", i));
    }

    let tags = vec![
        &env,
        String::from_slice(&env, "tag0"),
        String::from_slice(&env, "tag1"),
        String::from_slice(&env, "tag2"),
        String::from_slice(&env, "tag3"),
        String::from_slice(&env, "tag4"),
        String::from_slice(&env, "tag5"),
    ];

    client.set_vault_tags(&vault_id, &owner, &tags);
}

// ========== Test: set_vault_tags_validates_tag_length ==========

#[test]
#[should_panic]
fn test_set_vault_tags_validates_tag_length() {
    let (env, owner, _, _, vault_id, client) = setup_tags_vault_env();

    // Create a tag longer than 32 characters
    let long_tag = String::from_slice(&env, "this-is-an-extremely-long-tag-that-exceeds-32-chars");
    let tags = vec![&env, long_tag];

    client.set_vault_tags(&vault_id, &owner, &tags);
}

// ========== Test: set_vault_tags_replaces_existing_tags ==========

#[test]
fn test_set_vault_tags_replaces_existing_tags() {
    let (env, owner, _, _, vault_id, client) = setup_tags_vault_env();

    let initial_tags = vec![
        &env,
        String::from_slice(&env, "old-tag"),
    ];

    client.set_vault_tags(&vault_id, &owner, &initial_tags);
    let retrieved_tags_1 = client.get_vault_tags(&vault_id);
    assert_eq!(retrieved_tags_1.len(), 1);

    let new_tags = vec![
        &env,
        String::from_slice(&env, "new-tag1"),
        String::from_slice(&env, "new-tag2"),
    ];

    client.set_vault_tags(&vault_id, &owner, &new_tags);
    let retrieved_tags_2 = client.get_vault_tags(&vault_id);

    assert_eq!(retrieved_tags_2.len(), 2);
    assert_eq!(retrieved_tags_2.get(0).unwrap(), String::from_slice(&env, "new-tag1"));
}

// ========== Test: set_vault_tags_only_callable_by_owner ==========

#[test]
#[should_panic]
fn test_set_vault_tags_only_callable_by_owner() {
    let (env, owner, _, _, vault_id, client) = setup_tags_vault_env();
    let unauthorized = Address::generate(&env);

    let tags = vec![&env, String::from_slice(&env, "secret")];

    client.set_vault_tags(&vault_id, &unauthorized, &tags);
}

// ========== Test: get_vault_tags_returns_empty_for_untagged_vault ==========

#[test]
fn test_get_vault_tags_returns_empty_for_untagged_vault() {
    let (env, _, _, _, vault_id, client) = setup_tags_vault_env();

    let tags = client.get_vault_tags(&vault_id);

    assert_eq!(tags.len(), 0);
}

// ========== Test: set_vault_tags_with_empty_list_clears_tags ==========

#[test]
fn test_set_vault_tags_with_empty_list_clears_tags() {
    let (env, owner, _, _, vault_id, client) = setup_tags_vault_env();

    // Add tags first
    let tags = vec![
        &env,
        String::from_slice(&env, "temp-tag"),
    ];
    client.set_vault_tags(&vault_id, &owner, &tags);

    // Clear tags with empty vector
    let empty_tags: Vec<String> = vec![&env];
    client.set_vault_tags(&vault_id, &owner, &empty_tags);

    let retrieved_tags = client.get_vault_tags(&vault_id);
    assert_eq!(retrieved_tags.len(), 0);
}

// ========== Test: set_vault_tags_allows_exactly_32_char_tags ==========

#[test]
fn test_set_vault_tags_allows_exactly_32_char_tags() {
    let (env, owner, _, _, vault_id, client) = setup_tags_vault_env();

    // Create a tag exactly 32 characters
    let tag_32_chars = String::from_slice(&env, "12345678901234567890123456789012");
    let tags = vec![&env, tag_32_chars];

    client.set_vault_tags(&vault_id, &owner, &tags);

    let retrieved_tags = client.get_vault_tags(&vault_id);
    assert_eq!(retrieved_tags.len(), 1);
}

// ========== Test: set_vault_tags_with_special_characters ==========

#[test]
fn test_set_vault_tags_with_special_characters() {
    let (env, owner, _, _, vault_id, client) = setup_tags_vault_env();

    let tags = vec![
        &env,
        String::from_slice(&env, "tag-with-dashes"),
        String::from_slice(&env, "tag_with_underscores"),
        String::from_slice(&env, "tag.with.dots"),
    ];

    client.set_vault_tags(&vault_id, &owner, &tags);

    let retrieved_tags = client.get_vault_tags(&vault_id);
    assert_eq!(retrieved_tags.len(), 3);
}

// ========== Test: get_vault_tags_preserves_tag_order ==========

#[test]
fn test_get_vault_tags_preserves_tag_order() {
    let (env, owner, _, _, vault_id, client) = setup_tags_vault_env();

    let tags = vec![
        &env,
        String::from_slice(&env, "first"),
        String::from_slice(&env, "second"),
        String::from_slice(&env, "third"),
    ];

    client.set_vault_tags(&vault_id, &owner, &tags);

    let retrieved_tags = client.get_vault_tags(&vault_id);

    assert_eq!(retrieved_tags.get(0).unwrap(), String::from_slice(&env, "first"));
    assert_eq!(retrieved_tags.get(1).unwrap(), String::from_slice(&env, "second"));
    assert_eq!(retrieved_tags.get(2).unwrap(), String::from_slice(&env, "third"));
}

// ========== Test: multiple_vaults_have_independent_tags ==========

#[test]
fn test_multiple_vaults_have_independent_tags() {
    let (env, owner, beneficiary, _, vault_1_id, client) = setup_tags_vault_env();

    let vault_2_id = client.create_vault(&owner, &beneficiary, &100u64, &None);

    let tags_1 = vec![&env, String::from_slice(&env, "vault1-tag")];
    let tags_2 = vec![&env, String::from_slice(&env, "vault2-tag")];

    client.set_vault_tags(&vault_1_id, &owner, &tags_1);
    client.set_vault_tags(&vault_2_id, &owner, &tags_2);

    let retrieved_tags_1 = client.get_vault_tags(&vault_1_id);
    let retrieved_tags_2 = client.get_vault_tags(&vault_2_id);

    assert_eq!(retrieved_tags_1.get(0).unwrap(), String::from_slice(&env, "vault1-tag"));
    assert_eq!(retrieved_tags_2.get(0).unwrap(), String::from_slice(&env, "vault2-tag"));
}
