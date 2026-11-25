//! Integration tests for issue #2049:
//! last_used_at is not updated when a passkey is used for check-in delegation.

use std::sync::{Arc, Mutex};

use chrono::Utc;
use ttl_legacy_backend::db::{create_audit_store, create_vault_store, AuditStore, PasskeyStore, VaultStore};
// check_in_delegation and check_in_with_passkey are re-exported by handlers::mod via
// `pub use passkey::*`, which is then re-exported at the crate root by `pub use handlers::*`.
use ttl_legacy_backend::{check_in_delegation, check_in_with_passkey};
use ttl_legacy_backend::models::{Passkey, Vault, VaultStatus};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_passkey_store() -> PasskeyStore {
    Arc::new(Mutex::new(Vec::<Passkey>::new()))
}

fn insert_passkey(store: &PasskeyStore, passkey: Passkey) {
    store.lock().unwrap().push(passkey);
}

fn insert_vault(store: &VaultStore, vault: Vault) {
    store.lock().unwrap().insert(vault.id.clone(), vault);
}

fn make_passkey(passkey_id: &str, vault_id: &str, owner: &str) -> Passkey {
    Passkey {
        passkey_id: passkey_id.to_string(),
        owner: owner.to_string(),
        vault_id: vault_id.to_string(),
        credential_id: "cred-test".to_string(),
        device_name: "Test Device".to_string(),
        registered_at: Utc::now(),
        last_used: None,
        is_backup: false,
    }
}

fn make_vault(vault_id: &str, owner: &str) -> Vault {
    Vault {
        id: vault_id.to_string(),
        owner: owner.to_string(),
        beneficiary: "beneficiary-1".to_string(),
        balance: 1000,
        check_in_interval: 86400,
        last_check_in: Utc::now() - chrono::Duration::hours(12),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(86400),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_check_in_with_passkey_updates_last_used() {
    let passkey_store = make_passkey_store();
    let vault_store = create_vault_store();
    let audit_store = create_audit_store();

    let passkey_id = "pk-001";
    let vault_id = "vault-001";
    let owner = "owner-1";

    insert_passkey(&passkey_store, make_passkey(passkey_id, vault_id, owner));
    insert_vault(&vault_store, make_vault(vault_id, owner));

    // Confirm last_used is None before the call.
    {
        let store = passkey_store.lock().unwrap();
        let pk = store
            .iter()
            .find(|pk| pk.passkey_id == passkey_id)
            .expect("passkey should exist");
        assert!(pk.last_used.is_none(), "last_used should be None before check-in");
    }

    let result = check_in_with_passkey(
        &passkey_store,
        &vault_store,
        &audit_store,
        passkey_id,
        vault_id,
    );
    assert!(result.is_ok(), "check_in_with_passkey should succeed");

    // Verify last_used is now Some.
    {
        let store = passkey_store.lock().unwrap();
        let pk = store
            .iter()
            .find(|pk| pk.passkey_id == passkey_id)
            .expect("passkey should still exist");
        assert!(
            pk.last_used.is_some(),
            "last_used should be Some after check_in_with_passkey"
        );
    }
}

#[test]
fn test_check_in_delegation_updates_last_used() {
    let passkey_store = make_passkey_store();
    let vault_store = create_vault_store();
    let audit_store = create_audit_store();

    let passkey_id = "pk-002";
    let vault_id = "vault-002";
    let owner = "owner-2";

    insert_passkey(&passkey_store, make_passkey(passkey_id, vault_id, owner));
    insert_vault(&vault_store, make_vault(vault_id, owner));

    // Confirm last_used is None before the call.
    {
        let store = passkey_store.lock().unwrap();
        let pk = store
            .iter()
            .find(|pk| pk.passkey_id == passkey_id)
            .expect("passkey should exist");
        assert!(pk.last_used.is_none(), "last_used should be None before delegation");
    }

    let result = check_in_delegation(
        &passkey_store,
        &vault_store,
        &audit_store,
        passkey_id,
        vault_id,
    );
    assert!(result.is_ok(), "check_in_delegation should succeed");

    // Verify last_used is now Some.
    {
        let store = passkey_store.lock().unwrap();
        let pk = store
            .iter()
            .find(|pk| pk.passkey_id == passkey_id)
            .expect("passkey should still exist");
        assert!(
            pk.last_used.is_some(),
            "last_used should be Some after check_in_delegation"
        );
    }
}

#[test]
fn test_check_in_delegation_unknown_passkey_returns_error() {
    let passkey_store = make_passkey_store();
    let vault_store = create_vault_store();
    let audit_store = create_audit_store();

    // No passkey is inserted — the call should return Err.
    let result = check_in_delegation(
        &passkey_store,
        &vault_store,
        &audit_store,
        "nonexistent-pk",
        "vault-003",
    );

    assert!(
        result.is_err(),
        "check_in_delegation should return Err for an unknown passkey"
    );
    assert_eq!(
        result.unwrap_err(),
        "Passkey not found",
        "error message should be 'Passkey not found'"
    );
}

#[test]
fn test_check_in_with_passkey_updates_vault_last_check_in() {
    let passkey_store = make_passkey_store();
    let vault_store = create_vault_store();
    let audit_store = create_audit_store();

    let passkey_id = "pk-004";
    let vault_id = "vault-004";
    let owner = "owner-4";

    insert_passkey(&passkey_store, make_passkey(passkey_id, vault_id, owner));

    // Create a vault whose last_check_in is well in the past.
    let old_time = Utc::now() - chrono::Duration::hours(48);
    let vault = Vault {
        last_check_in: old_time,
        ..make_vault(vault_id, owner)
    };
    insert_vault(&vault_store, vault);

    let before = Utc::now();

    let result = check_in_with_passkey(
        &passkey_store,
        &vault_store,
        &audit_store,
        passkey_id,
        vault_id,
    );
    assert!(result.is_ok(), "check_in_with_passkey should succeed");

    // last_check_in must have been updated to a time >= `before`.
    {
        let vaults = vault_store.lock().unwrap();
        let vault = vaults.get(vault_id).expect("vault should exist");
        assert!(
            vault.last_check_in >= before,
            "vault last_check_in should be updated to now (was {:?}, before = {:?})",
            vault.last_check_in,
            before
        );
    }
}

#[test]
fn test_check_in_delegation_updates_vault_last_check_in() {
    let passkey_store = make_passkey_store();
    let vault_store = create_vault_store();
    let audit_store = create_audit_store();

    let passkey_id = "pk-005";
    let vault_id = "vault-005";
    let owner = "owner-5";

    insert_passkey(&passkey_store, make_passkey(passkey_id, vault_id, owner));

    // Create a vault whose last_check_in is well in the past.
    let old_time = Utc::now() - chrono::Duration::hours(48);
    let vault = Vault {
        last_check_in: old_time,
        ..make_vault(vault_id, owner)
    };
    insert_vault(&vault_store, vault);

    let before = Utc::now();

    let result = check_in_delegation(
        &passkey_store,
        &vault_store,
        &audit_store,
        passkey_id,
        vault_id,
    );
    assert!(result.is_ok(), "check_in_delegation should succeed");

    // last_check_in must have been updated to a time >= `before`.
    {
        let vaults = vault_store.lock().unwrap();
        let vault = vaults.get(vault_id).expect("vault should exist");
        assert!(
            vault.last_check_in >= before,
            "vault last_check_in should be updated to now (was {:?}, before = {:?})",
            vault.last_check_in,
            before
        );
    }
}
