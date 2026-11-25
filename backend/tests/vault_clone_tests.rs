/// Integration tests for clone_vault (Issue #2050).
use chrono::Utc;
use ttl_legacy_backend::db::{
    create_audit_store, create_vault_store,
};
use ttl_legacy_backend::models::{Vault, VaultCloneConfig, VaultStatus};

// clone_vault is re-exported through handlers/mod.rs -> lib.rs pub use handlers::*
use ttl_legacy_backend::clone_vault;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn sample_vault(id: &str) -> Vault {
    Vault {
        id: id.to_string(),
        owner: "owner1".to_string(),
        beneficiary: "ben1".to_string(),
        balance: 5_000,
        check_in_interval: 86_400,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(86_400),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_clone_vault_creates_new_vault() {
    let store = create_vault_store();
    let audit_store = create_audit_store();

    store
        .lock()
        .unwrap()
        .insert("orig".to_string(), sample_vault("orig"));

    let resp = clone_vault(&store, &audit_store, "orig", VaultCloneConfig::default())
        .expect("clone should succeed");

    // IDs must differ
    assert_ne!(
        resp.original_vault_id, resp.cloned_vault_id,
        "cloned vault must have a new id"
    );

    // Both vaults must exist in the store
    let vaults = store.lock().unwrap();
    assert!(vaults.contains_key("orig"), "original vault must still exist");
    assert!(
        vaults.contains_key(&resp.cloned_vault_id),
        "cloned vault must be inserted into the store"
    );
}

#[test]
fn test_clone_vault_default_config_copies_base_fields() {
    let store = create_vault_store();
    let audit_store = create_audit_store();

    store
        .lock()
        .unwrap()
        .insert("orig".to_string(), sample_vault("orig"));

    let resp = clone_vault(&store, &audit_store, "orig", VaultCloneConfig::default())
        .expect("clone should succeed");

    let cloned = &resp.cloned_vault;
    assert_eq!(cloned.owner, "owner1");
    assert_eq!(cloned.beneficiary, "ben1");
    assert_eq!(cloned.balance, 5_000);
    assert_eq!(cloned.check_in_interval, 86_400);
}

#[test]
fn test_clone_vault_not_found_returns_error() {
    let store = create_vault_store();
    let audit_store = create_audit_store();

    let result = clone_vault(&store, &audit_store, "nonexistent", VaultCloneConfig::default());

    assert!(result.is_err(), "cloning a missing vault must return Err");
    assert_eq!(result.unwrap_err(), "Vault not found");
}

#[test]
fn test_clone_vault_with_all_fields_enabled() {
    let store = create_vault_store();
    let audit_store = create_audit_store();

    store
        .lock()
        .unwrap()
        .insert("orig".to_string(), sample_vault("orig"));

    let config = VaultCloneConfig {
        include_tags: true,
        include_notes: true,
        include_beneficiaries: true,
        include_vesting_schedule: true,
        include_release_conditions: true,
    };

    let resp = clone_vault(&store, &audit_store, "orig", config)
        .expect("clone with all fields enabled should succeed");

    assert!(!resp.cloned_vault_id.is_empty());
    assert_ne!(resp.original_vault_id, resp.cloned_vault_id);
}

#[test]
fn test_clone_vault_with_no_optional_fields() {
    let store = create_vault_store();
    let audit_store = create_audit_store();

    store
        .lock()
        .unwrap()
        .insert("orig".to_string(), sample_vault("orig"));

    let config = VaultCloneConfig {
        include_tags: false,
        include_notes: false,
        include_beneficiaries: false,
        include_vesting_schedule: false,
        include_release_conditions: false,
    };

    let resp = clone_vault(&store, &audit_store, "orig", config)
        .expect("clone with no optional fields should succeed");

    assert!(!resp.cloned_vault_id.is_empty());
}

#[test]
fn test_clone_vault_audit_entry_created() {
    let store = create_vault_store();
    let audit_store = create_audit_store();

    store
        .lock()
        .unwrap()
        .insert("orig".to_string(), sample_vault("orig"));

    let resp = clone_vault(&store, &audit_store, "orig", VaultCloneConfig::default())
        .expect("clone should succeed");

    let entries = audit_store.lock().unwrap();
    assert_eq!(entries.len(), 1, "exactly one audit entry should be written");

    let entry = &entries[0];
    assert_eq!(entry.action, "vault_cloned");
    assert_eq!(entry.actor, "owner1");

    let details = &entry.details;
    assert_eq!(
        details["original_vault_id"].as_str(),
        Some("orig"),
        "audit details must include original_vault_id"
    );
    assert_eq!(
        details["cloned_vault_id"].as_str(),
        Some(resp.cloned_vault_id.as_str()),
        "audit details must include cloned_vault_id"
    );
}
