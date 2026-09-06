//! Tests for Issue #1339: legal document anchoring via Stellar.
//!
//! These tests verify:
//!   1. `anchor_document` stores an anchor record and returns a sequential doc_id.
//!   2. `get_document_anchor` retrieves a specific anchor by vault_id + doc_id.
//!   3. `list_document_anchors` returns all anchors (including removed).
//!   4. `remove_document_anchor` soft-deletes (sets removed = true).
//!   5. Access controls: only the vault owner can anchor or remove documents.
//!   6. Multiple anchors per vault receive distinct, sequential IDs.

#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, BytesN, Env, String,
};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn setup() -> (
    Env,
    Address, // owner
    Address, // beneficiary
    Address, // admin
    Address, // token_address
    TtlVaultContractClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_id = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_id);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    (env, owner, beneficiary, admin, token_address, client)
}

/// Returns a deterministic 32-byte hash seeded with `seed`.
fn mock_hash(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

// Helper to create a vault with the correct argument order
fn create_test_vault(client: &TtlVaultContractClient<'static>, owner: &Address, beneficiary: &Address) -> u64 {
    client.create_vault(owner, beneficiary, &7_200_u64, &None)
}

// ─── anchor_document ─────────────────────────────────────────────────────────

/// Anchoring a document returns doc_id = 1 for the first document.
#[test]
fn test_anchor_document_returns_doc_id_one() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let vault_id = create_test_vault(&client, &owner, &beneficiary);

    let doc_id = client.anchor_document(
        &vault_id,
        &owner,
        &mock_hash(&env, 1),
        &LegalDocumentType::Will,
        &None,
    );
    assert_eq!(doc_id, 1);
}

/// Each successive anchor gets the next sequential ID.
#[test]
fn test_anchor_document_sequential_ids() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let vault_id = create_test_vault(&client, &owner, &beneficiary);

    let id1 = client.anchor_document(
        &vault_id,
        &owner,
        &mock_hash(&env, 1),
        &LegalDocumentType::Will,
        &None,
    );
    let id2 = client.anchor_document(
        &vault_id,
        &owner,
        &mock_hash(&env, 2),
        &LegalDocumentType::TrustDeed,
        &None,
    );
    let id3 = client.anchor_document(
        &vault_id,
        &owner,
        &mock_hash(&env, 3),
        &LegalDocumentType::PowerOfAttorney,
        &None,
    );

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

/// Anchored document hash is stored correctly.
#[test]
fn test_anchor_document_stores_hash() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let vault_id = create_test_vault(&client, &owner, &beneficiary);

    let expected_hash = mock_hash(&env, 42);
    let doc_id = client.anchor_document(
        &vault_id,
        &owner,
        &expected_hash,
        &LegalDocumentType::Will,
        &None,
    );

    let anchor = client.get_document_anchor(&vault_id, &doc_id);
    assert_eq!(anchor.doc_hash, expected_hash);
    assert_eq!(anchor.doc_id, doc_id);
    assert!(!anchor.removed);
}

/// Document type is preserved on retrieval.
#[test]
fn test_anchor_document_stores_doc_type() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let vault_id = create_test_vault(&client, &owner, &beneficiary);

    let doc_id = client.anchor_document(
        &vault_id,
        &owner,
        &mock_hash(&env, 5),
        &LegalDocumentType::TrustDeed,
        &None,
    );

    let anchor = client.get_document_anchor(&vault_id, &doc_id);
    assert_eq!(anchor.doc_type, LegalDocumentType::TrustDeed);
}

/// Optional storage_ref is preserved on retrieval.
#[test]
fn test_anchor_document_stores_storage_ref() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let vault_id = create_test_vault(&client, &owner, &beneficiary);

    let ipfs_ref = String::from_str(&env, "ipfs://QmExampleHashOfDocumentCid1234567890");
    let doc_id = client.anchor_document(
        &vault_id,
        &owner,
        &mock_hash(&env, 7),
        &LegalDocumentType::Will,
        &Some(ipfs_ref.clone()),
    );

    let anchor = client.get_document_anchor(&vault_id, &doc_id);
    assert_eq!(anchor.storage_ref, Some(ipfs_ref));
}

/// anchored_at timestamp is recorded as the ledger timestamp at call time.
#[test]
fn test_anchor_document_stores_timestamp() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();

    env.ledger().with_mut(|li| {
        li.timestamp = 1_700_000_000;
    });

    let vault_id = create_test_vault(&client, &owner, &beneficiary);
    let doc_id = client.anchor_document(
        &vault_id,
        &owner,
        &mock_hash(&env, 9),
        &LegalDocumentType::Other,
        &None,
    );

    let anchor = client.get_document_anchor(&vault_id, &doc_id);
    assert_eq!(anchor.anchored_at, 1_700_000_000);
}

