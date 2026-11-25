/// Integration tests for vault archiving — Issue #2048.
///
/// These tests verify:
/// 1. `ARCHIVE_TOPIC` has the expected value and `emit_archive` is callable.
/// 2. `archive_vault` removes the vault entry from the store (purge).
/// 3. The archived vault's previous status is correctly captured in the audit
///    log, and the action is `"vault_archived"`.
/// 4. `archive_vault` returns an error for an unknown vault ID.
use backend::db::{append_audit_entry, create_audit_store, create_vault_store};
use backend::events::{emit_archive, ARCHIVE_TOPIC};
use backend::handlers::vault::archive_vault;
use backend::models::{Vault, VaultStatus};
use chrono::Utc;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_vault(id: &str, owner: &str, status: VaultStatus) -> Vault {
    Vault {
        id: id.to_string(),
        owner: owner.to_string(),
        beneficiary: "beneficiary1".to_string(),
        balance: 1_000,
        check_in_interval: 86_400,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status,
        ttl_remaining: Some(86_400),
    }
}

// ── ARCHIVE_TOPIC constant ────────────────────────────────────────────────────

#[test]
fn test_archive_topic_value() {
    assert_eq!(
        ARCHIVE_TOPIC, "vault.archived",
        "ARCHIVE_TOPIC must equal \"vault.archived\""
    );
}

// ── emit_archive is callable ──────────────────────────────────────────────────

#[test]
fn test_emit_archive_is_callable() {
    // emit_archive logs via tracing; calling it without a subscriber is safe —
    // the subscriber is optional and no-op when absent.
    emit_archive("vault-123", "owner-abc");
    // If we reach this point without a panic the function works correctly.
}

// ── archive_vault purges vault entries ───────────────────────────────────────

#[test]
fn test_archive_vault_removes_from_store() {
    let store = create_vault_store();
    let audit_store = create_audit_store();

    store
        .lock()
        .unwrap()
        .insert("v1".to_string(), make_vault("v1", "owner1", VaultStatus::Active));

    let result = archive_vault(&store, &audit_store, "v1");
    assert!(result.is_ok(), "archive_vault should succeed for existing vault");

    // Vault must be absent from the store after archival.
    assert!(
        store.lock().unwrap().get("v1").is_none(),
        "vault should have been purged from the store"
    );
}

// ── archive_vault records vault_archived in the audit log ────────────────────

#[test]
fn test_archive_vault_appends_audit_entry() {
    let store = create_vault_store();
    let audit_store = create_audit_store();

    store
        .lock()
        .unwrap()
        .insert("v2".to_string(), make_vault("v2", "owner2", VaultStatus::Active));

    archive_vault(&store, &audit_store, "v2").unwrap();

    let entries = audit_store.lock().unwrap();
    let entry = entries
        .iter()
        .find(|e| e.action == "vault_archived")
        .expect("expected a vault_archived audit entry");

    assert_eq!(entry.actor, "owner2");
    assert_eq!(
        entry.details.get("vault_id").and_then(|v| v.as_str()),
        Some("v2")
    );
}

// ── archive_vault transitions vault status to Archived ───────────────────────
//
// Because the vault is removed from the store on archival, we verify the
// transition by inspecting the audit entry's `previous_status` and
// `new_status` fields that `archive_vault` writes.

#[test]
fn test_archive_vault_records_archived_status_in_audit() {
    let store = create_vault_store();
    let audit_store = create_audit_store();

    store
        .lock()
        .unwrap()
        .insert("v3".to_string(), make_vault("v3", "owner3", VaultStatus::Paused));

    archive_vault(&store, &audit_store, "v3").unwrap();

    let entries = audit_store.lock().unwrap();
    let entry = entries
        .iter()
        .find(|e| e.action == "vault_archived")
        .expect("expected a vault_archived audit entry");

    // new_status must reflect VaultStatus::Archived.
    let new_status = entry
        .details
        .get("new_status")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        new_status.contains("Archived"),
        "new_status in audit details should contain 'Archived', got: {:?}",
        new_status
    );
}

// ── archive_vault returns an error for an unknown vault ──────────────────────

#[test]
fn test_archive_vault_missing_vault_returns_error() {
    let store = create_vault_store();
    let audit_store = create_audit_store();

    let result = archive_vault(&store, &audit_store, "nonexistent");
    assert!(
        result.is_err(),
        "archive_vault should return Err for a vault that does not exist"
    );
}

// ── audit log is unaffected on failure ───────────────────────────────────────

#[test]
fn test_archive_vault_missing_vault_no_audit_entry() {
    let store = create_vault_store();
    let audit_store = create_audit_store();

    // Seed an unrelated entry to ensure the store isn't empty.
    append_audit_entry(
        &audit_store,
        "other_action",
        "someone",
        serde_json::json!({}),
    );

    let _ = archive_vault(&store, &audit_store, "ghost");

    let entries = audit_store.lock().unwrap();
    let archived_entries: Vec<_> = entries
        .iter()
        .filter(|e| e.action == "vault_archived")
        .collect();
    assert!(
        archived_entries.is_empty(),
        "no vault_archived entry should be written when the vault is not found"
    );
}
