//! Integration tests for Issue #2051: vault owner notification when a
//! beneficiary updates their payout address.

use chrono::Utc;
use ttl_legacy_backend::db::{
    create_audit_store, create_payout_address_store, create_vault_store,
};
use ttl_legacy_backend::handlers::update_beneficiary_payout_address;
use ttl_legacy_backend::models::{UpdatePayoutAddressRequest, Vault, VaultStatus};

fn make_vault(id: &str, owner: &str, beneficiary: &str) -> Vault {
    Vault {
        id: id.to_string(),
        owner: owner.to_string(),
        beneficiary: beneficiary.to_string(),
        balance: 1000,
        check_in_interval: 86400,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(86400),
    }
}

/// Updating a payout address stores the new address in the payout store.
#[test]
fn test_update_payout_address_stores_new_address() {
    let vault_store = create_vault_store();
    let payout_store = create_payout_address_store();
    let audit_store = create_audit_store();

    vault_store
        .lock()
        .unwrap()
        .insert("v1".to_string(), make_vault("v1", "owner1", "ben1"));

    let request = UpdatePayoutAddressRequest {
        beneficiary: "ben1".to_string(),
        new_payout_address: "GCEZWK...NEWADDR".to_string(),
    };

    let result =
        update_beneficiary_payout_address(&vault_store, &payout_store, &audit_store, "v1", request);

    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    let entry = result.unwrap();
    assert_eq!(entry.vault_id, "v1");
    assert_eq!(entry.beneficiary, "ben1");
    assert_eq!(entry.payout_address, "GCEZWK...NEWADDR");

    // Also verify the value is persisted in the store
    let stored = payout_store
        .lock()
        .unwrap()
        .get("v1:ben1")
        .cloned();
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().payout_address, "GCEZWK...NEWADDR");
}

/// Updating a payout address appends an audit entry with action
/// "beneficiary_payout_address_changed".
#[test]
fn test_update_payout_address_emits_audit_event() {
    let vault_store = create_vault_store();
    let payout_store = create_payout_address_store();
    let audit_store = create_audit_store();

    vault_store
        .lock()
        .unwrap()
        .insert("v2".to_string(), make_vault("v2", "owner2", "ben2"));

    let request = UpdatePayoutAddressRequest {
        beneficiary: "ben2".to_string(),
        new_payout_address: "GNEW...ADDR2".to_string(),
    };

    update_beneficiary_payout_address(&vault_store, &payout_store, &audit_store, "v2", request)
        .expect("update should succeed");

    let entries = audit_store.lock().unwrap();
    let found = entries
        .iter()
        .any(|e| e.action == "beneficiary_payout_address_changed");
    assert!(found, "expected an audit entry with action 'beneficiary_payout_address_changed'");
}

/// When a previous payout address exists, the audit entry details must include
/// the old address.
#[test]
fn test_update_payout_address_records_old_address_in_audit() {
    let vault_store = create_vault_store();
    let payout_store = create_payout_address_store();
    let audit_store = create_audit_store();

    vault_store
        .lock()
        .unwrap()
        .insert("v3".to_string(), make_vault("v3", "owner3", "ben3"));

    // Set an initial address
    update_beneficiary_payout_address(
        &vault_store,
        &payout_store,
        &audit_store,
        "v3",
        UpdatePayoutAddressRequest {
            beneficiary: "ben3".to_string(),
            new_payout_address: "GOLD...ADDR".to_string(),
        },
    )
    .expect("first update should succeed");

    // Clear audit log so we only examine the second update's entry
    audit_store.lock().unwrap().clear();

    // Update to a new address
    update_beneficiary_payout_address(
        &vault_store,
        &payout_store,
        &audit_store,
        "v3",
        UpdatePayoutAddressRequest {
            beneficiary: "ben3".to_string(),
            new_payout_address: "GNEW...ADDR3".to_string(),
        },
    )
    .expect("second update should succeed");

    let entries = audit_store.lock().unwrap();
    let audit_entry = entries
        .iter()
        .find(|e| e.action == "beneficiary_payout_address_changed")
        .expect("audit entry not found");

    let old_addr = audit_entry
        .details
        .get("old_payout_address")
        .and_then(|v| v.as_str());
    assert_eq!(
        old_addr,
        Some("GOLD...ADDR"),
        "audit entry should record the previous payout address"
    );

    let new_addr = audit_entry
        .details
        .get("new_payout_address")
        .and_then(|v| v.as_str());
    assert_eq!(new_addr, Some("GNEW...ADDR3"));
}

/// Calling the handler with a beneficiary that does not match the vault's
/// registered beneficiary must return an error.
#[test]
fn test_update_payout_address_non_beneficiary_returns_error() {
    let vault_store = create_vault_store();
    let payout_store = create_payout_address_store();
    let audit_store = create_audit_store();

    vault_store
        .lock()
        .unwrap()
        .insert("v4".to_string(), make_vault("v4", "owner4", "real_ben"));

    let request = UpdatePayoutAddressRequest {
        beneficiary: "imposter".to_string(),
        new_payout_address: "GBAD...ADDR".to_string(),
    };

    let result =
        update_beneficiary_payout_address(&vault_store, &payout_store, &audit_store, "v4", request);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Not the vault beneficiary");
}

/// Calling the handler with a vault ID that does not exist must return an error.
#[test]
fn test_update_payout_address_vault_not_found_returns_error() {
    let vault_store = create_vault_store();
    let payout_store = create_payout_address_store();
    let audit_store = create_audit_store();

    let request = UpdatePayoutAddressRequest {
        beneficiary: "anyone".to_string(),
        new_payout_address: "GSOME...ADDR".to_string(),
    };

    let result = update_beneficiary_payout_address(
        &vault_store,
        &payout_store,
        &audit_store,
        "nonexistent",
        request,
    );

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Vault not found");
}
