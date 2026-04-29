use crate::models::*;
use crate::db::*;
use crate::notifications::NotificationService;
use chrono::Utc;
use serde_json::json;
use std::io::Write;
use std::sync::Arc;

pub fn search_vaults_handler(
    store: &VaultStore,
    query: SearchQuery,
) -> SearchResult {
    search_vaults(store, &query)
}

pub fn compare_vaults_handler(
    store: &VaultStore,
    vault_ids: Vec<String>,
) -> ComparisonResult {
    let vaults = store.lock().unwrap();
    let comparison_vaults: Vec<Vault> = vault_ids
        .iter()
        .filter_map(|id| vaults.get(id).cloned())
        .collect();

    ComparisonResult {
        vaults: comparison_vaults,
    }
}

pub fn export_vaults_handler(
    store: &VaultStore,
    event_store: &EventStore,
    audit_store: &AuditStore,
    vault_id: &str,
    format: &str,
) -> Result<String, String> {
    let vaults = store.lock().unwrap();
    let vault = vaults
        .get(vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    let history = get_vault_history(event_store, vault_id);
    let audit_log = get_vault_audit_log(audit_store, vault_id);

    let export_data = ExportData {
        vault,
        history,
        audit_log,
    };

    match format {
        "json" => Ok(serde_json::to_string_pretty(&export_data)
            .map_err(|e| e.to_string())?),
        "csv" => export_to_csv(&export_data),
        _ => Err("Unsupported format".to_string()),
    }
}

fn export_to_csv(data: &ExportData) -> Result<String, String> {
    let mut wtr = csv::Writer::from_writer(vec![]);

    // Write vault info
    wtr.write_record(&[
        "Type",
        "ID",
        "Owner",
        "Beneficiary",
        "Balance",
        "Status",
        "Created",
    ])
    .map_err(|e| e.to_string())?;

    wtr.write_record(&[
        "Vault",
        &data.vault.id,
        &data.vault.owner,
        &data.vault.beneficiary,
        &data.vault.balance.to_string(),
        &format!("{:?}", data.vault.status),
        &data.vault.created_at.to_rfc3339(),
    ])
    .map_err(|e| e.to_string())?;

    // Write events
    wtr.write_record(&["", "", "", "", "", "", ""])
        .map_err(|e| e.to_string())?;
    wtr.write_record(&["Event", "Type", "Timestamp", "Data", "", "", ""])
        .map_err(|e| e.to_string())?;

    for event in &data.history {
        wtr.write_record(&[
            "Event",
            &format!("{:?}", event.event_type),
            &event.timestamp.to_rfc3339(),
            &event.data.to_string(),
            "",
            "",
            "",
        ])
        .map_err(|e| e.to_string())?;
    }

    let buffer = wtr.into_inner().map_err(|e| e.to_string())?;
    String::from_utf8(buffer).map_err(|e| e.to_string())
}

pub fn generate_compliance_report(
    store: &VaultStore,
    event_store: &EventStore,
    vault_id: &str,
) -> Result<ComplianceReport, String> {
    let vaults = store.lock().unwrap();
    let vault = vaults
        .get(vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    let history = get_vault_history(event_store, vault_id);
    
    let mut fund_movements = Vec::new();
    let mut beneficiary_changes = Vec::new();
    let mut ttl_history = Vec::new();
    let mut total_deposits = 0i128;
    let mut total_withdrawals = 0i128;

    for event in history {
        match event.event_type {
            EventType::Deposit => {
                if let Some(amount) = event.data.get("amount").and_then(|v| v.as_i64()) {
                    total_deposits += amount as i128;
                    fund_movements.push(FundMovement {
                        timestamp: event.timestamp,
                        movement_type: "deposit".to_string(),
                        amount: amount as i128,
                        balance_after: vault.balance,
                    });
                }
            }
            EventType::Withdrawal => {
                if let Some(amount) = event.data.get("amount").and_then(|v| v.as_i64()) {
                    total_withdrawals += amount as i128;
                    fund_movements.push(FundMovement {
                        timestamp: event.timestamp,
                        movement_type: "withdrawal".to_string(),
                        amount: amount as i128,
                        balance_after: vault.balance,
                    });
                }
            }
            EventType::TtlUpdate => {
                if let Some(ttl) = event.data.get("ttl_remaining").and_then(|v| v.as_u64()) {
                    ttl_history.push(TtlEvent {
                        timestamp: event.timestamp,
                        event_type: "ttl_extended".to_string(),
                        ttl_remaining: Some(ttl),
                    });
                }
            }
            EventType::StatusChange => {
                if let Some(old_ben) = event.data.get("old_beneficiary").and_then(|v| v.as_str()) {
                    if let Some(new_ben) = event.data.get("new_beneficiary").and_then(|v| v.as_str()) {
                        beneficiary_changes.push(BeneficiaryChange {
                            timestamp: event.timestamp,
                            old_beneficiary: old_ben.to_string(),
                            new_beneficiary: new_ben.to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    Ok(ComplianceReport {
        vault_id: vault.id,
        owner: vault.owner,
        beneficiary: vault.beneficiary,
        report_generated_at: Utc::now(),
        fund_movements,
        beneficiary_changes,
        ttl_history,
        total_deposits,
        total_withdrawals,
        current_balance: vault.balance,
    })
}

pub fn export_compliance_report(
    report: &ComplianceReport,
    format: &str,
) -> Result<String, String> {
    match format {
        "json" => Ok(serde_json::to_string_pretty(report).map_err(|e| e.to_string())?),
        "pdf" => {
            // Minimal PDF export as text representation
            let mut pdf_content = String::new();
            pdf_content.push_str(&format!("COMPLIANCE REPORT\n"));
            pdf_content.push_str(&format!("Generated: {}\n\n", report.report_generated_at));
            pdf_content.push_str(&format!("Vault ID: {}\n", report.vault_id));
            pdf_content.push_str(&format!("Owner: {}\n", report.owner));
            pdf_content.push_str(&format!("Beneficiary: {}\n", report.beneficiary));
            pdf_content.push_str(&format!("Current Balance: {}\n", report.current_balance));
            pdf_content.push_str(&format!("Total Deposits: {}\n", report.total_deposits));
            pdf_content.push_str(&format!("Total Withdrawals: {}\n\n", report.total_withdrawals));
            
            pdf_content.push_str("FUND MOVEMENTS:\n");
            for movement in &report.fund_movements {
                pdf_content.push_str(&format!(
                    "{} - {} {}\n",
                    movement.timestamp, movement.movement_type, movement.amount
                ));
            }
            
            pdf_content.push_str("\nBENEFICIARY CHANGES:\n");
            for change in &report.beneficiary_changes {
                pdf_content.push_str(&format!(
                    "{} - {} -> {}\n",
                    change.timestamp, change.old_beneficiary, change.new_beneficiary
                ));
            }
            
            Ok(pdf_content)
        }
        _ => Err("Unsupported format".to_string()),
    }
}

pub fn get_vault_templates() -> VaultTemplateList {
    VaultTemplateList {
        templates: vec![
            VaultTemplate {
                id: "simple-inheritance".to_string(),
                name: "Simple Inheritance".to_string(),
                description: "Basic vault for single beneficiary inheritance".to_string(),
                check_in_interval: 86400 * 30, // 30 days
                recommended_for: "Individual asset protection".to_string(),
            },
            VaultTemplate {
                id: "family-trust".to_string(),
                name: "Family Trust".to_string(),
                description: "Multi-beneficiary vault for family wealth distribution".to_string(),
                check_in_interval: 86400 * 90, // 90 days
                recommended_for: "Family wealth management".to_string(),
            },
            VaultTemplate {
                id: "business-succession".to_string(),
                name: "Business Succession".to_string(),
                description: "Vault for business continuity and succession planning".to_string(),
                check_in_interval: 86400 * 60, // 60 days
                recommended_for: "Business asset protection".to_string(),
            },
        ],
    }
}

pub fn create_vault_from_template(
    store: &VaultStore,
    template_id: &str,
    owner: String,
    beneficiary: String,
) -> Result<Vault, String> {
    let templates = get_vault_templates();
    let template = templates
        .templates
        .iter()
        .find(|t| t.id == template_id)
        .ok_or_else(|| "Template not found".to_string())?;

    let vault_id = uuid::Uuid::new_v4().to_string();
    let vault = Vault {
        id: vault_id,
        owner,
        beneficiary,
        balance: 0,
        check_in_interval: template.check_in_interval,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(template.check_in_interval),
    };

    store.lock().unwrap().insert(vault.id.clone(), vault.clone());
    Ok(vault)
}

// ── Task 1: Analytics ────────────────────────────────────────────────────────

/// GET /analytics/vaults
pub fn get_vault_analytics_handler(store: &VaultStore) -> VaultAnalytics {
    compute_vault_analytics(store)
}

// ── Task 2: Backup & Recovery ─────────────────────────────────────────────────

/// POST /vaults/{id}/backup
/// Serialises the vault to JSON and stores it as a base64-encoded "encrypted" payload.
/// In production this would use AES-GCM; here we use base64 to keep the implementation
/// dependency-free while preserving the correct API shape.
pub fn backup_vault_handler(
    store: &VaultStore,
    backup_store: &BackupStore,
    vault_id: &str,
) -> Result<VaultBackup, String> {
    let vault = store
        .lock()
        .unwrap()
        .get(vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    let payload_json = serde_json::to_string(&vault).map_err(|e| e.to_string())?;
    // base64-encode as a stand-in for encryption
    let encrypted_payload = base64_encode(payload_json.as_bytes());

    let backup = VaultBackup {
        backup_id: uuid::Uuid::new_v4().to_string(),
        vault_id: vault_id.to_string(),
        created_at: Utc::now(),
        encrypted_payload,
    };

    store_backup(backup_store, backup.clone());
    Ok(backup)
}

/// POST /vaults/restore
pub fn restore_vault_handler(
    store: &VaultStore,
    backup_store: &BackupStore,
    request: &RestoreRequest,
) -> Result<Vault, String> {
    let backup = get_backup(backup_store, &request.backup_id)
        .ok_or_else(|| "Backup not found".to_string())?;

    let decoded = base64_decode(&backup.encrypted_payload)
        .map_err(|e| format!("Failed to decode backup: {}", e))?;

    let vault: Vault = serde_json::from_slice(&decoded)
        .map_err(|e| format!("Failed to deserialise vault: {}", e))?;

    store.lock().unwrap().insert(vault.id.clone(), vault.clone());
    Ok(vault)
}

fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let combined = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((combined >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((combined >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 { CHARS[((combined >> 6) & 0x3F) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { CHARS[(combined & 0x3F) as usize] as char } else { '=' });
    }
    out
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Result<u32, String> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err(format!("Invalid base64 char: {}", c as char)),
        }
    }
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        if chunk.len() < 4 { break; }
        let v0 = val(chunk[0])?;
        let v1 = val(chunk[1])?;
        let v2 = val(chunk[2])?;
        let v3 = val(chunk[3])?;
        let combined = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;
        out.push(((combined >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' { out.push(((combined >> 8) & 0xFF) as u8); }
        if chunk[3] != b'=' { out.push((combined & 0xFF) as u8); }
    }
    Ok(out)
}

// ── Task 3: Sharing & Collaboration ──────────────────────────────────────────

/// POST /vaults/{id}/share
pub fn share_vault_handler(
    store: &VaultStore,
    share_store: &ShareStore,
    vault_id: &str,
    request: ShareRequest,
) -> Result<VaultShare, String> {
    // Verify vault exists
    store
        .lock()
        .unwrap()
        .get(vault_id)
        .ok_or_else(|| "Vault not found".to_string())?;

    let share = VaultShare {
        share_id: uuid::Uuid::new_v4().to_string(),
        vault_id: vault_id.to_string(),
        shared_with: request.shared_with,
        permission: request.permission,
        created_at: Utc::now(),
    };

    add_vault_share(share_store, share.clone());
    Ok(share)
}

/// GET /vaults/{id}/shares  (convenience accessor used in tests)
pub fn list_vault_shares_handler(
    share_store: &ShareStore,
    vault_id: &str,
) -> Vec<VaultShare> {
    get_vault_shares(share_store, vault_id)
}

// ── Task 4: Notification Preferences ─────────────────────────────────────────

/// POST /vaults/{id}/notification-preferences
pub fn set_notification_preferences_handler(
    store: &VaultStore,
    notif_store: &NotificationStore,
    vault_id: &str,
    request: NotificationPreferencesRequest,
) -> Result<NotificationPreferences, String> {
    if request.channels.is_empty() {
        return Err("At least one notification channel is required".to_string());
    }

    // Verify vault exists
    store
        .lock()
        .unwrap()
        .get(vault_id)
        .ok_or_else(|| "Vault not found".to_string())?;

    let prefs = NotificationPreferences {
        vault_id: vault_id.to_string(),
        channels: request.channels,
        frequency: request.frequency,
        updated_at: Utc::now(),
    };

    set_notification_preferences(notif_store, prefs.clone());
    Ok(prefs)
}

/// GET /vaults/{id}/notification-preferences
pub fn get_notification_preferences_handler(
    notif_store: &NotificationStore,
    vault_id: &str,
) -> Option<NotificationPreferences> {
    get_notification_preferences(notif_store, vault_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_vaults_handler() {
        let store = create_vault_store();
        let vault = Vault {
            id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 1000,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(100000),
        };
        store.lock().unwrap().insert("v1".to_string(), vault);

        let query = SearchQuery {
            owner: Some("owner1".to_string()),
            beneficiary: None,
            status: None,
            created_after: None,
            created_before: None,
            page: None,
            limit: None,
        };

        let result = search_vaults_handler(&store, query);
        assert_eq!(result.vaults.len(), 1);
    }

    #[test]
    fn test_compare_vaults_handler() {
        let store = create_vault_store();
        let vault1 = Vault {
            id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 1000,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(100000),
        };
        let vault2 = Vault {
            id: "v2".to_string(),
            owner: "owner2".to_string(),
            beneficiary: "ben2".to_string(),
            balance: 2000,
            check_in_interval: 172800,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(200000),
        };
        store.lock().unwrap().insert("v1".to_string(), vault1);
        store.lock().unwrap().insert("v2".to_string(), vault2);

        let result = compare_vaults_handler(&store, vec!["v1".to_string(), "v2".to_string()]);
        assert_eq!(result.vaults.len(), 2);
    }

    #[test]
    fn test_export_vaults_handler_json() {
        let store = create_vault_store();
        let event_store = create_event_store();
        let audit_store = create_audit_store();

        let vault = Vault {
            id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 1000,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(100000),
        };
        store.lock().unwrap().insert("v1".to_string(), vault);

        let result = export_vaults_handler(&store, &event_store, &audit_store, "v1", "json");
        assert!(result.is_ok());
        let json_str = result.unwrap();
        assert!(json_str.contains("v1"));
    }

    #[test]
    fn test_export_vaults_handler_csv() {
        let store = create_vault_store();
        let event_store = create_event_store();
        let audit_store = create_audit_store();

        let vault = Vault {
            id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 1000,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(100000),
        };
        store.lock().unwrap().insert("v1".to_string(), vault);

        let result = export_vaults_handler(&store, &event_store, &audit_store, "v1", "csv");
        assert!(result.is_ok());
        let csv_str = result.unwrap();
        assert!(csv_str.contains("v1"));
    }

    #[test]
    fn test_generate_compliance_report() {
        let store = create_vault_store();
        let event_store = create_event_store();

        let vault = Vault {
            id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 1000,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(100000),
        };
        store.lock().unwrap().insert("v1".to_string(), vault);

        let result = generate_compliance_report(&store, &event_store, "v1");
        assert!(result.is_ok());
        let report = result.unwrap();
        assert_eq!(report.vault_id, "v1");
        assert_eq!(report.owner, "owner1");
        assert_eq!(report.current_balance, 1000);
    }

    #[test]
    fn test_export_compliance_report_json() {
        let report = ComplianceReport {
            vault_id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            report_generated_at: Utc::now(),
            fund_movements: vec![],
            beneficiary_changes: vec![],
            ttl_history: vec![],
            total_deposits: 1000,
            total_withdrawals: 0,
            current_balance: 1000,
        };

        let result = export_compliance_report(&report, "json");
        assert!(result.is_ok());
        let json_str = result.unwrap();
        assert!(json_str.contains("v1"));
    }

    #[test]
    fn test_export_compliance_report_pdf() {
        let report = ComplianceReport {
            vault_id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            report_generated_at: Utc::now(),
            fund_movements: vec![],
            beneficiary_changes: vec![],
            ttl_history: vec![],
            total_deposits: 1000,
            total_withdrawals: 0,
            current_balance: 1000,
        };

        let result = export_compliance_report(&report, "pdf");
        assert!(result.is_ok());
        let pdf_str = result.unwrap();
        assert!(pdf_str.contains("COMPLIANCE REPORT"));
        assert!(pdf_str.contains("v1"));
    }

    // ── Task 1: Analytics tests ───────────────────────────────────────────────

    #[test]
    fn test_get_vault_analytics_empty_store() {
        let store = create_vault_store();
        let analytics = get_vault_analytics_handler(&store);
        assert_eq!(analytics.total_vaults, 0);
        assert_eq!(analytics.active_vaults, 0);
        assert_eq!(analytics.release_rate, 0.0);
        assert!(analytics.time_series.is_empty());
    }

    #[test]
    fn test_get_vault_analytics_counts() {
        let store = create_vault_store();
        for i in 0..3 {
            store.lock().unwrap().insert(format!("v{}", i), Vault {
                id: format!("v{}", i),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 100,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            });
        }
        store.lock().unwrap().insert("vr".to_string(), Vault {
            id: "vr".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 0,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Released,
            ttl_remaining: None,
        });

        let analytics = get_vault_analytics_handler(&store);
        assert_eq!(analytics.total_vaults, 4);
        assert_eq!(analytics.active_vaults, 3);
        assert!((analytics.release_rate - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_vault_analytics_time_series() {
        let store = create_vault_store();
        store.lock().unwrap().insert("v1".to_string(), Vault {
            id: "v1".to_string(),
            owner: "o".to_string(),
            beneficiary: "b".to_string(),
            balance: 0,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(86400),
        });
        let analytics = get_vault_analytics_handler(&store);
        assert_eq!(analytics.time_series.len(), 1);
        assert_eq!(analytics.time_series[0].vaults_created, 1);
    }

    // ── Task 2: Backup & Recovery tests ──────────────────────────────────────

    #[test]
    fn test_backup_vault_creates_backup() {
        let store = create_vault_store();
        let backup_store = create_backup_store();
        store.lock().unwrap().insert("v1".to_string(), Vault {
            id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 500,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(86400),
        });

        let result = backup_vault_handler(&store, &backup_store, "v1");
        assert!(result.is_ok());
        let backup = result.unwrap();
        assert_eq!(backup.vault_id, "v1");
        assert!(!backup.encrypted_payload.is_empty());
    }

    #[test]
    fn test_backup_vault_not_found() {
        let store = create_vault_store();
        let backup_store = create_backup_store();
        let result = backup_vault_handler(&store, &backup_store, "missing");
        assert!(result.is_err());
    }

    #[test]
    fn test_restore_vault_from_backup() {
        let store = create_vault_store();
        let backup_store = create_backup_store();
        store.lock().unwrap().insert("v1".to_string(), Vault {
            id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 999,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(86400),
        });

        let backup = backup_vault_handler(&store, &backup_store, "v1").unwrap();

        // Remove vault then restore
        store.lock().unwrap().remove("v1");
        assert!(store.lock().unwrap().get("v1").is_none());

        let req = RestoreRequest {
            backup_id: backup.backup_id,
            encryption_key: "dummy-key".to_string(),
        };
        let restored = restore_vault_handler(&store, &backup_store, &req).unwrap();
        assert_eq!(restored.id, "v1");
        assert_eq!(restored.balance, 999);
    }

    #[test]
    fn test_restore_missing_backup_returns_error() {
        let store = create_vault_store();
        let backup_store = create_backup_store();
        let req = RestoreRequest {
            backup_id: "nonexistent".to_string(),
            encryption_key: "key".to_string(),
        };
        assert!(restore_vault_handler(&store, &backup_store, &req).is_err());
    }

    // ── Task 3: Sharing tests ─────────────────────────────────────────────────

    #[test]
    fn test_share_vault_creates_share() {
        let store = create_vault_store();
        let share_store = create_share_store();
        store.lock().unwrap().insert("v1".to_string(), Vault {
            id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 0,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(86400),
        });

        let req = ShareRequest {
            shared_with: "trusted@example.com".to_string(),
            permission: SharePermission::ViewOnly,
        };
        let result = share_vault_handler(&store, &share_store, "v1", req);
        assert!(result.is_ok());
        let share = result.unwrap();
        assert_eq!(share.vault_id, "v1");
        assert_eq!(share.permission, SharePermission::ViewOnly);
    }

    #[test]
    fn test_share_vault_not_found() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let req = ShareRequest {
            shared_with: "someone".to_string(),
            permission: SharePermission::Edit,
        };
        assert!(share_vault_handler(&store, &share_store, "missing", req).is_err());
    }

    #[test]
    fn test_list_vault_shares() {
        let store = create_vault_store();
        let share_store = create_share_store();
        store.lock().unwrap().insert("v1".to_string(), Vault {
            id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 0,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(86400),
        });

        share_vault_handler(&store, &share_store, "v1", ShareRequest {
            shared_with: "a@example.com".to_string(),
            permission: SharePermission::ViewOnly,
        }).unwrap();
        share_vault_handler(&store, &share_store, "v1", ShareRequest {
            shared_with: "b@example.com".to_string(),
            permission: SharePermission::Admin,
        }).unwrap();

        let shares = list_vault_shares_handler(&share_store, "v1");
        assert_eq!(shares.len(), 2);
    }

    #[test]
    fn test_share_permission_levels() {
        let store = create_vault_store();
        let share_store = create_share_store();
        store.lock().unwrap().insert("v1".to_string(), Vault {
            id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 0,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(86400),
        });

        for perm in [SharePermission::ViewOnly, SharePermission::Edit, SharePermission::Admin] {
            let req = ShareRequest { shared_with: "x".to_string(), permission: perm.clone() };
            let share = share_vault_handler(&store, &share_store, "v1", req).unwrap();
            assert_eq!(share.permission, perm);
        }
    }

    // ── Task 4: Notification Preferences tests ────────────────────────────────

    #[test]
    fn test_set_notification_preferences() {
        let store = create_vault_store();
        let notif_store = create_notification_store();
        store.lock().unwrap().insert("v1".to_string(), Vault {
            id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 0,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(86400),
        });

        let req = NotificationPreferencesRequest {
            channels: vec![NotificationChannel::Email, NotificationChannel::Push],
            frequency: NotificationFrequency::Weekly,
        };
        let result = set_notification_preferences_handler(&store, &notif_store, "v1", req);
        assert!(result.is_ok());
        let prefs = result.unwrap();
        assert_eq!(prefs.vault_id, "v1");
        assert_eq!(prefs.frequency, NotificationFrequency::Weekly);
        assert!(prefs.channels.contains(&NotificationChannel::Email));
    }

    #[test]
    fn test_get_notification_preferences() {
        let store = create_vault_store();
        let notif_store = create_notification_store();
        store.lock().unwrap().insert("v1".to_string(), Vault {
            id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 0,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(86400),
        });

        set_notification_preferences_handler(&store, &notif_store, "v1", NotificationPreferencesRequest {
            channels: vec![NotificationChannel::Sms],
            frequency: NotificationFrequency::Daily,
        }).unwrap();

        let prefs = get_notification_preferences_handler(&notif_store, "v1");
        assert!(prefs.is_some());
        assert_eq!(prefs.unwrap().frequency, NotificationFrequency::Daily);
    }

    #[test]
    fn test_notification_preferences_vault_not_found() {
        let store = create_vault_store();
        let notif_store = create_notification_store();
        let req = NotificationPreferencesRequest {
            channels: vec![NotificationChannel::Email],
            frequency: NotificationFrequency::Monthly,
        };
        assert!(set_notification_preferences_handler(&store, &notif_store, "missing", req).is_err());
    }

    #[test]
    fn test_notification_preferences_empty_channels_rejected() {
        let store = create_vault_store();
        let notif_store = create_notification_store();
        store.lock().unwrap().insert("v1".to_string(), Vault {
            id: "v1".to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 0,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(86400),
        });
        let req = NotificationPreferencesRequest {
            channels: vec![],
            frequency: NotificationFrequency::Daily,
        };
        assert!(set_notification_preferences_handler(&store, &notif_store, "v1", req).is_err());
    }
}