// ─── access control ──────────────────────────────────────────────────────────

/// Non-owner cannot anchor a document.
#[test]
#[should_panic]
fn test_anchor_document_non_owner_rejected() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let vault_id = create_test_vault(&client, &owner, &beneficiary);

    let non_owner = Address::generate(&env);
    client.anchor_document(
        &vault_id,
        &non_owner,
        &mock_hash(&env, 99),
        &LegalDocumentType::Will,
        &None,
    );
}

/// Non-owner cannot remove a document anchor.
#[test]
#[should_panic]
fn test_remove_document_anchor_non_owner_rejected() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let vault_id = create_test_vault(&client, &owner, &beneficiary);

    let doc_id = client.anchor_document(
        &vault_id,
        &owner,
        &mock_hash(&env, 1),
        &LegalDocumentType::Will,
        &None,
    );

    let non_owner = Address::generate(&env);
    client.remove_document_anchor(&vault_id, &non_owner, &doc_id);
}

// ─── remove_document_anchor ──────────────────────────────────────────────────

/// Removing an anchor sets `removed = true` without deleting it.
#[test]
fn test_remove_document_anchor_soft_deletes() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let vault_id = create_test_vault(&client, &owner, &beneficiary);

    let doc_id = client.anchor_document(
        &vault_id,
        &owner,
        &mock_hash(&env, 3),
        &LegalDocumentType::Will,
        &None,
    );

    client.remove_document_anchor(&vault_id, &owner, &doc_id);

    let anchor = client.get_document_anchor(&vault_id, &doc_id);
    assert!(anchor.removed, "anchor should be marked as removed");
    // Hash is still present — record is not deleted.
    assert_eq!(anchor.doc_hash, mock_hash(&env, 3));
}

// ─── list_document_anchors ───────────────────────────────────────────────────

/// list_document_anchors returns all anchors in insertion order.
#[test]
fn test_list_document_anchors_returns_all() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let vault_id = create_test_vault(&client, &owner, &beneficiary);

    client.anchor_document(&vault_id, &owner, &mock_hash(&env, 1), &LegalDocumentType::Will, &None);
    client.anchor_document(&vault_id, &owner, &mock_hash(&env, 2), &LegalDocumentType::TrustDeed, &None);
    client.anchor_document(&vault_id, &owner, &mock_hash(&env, 3), &LegalDocumentType::PowerOfAttorney, &None);

    let anchors = client.list_document_anchors(&vault_id);
    assert_eq!(anchors.len(), 3);
}

/// list_document_anchors returns an empty vec when no documents are anchored.
#[test]
fn test_list_document_anchors_empty() {
    let (_env, owner, beneficiary, _admin, _token, client) = setup();
    let vault_id = create_test_vault(&client, &owner, &beneficiary);

    let anchors = client.list_document_anchors(&vault_id);
    assert_eq!(anchors.len(), 0);
}

/// list_document_anchors includes removed anchors (callers filter client-side).
#[test]
fn test_list_document_anchors_includes_removed() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();
    let vault_id = create_test_vault(&client, &owner, &beneficiary);

    let doc_id = client.anchor_document(
        &vault_id,
        &owner,
        &mock_hash(&env, 1),
        &LegalDocumentType::Will,
        &None,
    );
    client.remove_document_anchor(&vault_id, &owner, &doc_id);

    let anchors = client.list_document_anchors(&vault_id);
    assert_eq!(anchors.len(), 1);
    assert!(anchors.get(0).unwrap().removed);
}

/// Anchors from different vaults are independent.
#[test]
fn test_document_anchors_are_per_vault() {
    let (env, owner, beneficiary, _admin, _token, client) = setup();

    let vault_a = create_test_vault(&client, &owner, &beneficiary);
    let vault_b = create_test_vault(&client, &owner, &beneficiary);

    client.anchor_document(&vault_a, &owner, &mock_hash(&env, 1), &LegalDocumentType::Will, &None);
    client.anchor_document(&vault_a, &owner, &mock_hash(&env, 2), &LegalDocumentType::TrustDeed, &None);
    client.anchor_document(&vault_b, &owner, &mock_hash(&env, 9), &LegalDocumentType::Other, &None);

    let anchors_a = client.list_document_anchors(&vault_a);
    let anchors_b = client.list_document_anchors(&vault_b);

    assert_eq!(anchors_a.len(), 2);
    assert_eq!(anchors_b.len(), 1);
}
