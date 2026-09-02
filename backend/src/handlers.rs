use crate::db::*;
use crate::error::AppError;
use crate::models::*;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::{DateTime, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tracing::instrument;

pub fn search_vaults_handler(store: &VaultStore, query: SearchQuery) -> SearchResult {
    search_vaults(store, &query)
}

pub fn compare_vaults_handler(store: &VaultStore, vault_ids: Vec<String>) -> ComparisonResult {
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
    from: Option<chrono::DateTime<chrono::Utc>>,
    to: Option<chrono::DateTime<chrono::Utc>>,
    user_id: Option<&str>,
) -> Result<String, String> {
    let vaults = store.lock().unwrap();
    let vault = vaults
        .get(vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    if let Some(uid) = user_id {
        if vault.owner != uid {
            return Err("Forbidden: not vault owner".to_string());
        }
    }

    let history = get_vault_history(event_store, vault_id);
    let audit_log = get_vault_audit_log(audit_store, vault_id);

    let filtered_history: Vec<VaultEvent> = history
        .into_iter()
        .filter(|e| {
            if let Some(from) = from {
                if e.timestamp < from {
                    return false;
                }
            }
            if let Some(to) = to {
                if e.timestamp > to {
                    return false;
                }
            }
            true
        })
        .collect();

    let filtered_audit: Vec<AuditEntry> = audit_log
        .into_iter()
        .filter(|a| {
            if let Some(from) = from {
                if a.timestamp < from {
                    return false;
                }
            }
            if let Some(to) = to {
                if a.timestamp > to {
                    return false;
                }
            }
            true
        })
        .collect();

    let export_data = ExportData {
        vault,
        history: filtered_history,
        audit_log: filtered_audit,
    };

    match format {
        "json" => Ok(serde_json::to_string_pretty(&export_data).map_err(|e| e.to_string())?),
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
                    if let Some(new_ben) =
                        event.data.get("new_beneficiary").and_then(|v| v.as_str())
                    {
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

pub fn export_compliance_report(report: &ComplianceReport, format: &str) -> Result<String, String> {
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
            pdf_content.push_str(&format!(
                "Total Withdrawals: {}\n\n",
                report.total_withdrawals
            ));

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

    store
        .lock()
        .unwrap()
        .insert(vault.id.clone(), vault.clone());
    Ok(vault)
}

// ── Task 1: Analytics ────────────────────────────────────────────────────────

/// GET /analytics/vaults
pub fn get_vault_analytics_handler(store: &VaultStore) -> VaultAnalytics {
    compute_vault_analytics(store)
}

/// GET /api/vaults/{id}/analytics
pub fn get_vault_detail_analytics_handler(
    store: &VaultStore,
    event_store: &EventStore,
    vault_id: &str,
) -> Result<VaultDetailAnalytics, String> {
    let vaults = store.lock().unwrap();
    let vault = vaults
        .get(vault_id)
        .ok_or_else(|| "Vault not found".to_string())?;

    let history = get_vault_history(event_store, vault_id);

    // TTL history: last 30 days of TTL-related events
    let thirty_days_ago = Utc::now() - chrono::Duration::days(30);
    let mut ttl_history: Vec<TtlHistoryPoint> = history
        .iter()
        .filter(|e| {
            e.timestamp >= thirty_days_ago
                && matches!(
                    e.event_type,
                    EventType::TtlUpdate | EventType::CheckIn | EventType::StatusChange
                )
        })
        .map(|e| TtlHistoryPoint {
            date: e.timestamp.format("%Y-%m-%d").to_string(),
            ttl_remaining_seconds: e
                .data
                .get("ttl_remaining")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            event: format!("{:?}", e.event_type),
        })
        .collect();

    // If no TTL events in last 30 days, add current state
    if ttl_history.is_empty() {
        ttl_history.push(TtlHistoryPoint {
            date: Utc::now().format("%Y-%m-%d").to_string(),
            ttl_remaining_seconds: vault.ttl_remaining.unwrap_or(0),
            event: "current_state".to_string(),
        });
    }

    // Check-in frequency
    let check_ins: Vec<&VaultEvent> = history
        .iter()
        .filter(|e| matches!(e.event_type, EventType::CheckIn))
        .collect();

    let total_check_ins = check_ins.len() as u64;
    let avg_interval = if total_check_ins > 1 {
        let first = check_ins.first().map(|e| e.timestamp).unwrap_or(Utc::now());
        let last = check_ins.last().map(|e| e.timestamp).unwrap_or(Utc::now());
        let span_seconds = (last - first).num_seconds().max(1) as u64;
        span_seconds / (total_check_ins - 1).max(1)
    } else {
        vault.check_in_interval
    };

    let next_deadline =
        vault.last_check_in + chrono::Duration::seconds(vault.check_in_interval as i64);
    let days_until_deadline = (next_deadline - Utc::now()).num_seconds() / 86400;

    let check_in_frequency = CheckInFrequency {
        average_interval_seconds: avg_interval,
        total_check_ins,
        next_deadline: next_deadline.to_rfc3339(),
        days_until_deadline,
    };

    // Withdrawal trends
    let withdrawals: Vec<&VaultEvent> = history
        .iter()
        .filter(|e| matches!(e.event_type, EventType::Withdrawal))
        .collect();

    let withdrawal_count = withdrawals.len() as u64;
    let total_withdrawals: i128 = withdrawals
        .iter()
        .filter_map(|e| e.data.get("amount").and_then(|v| v.as_i64()))
        .map(|v| v as i128)
        .sum();

    let average_withdrawal_amount = if withdrawal_count > 0 {
        total_withdrawals as f64 / withdrawal_count as f64
    } else {
        0.0
    };

    let last_withdrawal_date = withdrawals
        .last()
        .map(|e| e.timestamp.format("%Y-%m-%d").to_string());

    let withdrawal_trends = WithdrawalTrends {
        total_withdrawals,
        withdrawal_count,
        average_withdrawal_amount,
        last_withdrawal_date,
    };

    // Beneficiary status
    let beneficiary_status = BeneficiaryStatus {
        beneficiary_address: vault.beneficiary.clone(),
        is_active: vault.status == VaultStatus::Active,
        vault_status: format!("{:?}", vault.status),
        can_receive_funds: vault.status == VaultStatus::Released
            || vault.status == VaultStatus::Active,
    };

    Ok(VaultDetailAnalytics {
        vault_id: vault.id.clone(),
        ttl_history,
        check_in_frequency,
        withdrawal_trends,
        beneficiary_status,
    })
}

// ── Task 2: Backup & Recovery ─────────────────────────────────────────────────

/// POST /vaults/{id}/backup
/// Serialises the vault to JSON and stores it as a base64-encoded "encrypted" payload.
/// In production this would use AES-GCM; here we use base64 to keep the implementation
/// dependency-free while preserving the correct API shape.
///
/// Instrumented with an OpenTelemetry span (Issue #1145).
#[instrument(skip(store, backup_store), fields(vault_id = %vault_id))]
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
///
/// Instrumented with an OpenTelemetry span (Issue #1145).
#[instrument(skip(store, backup_store), fields(backup_id = %request.backup_id))]
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

    store
        .lock()
        .unwrap()
        .insert(vault.id.clone(), vault.clone());
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
        out.push(if chunk.len() > 1 {
            CHARS[((combined >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            CHARS[(combined & 0x3F) as usize] as char
        } else {
            '='
        });
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
        if chunk.len() < 4 {
            break;
        }
        let v0 = val(chunk[0])?;
        let v1 = val(chunk[1])?;
        let v2 = val(chunk[2])?;
        let v3 = val(chunk[3])?;
        let combined = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;
        out.push(((combined >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' {
            out.push(((combined >> 8) & 0xFF) as u8);
        }
        if chunk[3] != b'=' {
            out.push((combined & 0xFF) as u8);
        }
    }
    Ok(out)
}

// ── Task 3: Sharing & Collaboration ──────────────────────────────────────────

const DEFAULT_TOKEN_EXPIRY_SECONDS: u64 = 604800; // 7 days

/// POST /vaults/{id}/share
pub fn share_vault_handler(
    store: &VaultStore,
    share_store: &ShareStore,
    token_store: &ShareTokenStore,
    audit_store: &AuditStore,
    vault_id: &str,
    request: ShareRequest,
) -> Result<VaultShare, String> {
    // Verify vault exists
    let vault = store
        .lock()
        .unwrap()
        .get(vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    let share = VaultShare {
        share_id: uuid::Uuid::new_v4().to_string(),
        vault_id: vault_id.to_string(),
        shared_with: request.shared_with.clone(),
        permission: request.permission,
        created_at: Utc::now(),
    };

    add_vault_share(share_store, share.clone());

    // Audit log
    append_audit_entry(
        audit_store,
        "vault_shared",
        &vault.owner,
        serde_json::json!({
            "vault_id": vault_id,
            "share_id": share.share_id,
            "shared_with": request.shared_with,
            "permission": share.permission,
        }),
    );

    Ok(share)
}

/// POST /vaults/{id}/share/tokens
pub fn generate_share_token_handler(
    store: &VaultStore,
    share_store: &ShareStore,
    token_store: &ShareTokenStore,
    audit_store: &AuditStore,
    vault_id: &str,
    request: GenerateTokenRequest,
) -> Result<ShareTokenResponse, String> {
    let vault = store
        .lock()
        .unwrap()
        .get(vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    let permission = request.permission.unwrap_or(SharePermission::ViewOnly);
    let expires_at = Utc::now()
        + chrono::Duration::seconds(
            request
                .expiry_seconds
                .unwrap_or(DEFAULT_TOKEN_EXPIRY_SECONDS) as i64,
        );

    // Create a VaultShare entry (reuses existing share infrastructure)
    let share = VaultShare {
        share_id: uuid::Uuid::new_v4().to_string(),
        vault_id: vault_id.to_string(),
        shared_with: request.shared_with.clone(),
        permission: permission.clone(),
        created_at: Utc::now(),
    };
    add_vault_share(share_store, share.clone());

    // Generate the access token
    let token = ShareToken {
        token: uuid::Uuid::new_v4().to_string(),
        share_id: share.share_id.clone(),
        vault_id: vault_id.to_string(),
        shared_with: request.shared_with,
        permission,
        created_at: Utc::now(),
        expires_at,
        revoked: false,
    };
    add_share_token(token_store, token.clone());

    let access_url = format!("/api/shared/vaults/{}", token.token);

    // Audit log
    append_audit_entry(
        audit_store,
        "share_token_generated",
        &vault.owner,
        serde_json::json!({
            "vault_id": vault_id,
            "share_id": share.share_id,
            "token": token.token,
            "expires_at": token.expires_at,
        }),
    );

    Ok(ShareTokenResponse {
        share,
        token,
        access_url,
    })
}

/// POST /vaults/{id}/share/tokens/revoke
pub fn revoke_share_token_handler(
    store: &VaultStore,
    token_store: &ShareTokenStore,
    audit_store: &AuditStore,
    vault_id: &str,
    request: RevokeTokenRequest,
) -> Result<ShareToken, String> {
    let vault = store
        .lock()
        .unwrap()
        .get(vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    let token = revoke_share_token(token_store, &request.token)
        .ok_or_else(|| "Share token not found".to_string())?;

    if token.vault_id != vault_id {
        return Err("Token does not belong to this vault".to_string());
    }

    append_audit_entry(
        audit_store,
        "share_token_revoked",
        &vault.owner,
        serde_json::json!({
            "vault_id": vault_id,
            "token": token.token,
            "share_id": token.share_id,
        }),
    );

    Ok(token)
}

/// GET /vaults/{id}/share/tokens
pub fn list_share_tokens_handler(token_store: &ShareTokenStore, vault_id: &str) -> Vec<ShareToken> {
    get_vault_share_tokens(token_store, vault_id)
}

// ── Read-only access via share token ─────────────────────────────────────────

/// GET /shared/vaults/{token}
pub fn access_vault_via_share_handler(
    store: &VaultStore,
    token_store: &ShareTokenStore,
    audit_store: &AuditStore,
    token: &str,
) -> Result<Vault, String> {
    let share_token = validate_share_token(token_store, token)?;

    let vault = store
        .lock()
        .unwrap()
        .get(&share_token.vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    append_audit_entry(
        audit_store,
        "vault_accessed_via_share",
        &share_token.shared_with,
        serde_json::json!({
            "vault_id": share_token.vault_id,
            "token": token,
        }),
    );

    Ok(vault)
}

/// GET /shared/vaults/{token}/export
pub fn access_vault_export_via_share_handler(
    store: &VaultStore,
    event_store: &EventStore,
    audit_store: &AuditStore,
    token_store: &ShareTokenStore,
    token: &str,
    format: &str,
) -> Result<String, String> {
    let share_token = validate_share_token(token_store, token)?;

    let vault = store
        .lock()
        .unwrap()
        .get(&share_token.vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    let history = get_vault_history(event_store, &share_token.vault_id);
    let audit_log = get_vault_audit_log(audit_store, &share_token.vault_id);

    let export_data = ExportData {
        vault,
        history,
        audit_log,
    };

    append_audit_entry(
        audit_store,
        "vault_exported_via_share",
        &share_token.shared_with,
        serde_json::json!({
            "vault_id": share_token.vault_id,
            "token": token,
            "format": format,
        }),
    );

    match format {
        "json" => Ok(serde_json::to_string_pretty(&export_data).map_err(|e| e.to_string())?),
        "csv" => export_to_csv(&export_data),
        _ => Err("Unsupported format".to_string()),
    }
}

fn validate_share_token(token_store: &ShareTokenStore, token: &str) -> Result<ShareToken, String> {
    let share_token =
        get_share_token(token_store, token).ok_or_else(|| "Invalid share token".to_string())?;

    if share_token.revoked {
        return Err("Share token has been revoked".to_string());
    }

    if Utc::now() > share_token.expires_at {
        return Err("Share token has expired".to_string());
    }

    if share_token.permission != SharePermission::ViewOnly {
        return Err("Share token does not have ViewOnly permission".to_string());
    }

    Ok(share_token)
}

/// GET /vaults/{id}/shares  (convenience accessor used in tests)
pub fn list_vault_shares_handler(share_store: &ShareStore, vault_id: &str) -> Vec<VaultShare> {
    get_vault_shares(share_store, vault_id)
}

// ── Task 4: Notification Preferences ─────────────────────────────────────────

/// POST /vaults/{id}/notification-preferences
///
/// Instrumented with an OpenTelemetry span (Issue #1145).
#[instrument(skip(store, notif_store), fields(vault_id = %vault_id))]
pub fn set_notification_preferences_handler(
    store: &VaultStore,
    notif_store: &NotificationStore,
    vault_id: &str,
    request: NotificationPreferencesRequest,
) -> Result<VaultNotificationPreferences, String> {
    if request.channels.is_empty() {
        return Err("At least one notification channel is required".to_string());
    }

    // Verify vault exists
    store
        .lock()
        .unwrap()
        .get(vault_id)
        .ok_or_else(|| "Vault not found".to_string())?;

    // Map HTTP channels into legacy boolean flags.
    let preferred = request.channels.first().cloned();
    let fallback = request.channels.get(1).cloned();
    let prefs = NotificationPreferences {
        owner: vault_id.to_string(),
        expiry_warning_enabled: request
            .channels
            .iter()
            .any(|c| matches!(c, NotificationChannel::Email | NotificationChannel::Push)),
        check_in_reminder_enabled: request
            .channels
            .iter()
            .any(|c| matches!(c, NotificationChannel::Sms | NotificationChannel::Push)),
        vault_released_enabled: request
            .channels
            .iter()
            .any(|c| matches!(c, NotificationChannel::Push)),
        warning_hours_before: 24,
        locale: None,
        preferred_channel: preferred,
        fallback_channel: fallback,
        unsubscribed: false,
    };

    set_notification_preferences(notif_store, prefs.clone());
    Ok(prefs)
}

/// GET /vaults/{id}/notification-preferences
pub fn get_notification_preferences_handler(
    notif_store: &NotificationStore,
    vault_id: &str,
) -> Option<VaultNotificationPreferences> {
    get_notification_preferences(notif_store, vault_id)
}

// ── Issue #1099: Vault Health Score ──────────────────────────────────────────

/// Healthy balance threshold in the same units as `Vault::balance`.
/// Vaults at or above this level score full points for the balance factor.
const HEALTH_BALANCE_THRESHOLD: i128 = 1_000;

/// Number of consecutive check-ins needed to earn full streak points.
const HEALTH_STREAK_FULL: u32 = 5;

/// Number of passkeys needed to earn full passkey-diversity points.
const HEALTH_PASSKEY_FULL: u32 = 3;

/// Cache TTL: 5 minutes in seconds.
const HEALTH_CACHE_TTL_SECS: i64 = 300;

/// In-memory health score cache entry.
#[derive(Clone)]
struct HealthCacheEntry {
    response: VaultHealthResponse,
    expires_at: chrono::DateTime<Utc>,
}

/// A simple in-memory cache for vault health scores.
pub type HealthCache = Arc<Mutex<HashMap<String, HealthCacheEntry>>>;

pub fn create_health_cache() -> HealthCache {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Invalidates any cached health score for `vault_id`.
/// Call this after a check-in or deposit that changes health inputs.
pub fn invalidate_health_cache(cache: &HealthCache, vault_id: &str) {
    cache.lock().unwrap().remove(vault_id);
}

/// `GET /api/vaults/{id}/health`
///
/// Returns a 0-100 health score with a per-factor breakdown:
/// - TTL buffer   30 pts — ratio of `ttl_remaining` to `check_in_interval`
/// - Streak       20 pts — consecutive check-ins (full at `HEALTH_STREAK_FULL`)
/// - Balance      30 pts — balance relative to `HEALTH_BALANCE_THRESHOLD`
/// - Passkey div. 20 pts — number of distinct passkeys (full at `HEALTH_PASSKEY_FULL`)
///
/// Results are cached for 5 minutes and invalidated on check-in or deposit.
pub fn get_vault_health_handler(
    store: &VaultStore,
    cache: &HealthCache,
    vault_id: &str,
    // Streak value sourced from external context (0 if unavailable).
    streak: u32,
    // Number of registered passkeys (0 if unavailable).
    passkey_count: u32,
) -> Result<VaultHealthResponse, String> {
    // Return cached result if still fresh
    {
        let cache_lock = cache.lock().unwrap();
        if let Some(entry) = cache_lock.get(vault_id) {
            if Utc::now() < entry.expires_at {
                return Ok(entry.response.clone());
            }
        }
    }

    let vault = store
        .lock()
        .unwrap()
        .get(vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    // ── TTL buffer factor (0-30) ──────────────────────────────────────────
    let ttl_score: u8 = match vault.ttl_remaining {
        Some(ttl) if vault.check_in_interval > 0 => {
            let ratio = ttl as f64 / vault.check_in_interval as f64;
            (ratio.min(1.0) * 30.0).round() as u8
        }
        _ => 0,
    };

    // ── Streak factor (0-20) ──────────────────────────────────────────────
    let streak_score: u8 = {
        let ratio = (streak as f64) / (HEALTH_STREAK_FULL as f64);
        (ratio.min(1.0) * 20.0).round() as u8
    };

    // ── Balance factor (0-30) ─────────────────────────────────────────────
    let balance_score: u8 = if HEALTH_BALANCE_THRESHOLD > 0 {
        let ratio = vault.balance as f64 / HEALTH_BALANCE_THRESHOLD as f64;
        (ratio.min(1.0) * 30.0).round() as u8
    } else {
        0
    };

    // ── Passkey diversity factor (0-20) ───────────────────────────────────
    let passkey_score: u8 = {
        let ratio = (passkey_count as f64) / (HEALTH_PASSKEY_FULL as f64);
        (ratio.min(1.0) * 20.0).round() as u8
    };

    let score = ttl_score
        .saturating_add(streak_score)
        .saturating_add(balance_score)
        .saturating_add(passkey_score);

    let response = VaultHealthResponse {
        score,
        factors: HealthFactors {
            ttl_buffer: ttl_score,
            streak: streak_score,
            balance: balance_score,
            passkey_diversity: passkey_score,
        },
        computed_at: Utc::now(),
    };

    // Store in cache
    cache.lock().unwrap().insert(
        vault_id.to_string(),
        HealthCacheEntry {
            response: response.clone(),
            expires_at: Utc::now() + chrono::Duration::seconds(HEALTH_CACHE_TTL_SECS),
        },
    );

    Ok(response)
}

// ── Issue #1100: Bulk Vault Summary ──────────────────────────────────────────

/// Maximum vault IDs accepted per bulk request.
pub const BULK_SUMMARY_MAX_IDS: usize = 20;

/// Rate-limit window in seconds.
const BULK_RATE_WINDOW_SECS: i64 = 60;

/// Maximum bulk requests per user per window.
const BULK_RATE_LIMIT: u32 = 10;

/// Per-user rate-limit state.
#[derive(Clone)]
struct BulkRateEntry {
    count: u32,
    window_start: chrono::DateTime<Utc>,
}

/// Shared rate-limit state for bulk summary requests.
pub type BulkRateStore = Arc<Mutex<HashMap<String, BulkRateEntry>>>;

pub fn create_bulk_rate_store() -> BulkRateStore {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Returns `true` if `user_id` is within their rate limit, recording the call.
/// Returns `false` if the limit is exceeded.
fn check_bulk_rate_limit(rate_store: &BulkRateStore, user_id: &str) -> bool {
    let mut store = rate_store.lock().unwrap();
    let now = Utc::now();
    let entry = store.entry(user_id.to_string()).or_insert(BulkRateEntry {
        count: 0,
        window_start: now,
    });
    // Reset window if expired
    if now.signed_duration_since(entry.window_start).num_seconds() >= BULK_RATE_WINDOW_SECS {
        entry.count = 0;
        entry.window_start = now;
    }
    if entry.count >= BULK_RATE_LIMIT {
        return false;
    }
    entry.count += 1;
    true
}

/// `POST /api/vaults/bulk-summary`
///
/// Accepts up to `BULK_SUMMARY_MAX_IDS` vault IDs and returns a summary for each.
/// Missing vaults produce a null-field entry rather than failing the entire request.
/// Rate-limited to `BULK_RATE_LIMIT` requests per `user_id` per minute.
pub fn bulk_vault_summary_handler(
    store: &VaultStore,
    rate_store: &BulkRateStore,
    request: &BulkSummaryRequest,
    user_id: &str,
) -> Result<BulkSummaryResponse, String> {
    if !check_bulk_rate_limit(rate_store, user_id) {
        return Err("Rate limit exceeded: max 10 bulk requests per minute".to_string());
    }
    if request.vault_ids.len() > BULK_SUMMARY_MAX_IDS {
        return Err(format!(
            "Too many vault IDs: max {} allowed",
            BULK_SUMMARY_MAX_IDS
        ));
    }

    let vaults = store.lock().unwrap();
    let summaries = request
        .vault_ids
        .iter()
        .map(|id| match vaults.get(id) {
            Some(v) => VaultSummaryEntry {
                vault_id: id.clone(),
                status: Some(v.status.clone()),
                ttl_remaining: v.ttl_remaining,
                balance: Some(v.balance),
                last_check_in: Some(v.last_check_in),
            },
            None => VaultSummaryEntry {
                vault_id: id.clone(),
                status: None,
                ttl_remaining: None,
                balance: None,
                last_check_in: None,
            },
        })
        .collect();

    Ok(BulkSummaryResponse { summaries })
}

// ── Passkey Recovery Flow Handlers (#1299) ─────────────────────────────────

use crate::models::{
    GenerateRecoveryCodesRequest, GenerateRecoveryCodesResponse, Passkey, RecoveryCodeSet,
    RecoveryMethod, RecoveryRequest, RecoveryResponse, RegisterPasskeyRequest,
    RegisterPasskeyResponse,
};
use sha2::{Digest, Sha256};

fn generate_recovery_codes(count: usize) -> Vec<String> {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();

    (0..count)
        .map(|_| {
            (0..6)
                .map(|_| {
                    let idx = rng.gen_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect()
        })
        .collect()
}

fn hash_code(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn verify_code(code: &str, hash: &str) -> bool {
    hash_code(code) == hash
}

pub async fn register_passkey_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterPasskeyRequest>,
) -> Result<(StatusCode, Json<RegisterPasskeyResponse>), AppError> {
    let passkey_id = uuid::Uuid::new_v4().to_string();
    let passkey = Passkey {
        passkey_id: passkey_id.clone(),
        owner: body.owner.clone(),
        vault_id: body.vault_id.clone(),
        credential_id: body.credential_id,
        device_name: body.device_name.clone(),
        registered_at: Utc::now(),
        last_used: None,
        is_backup: body.is_backup.unwrap_or(false),
    };

    let mut store = state.passkey_store.lock().unwrap();
    store.push(passkey);
    drop(store);

    audit::log_action(
        &state.audit_store,
        "register_passkey",
        &body.owner,
        serde_json::json!({
            "vault_id": body.vault_id,
            "device_name": body.device_name,
            "is_backup": body.is_backup.unwrap_or(false),
        }),
    );

    Ok((
        StatusCode::CREATED,
        Json(RegisterPasskeyResponse {
            passkey_id,
            vault_id: body.vault_id,
            device_name: body.device_name,
            registered_at: Utc::now(),
        }),
    ))
}

pub async fn generate_recovery_codes_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GenerateRecoveryCodesRequest>,
) -> Result<Json<GenerateRecoveryCodesResponse>, AppError> {
    let codes = generate_recovery_codes(10);
    let set_id = uuid::Uuid::new_v4().to_string();

    let code_set = RecoveryCodeSet {
        set_id: set_id.clone(),
        owner: body.owner.clone(),
        vault_id: body.vault_id.clone(),
        codes: codes.clone(),
        generated_at: Utc::now(),
        codes_used: 0,
        total_codes: codes.len() as u32,
    };

    let mut recovery_sets = state.recovery_code_set_store.lock().unwrap();
    recovery_sets.push(code_set);
    drop(recovery_sets);

    let mut recovery_codes = state.recovery_code_store.lock().unwrap();
    for (i, code) in codes.iter().enumerate() {
        let code_id = uuid::Uuid::new_v4().to_string();
        let recovery_code = crate::models::RecoveryCode {
            code_id,
            owner: body.owner.clone(),
            vault_id: body.vault_id.clone(),
            code_hash: hash_code(code),
            generated_at: Utc::now(),
            used_at: None,
        };
        recovery_codes.push(recovery_code);
    }
    drop(recovery_codes);

    audit::log_action(
        &state.audit_store,
        "generate_recovery_codes",
        &body.owner,
        serde_json::json!({
            "vault_id": body.vault_id,
            "count": codes.len(),
        }),
    );

    Ok(Json(GenerateRecoveryCodesResponse {
        set_id,
        vault_id: body.vault_id,
        recovery_codes: codes,
        generated_at: Utc::now(),
        note: "Store these codes in a safe place. Each code can only be used once.".to_string(),
    }))
}

pub async fn recover_with_credential_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RecoveryRequest>,
) -> Result<Json<RecoveryResponse>, AppError> {
    let recovery_id = uuid::Uuid::new_v4().to_string();

    match body.recovery_method {
        RecoveryMethod::BackupPasskey => {
            let store = state.passkey_store.lock().unwrap();
            let passkey = store
                .iter()
                .find(|pk| {
                    pk.owner == body.owner
                        && pk.vault_id == body.vault_id
                        && pk.is_backup
                        && pk.credential_id == body.recovery_credential
                })
                .ok_or(AppError::NotFound("Backup passkey not found".to_string()))?;

            let passkey_id = passkey.passkey_id.clone();
            drop(store);

            let mut store = state.passkey_store.lock().unwrap();
            if let Some(pk) = store.iter_mut().find(|pk| pk.passkey_id == passkey_id) {
                pk.last_used = Some(Utc::now());
            }
            drop(store);

            audit::log_action(
                &state.audit_store,
                "recovery_backup_passkey",
                &body.owner,
                serde_json::json!({
                    "vault_id": body.vault_id,
                    "recovery_id": recovery_id,
                }),
            );
        }
        RecoveryMethod::RecoveryCode => {
            let mut recovery_codes = state.recovery_code_store.lock().unwrap();
            let code_entry = recovery_codes
                .iter_mut()
                .find(|rc| {
                    rc.owner == body.owner
                        && rc.vault_id == body.vault_id
                        && verify_code(&body.recovery_credential, &rc.code_hash)
                        && rc.used_at.is_none()
                })
                .ok_or(AppError::NotFound(
                    "Invalid or expired recovery code".to_string(),
                ))?;

            code_entry.used_at = Some(Utc::now());
            drop(recovery_codes);

            audit::log_action(
                &state.audit_store,
                "recovery_code_used",
                &body.owner,
                serde_json::json!({
                    "vault_id": body.vault_id,
                    "recovery_id": recovery_id,
                }),
            );
        }
    }

    Ok(Json(RecoveryResponse {
        recovery_id,
        vault_id: body.vault_id,
        owner: body.owner,
        recovery_method: body.recovery_method,
        authenticated_at: Utc::now(),
    }))
}

pub async fn list_passkeys_handler(
    State(state): State<Arc<AppState>>,
    Path((vault_id, owner)): Path<(String, String)>,
) -> Result<Json<Vec<Passkey>>, AppError> {
    let store = state.passkey_store.lock().unwrap();
    let passkeys: Vec<Passkey> = store
        .iter()
        .filter(|pk| pk.vault_id == vault_id && pk.owner == owner)
        .cloned()
        .collect();
    Ok(Json(passkeys))
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

        let result = export_vaults_handler(
            &store,
            &event_store,
            &audit_store,
            "v1",
            "json",
            None,
            None,
            None,
        );
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

        let result = export_vaults_handler(
            &store,
            &event_store,
            &audit_store,
            "v1",
            "csv",
            None,
            None,
            None,
        );
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
            store.lock().unwrap().insert(
                format!("v{}", i),
                Vault {
                    id: format!("v{}", i),
                    owner: "owner1".to_string(),
                    beneficiary: "ben1".to_string(),
                    balance: 100,
                    check_in_interval: 86400,
                    last_check_in: Utc::now(),
                    created_at: Utc::now(),
                    status: VaultStatus::Active,
                    ttl_remaining: Some(86400),
                },
            );
        }
        store.lock().unwrap().insert(
            "vr".to_string(),
            Vault {
                id: "vr".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 0,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Released,
                ttl_remaining: None,
            },
        );

        let analytics = get_vault_analytics_handler(&store);
        assert_eq!(analytics.total_vaults, 4);
        assert_eq!(analytics.active_vaults, 3);
        assert!((analytics.release_rate - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_vault_analytics_time_series() {
        let store = create_vault_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "o".to_string(),
                beneficiary: "b".to_string(),
                balance: 0,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );
        let analytics = get_vault_analytics_handler(&store);
        assert_eq!(analytics.time_series.len(), 1);
        assert_eq!(analytics.time_series[0].vaults_created, 1);
    }

    // ── Task 2: Backup & Recovery tests ──────────────────────────────────────

    #[test]
    fn test_backup_vault_creates_backup() {
        let store = create_vault_store();
        let backup_store = create_backup_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 500,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

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
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 999,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

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
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 0,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        let req = ShareRequest {
            shared_with: "trusted@example.com".to_string(),
            permission: SharePermission::ViewOnly,
        };
        let result =
            share_vault_handler(&store, &share_store, &token_store, &audit_store, "v1", req);
        assert!(result.is_ok());
        let share = result.unwrap();
        assert_eq!(share.vault_id, "v1");
        assert_eq!(share.permission, SharePermission::ViewOnly);

        // Verify audit entry created
        assert!(audit_store
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.action == "vault_shared"));
    }

    #[test]
    fn test_share_vault_not_found() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        let req = ShareRequest {
            shared_with: "someone".to_string(),
            permission: SharePermission::Edit,
        };
        assert!(share_vault_handler(
            &store,
            &share_store,
            &token_store,
            &audit_store,
            "missing",
            req
        )
        .is_err());
    }

    #[test]
    fn test_list_vault_shares() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 0,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        share_vault_handler(
            &store,
            &share_store,
            &token_store,
            &audit_store,
            "v1",
            ShareRequest {
                shared_with: "a@example.com".to_string(),
                permission: SharePermission::ViewOnly,
            },
        )
        .unwrap();
        share_vault_handler(
            &store,
            &share_store,
            &token_store,
            &audit_store,
            "v1",
            ShareRequest {
                shared_with: "b@example.com".to_string(),
                permission: SharePermission::Admin,
            },
        )
        .unwrap();

        let shares = list_vault_shares_handler(&share_store, "v1");
        assert_eq!(shares.len(), 2);
    }

    #[test]
    fn test_share_permission_levels() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 0,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        for perm in [
            SharePermission::ViewOnly,
            SharePermission::Edit,
            SharePermission::Admin,
        ] {
            let req = ShareRequest {
                shared_with: "x".to_string(),
                permission: perm.clone(),
            };
            let share =
                share_vault_handler(&store, &share_store, &token_store, &audit_store, "v1", req)
                    .unwrap();
            assert_eq!(share.permission, perm);
        }
    }

    // ── Share token handler tests (#966) ──────────────────────────────────────

    #[test]
    fn test_generate_share_token_creates_token() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 1000,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        let result = generate_share_token_handler(
            &store,
            &share_store,
            &token_store,
            &audit_store,
            "v1",
            GenerateTokenRequest {
                shared_with: "family@example.com".to_string(),
                permission: None,
                expiry_seconds: Some(3600),
            },
        );
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.share.vault_id, "v1");
        assert_eq!(resp.token.permission, SharePermission::ViewOnly);
        assert_eq!(resp.token.revoked, false);
        assert!(resp.access_url.contains(&resp.token.token));

        // Verify persistence
        let stored = get_share_token(&token_store, &resp.token.token);
        assert!(stored.is_some());
        assert!(!stored.unwrap().revoked);
    }

    #[test]
    fn test_generate_share_token_vault_not_found() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        let result = generate_share_token_handler(
            &store,
            &share_store,
            &token_store,
            &audit_store,
            "nonexistent",
            GenerateTokenRequest {
                shared_with: "x@example.com".to_string(),
                permission: None,
                expiry_seconds: None,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_revoke_share_token_revokes() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 100,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        // Seed a token
        add_share_token(
            &token_store,
            ShareToken {
                token: "tok-1".to_string(),
                share_id: "s1".to_string(),
                vault_id: "v1".to_string(),
                shared_with: "test@example.com".to_string(),
                permission: SharePermission::ViewOnly,
                created_at: Utc::now(),
                expires_at: Utc::now() + chrono::Duration::days(7),
                revoked: false,
            },
        );

        let result = revoke_share_token_handler(
            &store,
            &token_store,
            &audit_store,
            "v1",
            RevokeTokenRequest {
                token: "tok-1".to_string(),
            },
        );
        assert!(result.is_ok());
        let token = result.unwrap();
        assert!(token.revoked);

        // Verify storage updated
        let stored = get_share_token(&token_store, "tok-1").unwrap();
        assert!(stored.revoked);
    }

    #[test]
    fn test_revoke_nonexistent_token_returns_error() {
        let store = create_vault_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 100,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        let result = revoke_share_token_handler(
            &store,
            &token_store,
            &audit_store,
            "v1",
            RevokeTokenRequest {
                token: "does-not-exist".to_string(),
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_revoke_token_wrong_vault_returns_error() {
        let store = create_vault_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 100,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        add_share_token(
            &token_store,
            ShareToken {
                token: "tok-other".to_string(),
                share_id: "s1".to_string(),
                vault_id: "other-vault".to_string(),
                shared_with: "test@example.com".to_string(),
                permission: SharePermission::ViewOnly,
                created_at: Utc::now(),
                expires_at: Utc::now() + chrono::Duration::days(7),
                revoked: false,
            },
        );

        let result = revoke_share_token_handler(
            &store,
            &token_store,
            &audit_store,
            "v1",
            RevokeTokenRequest {
                token: "tok-other".to_string(),
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_access_vault_via_valid_token() {
        let store = create_vault_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 5000,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        add_share_token(
            &token_store,
            ShareToken {
                token: "valid-tok".to_string(),
                share_id: "s1".to_string(),
                vault_id: "v1".to_string(),
                shared_with: "reader@example.com".to_string(),
                permission: SharePermission::ViewOnly,
                created_at: Utc::now(),
                expires_at: Utc::now() + chrono::Duration::days(7),
                revoked: false,
            },
        );

        let result =
            access_vault_via_share_handler(&store, &token_store, &audit_store, "valid-tok");
        assert!(result.is_ok());
        let vault = result.unwrap();
        assert_eq!(vault.balance, 5000);
        assert_eq!(vault.owner, "owner1");

        // Audit log written
        assert!(audit_store
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.action == "vault_accessed_via_share"));
    }

    // ── Issue #1099: Vault Health Score tests ─────────────────────────────────

    fn make_vault_health(id: &str, balance: i128, ttl: Option<u64>, interval: u64) -> Vault {
        Vault {
            id: id.to_string(),
            owner: "owner".to_string(),
            beneficiary: "ben".to_string(),
            balance,
            check_in_interval: interval,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: ttl,
        }
    }

    #[test]
    fn test_health_score_perfect_vault() {
        let store = create_vault_store();
        let cache = create_health_cache();
        store.lock().unwrap().insert(
            "v1".to_string(),
            make_vault_health("v1", HEALTH_BALANCE_THRESHOLD, Some(86400), 86400),
        );
        let r = get_vault_health_handler(
            &store,
            &cache,
            "v1",
            HEALTH_STREAK_FULL,
            HEALTH_PASSKEY_FULL,
        )
        .unwrap();
        assert_eq!(r.score, 100);
        assert_eq!(r.factors.ttl_buffer, 30);
        assert_eq!(r.factors.streak, 20);
        assert_eq!(r.factors.balance, 30);
        assert_eq!(r.factors.passkey_diversity, 20);
    }

    #[test]
    fn test_health_score_zero_balance_no_streak_no_passkeys() {
        let store = create_vault_store();
        let cache = create_health_cache();
        store.lock().unwrap().insert(
            "v1".to_string(),
            make_vault_health("v1", 0, Some(86400), 86400),
        );
        let r = get_vault_health_handler(&store, &cache, "v1", 0, 0).unwrap();
        assert_eq!(r.factors.balance, 0);
        assert_eq!(r.factors.streak, 0);
        assert_eq!(r.factors.passkey_diversity, 0);
        // ttl is 100% → 30 pts
        assert_eq!(r.factors.ttl_buffer, 30);
        assert_eq!(r.score, 30);
    }

    #[test]
    fn test_health_score_half_ttl() {
        let store = create_vault_store();
        let cache = create_health_cache();
        // ttl_remaining = 43200, interval = 86400 → ratio 0.5 → 15 pts
        store.lock().unwrap().insert(
            "v1".to_string(),
            make_vault_health("v1", 500, Some(43200), 86400),
        );
        let r = get_vault_health_handler(&store, &cache, "v1", 0, 1).unwrap();
        assert_eq!(r.factors.ttl_buffer, 15);
    }

    #[test]
    fn test_health_score_not_found_returns_error() {
        let store = create_vault_store();
        let cache = create_health_cache();
        let result = get_vault_health_handler(&store, &cache, "missing", 0, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_health_score_cached_result_returned() {
        let store = create_vault_store();
        let cache = create_health_cache();
        store.lock().unwrap().insert(
            "v1".to_string(),
            make_vault_health("v1", 1000, Some(86400), 86400),
        );
        let r1 = get_vault_health_handler(&store, &cache, "v1", 3, 2).unwrap();
        // Mutate vault — cached result should be returned unchanged
        store.lock().unwrap().get_mut("v1").unwrap().balance = 0;
        let r2 = get_vault_health_handler(&store, &cache, "v1", 3, 2).unwrap();
        assert_eq!(
            r1.score, r2.score,
            "cache should shield against store mutation"
        );
    }

    #[test]
    fn test_health_cache_invalidated_yields_fresh_score() {
        let store = create_vault_store();
        let cache = create_health_cache();
        store.lock().unwrap().insert(
            "v1".to_string(),
            make_vault_health("v1", 1000, Some(86400), 86400),
        );
        let r1 = get_vault_health_handler(&store, &cache, "v1", 5, 3).unwrap();
        invalidate_health_cache(&cache, "v1");
        // Zero out balance after invalidation
        store.lock().unwrap().get_mut("v1").unwrap().balance = 0;
        let r2 = get_vault_health_handler(&store, &cache, "v1", 5, 3).unwrap();
        assert!(
            r2.score < r1.score,
            "score must drop after balance zeroed and cache cleared"
        );
    }

    // ── Issue #1100: Bulk Vault Summary tests ─────────────────────────────────

    fn insert_vault(store: &VaultStore, id: &str, balance: i128, ttl: Option<u64>) {
        store.lock().unwrap().insert(
            id.to_string(),
            Vault {
                id: id.to_string(),
                owner: "owner".to_string(),
                beneficiary: "ben".to_string(),
                balance,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: ttl,
            },
        );
    }

    #[test]
    fn test_bulk_summary_full_success() {
        let store = create_vault_store();
        let rate = create_bulk_rate_store();
        insert_vault(&store, "v1", 100, Some(86400));
        insert_vault(&store, "v2", 200, Some(43200));

        let req = BulkSummaryRequest {
            vault_ids: vec!["v1".to_string(), "v2".to_string()],
        };
        let resp = bulk_vault_summary_handler(&store, &rate, &req, "user1").unwrap();
        assert_eq!(resp.summaries.len(), 2);
        assert!(resp.summaries[0].status.is_some());
        assert_eq!(resp.summaries[0].balance, Some(100));
        assert!(resp.summaries[1].status.is_some());
        assert_eq!(resp.summaries[1].balance, Some(200));
    }

    #[test]
    fn test_bulk_summary_partial_missing_vault() {
        let store = create_vault_store();
        let rate = create_bulk_rate_store();
        insert_vault(&store, "v1", 500, Some(86400));

        let req = BulkSummaryRequest {
            vault_ids: vec!["v1".to_string(), "missing".to_string()],
        };
        let resp = bulk_vault_summary_handler(&store, &rate, &req, "user1").unwrap();
        assert_eq!(resp.summaries.len(), 2);
        // Known vault
        assert!(resp.summaries[0].status.is_some());
        // Missing vault — all fields null
        assert_eq!(resp.summaries[1].vault_id, "missing");
        assert!(resp.summaries[1].status.is_none());
        assert!(resp.summaries[1].balance.is_none());
    }

    #[test]
    fn test_bulk_summary_over_limit_rejected() {
        let store = create_vault_store();
        let rate = create_bulk_rate_store();
        let ids: Vec<String> = (0..=BULK_SUMMARY_MAX_IDS)
            .map(|i| format!("v{}", i))
            .collect();
        let req = BulkSummaryRequest { vault_ids: ids };
        let result = bulk_vault_summary_handler(&store, &rate, &req, "user1");
        assert!(
            result.is_err(),
            "should reject > {} vault IDs",
            BULK_SUMMARY_MAX_IDS
        );
    }

    #[test]
    fn test_bulk_summary_rate_limit_enforced() {
        let store = create_vault_store();
        let rate = create_bulk_rate_store();
        let req = BulkSummaryRequest { vault_ids: vec![] };

        for _ in 0..BULK_RATE_LIMIT {
            assert!(bulk_vault_summary_handler(&store, &rate, &req, "user1").is_ok());
        }
        // 11th request in the same window should be rejected
        let result = bulk_vault_summary_handler(&store, &rate, &req, "user1");
        assert!(result.is_err(), "11th request should be rate-limited");
        assert!(result.unwrap_err().contains("Rate limit exceeded"));
    }

    #[test]
    fn test_bulk_summary_rate_limit_independent_per_user() {
        let store = create_vault_store();
        let rate = create_bulk_rate_store();
        let req = BulkSummaryRequest { vault_ids: vec![] };

        for _ in 0..BULK_RATE_LIMIT {
            bulk_vault_summary_handler(&store, &rate, &req, "user1").unwrap();
        }
        // Different user should still have their full quota
        assert!(
            bulk_vault_summary_handler(&store, &rate, &req, "user2").is_ok(),
            "user2 should have independent rate limit"
        );
    }

    // ── Passkey Recovery Flow Tests (#1299) ─────────────────────────────────

    #[test]
    fn test_generate_recovery_codes() {
        let codes = generate_recovery_codes(10);
        assert_eq!(codes.len(), 10);
        for code in &codes {
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|c| c.is_alphanumeric()));
        }
        // All codes should be unique
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), codes.len());
    }

    #[test]
    fn test_hash_and_verify_recovery_code() {
        let code = "ABC123";
        let hash = hash_code(code);
        assert!(verify_code(code, &hash));
        assert!(!verify_code("wrong_code", &hash));
    }

    #[test]
    fn test_register_backup_passkey() {
        let passkey = Passkey {
            passkey_id: "pk-1".to_string(),
            owner: "owner1".to_string(),
            vault_id: "vault-1".to_string(),
            credential_id: "cred-abc".to_string(),
            device_name: "iPhone 15".to_string(),
            registered_at: Utc::now(),
            last_used: None,
            is_backup: true,
        };

        assert!(passkey.is_backup);
        assert_eq!(passkey.owner, "owner1");
        assert_eq!(passkey.vault_id, "vault-1");
        assert!(passkey.last_used.is_none());
    }

    #[test]
    fn test_recovery_code_single_use() {
        let code = RecoveryCode {
            code_id: "rc-1".to_string(),
            owner: "owner1".to_string(),
            vault_id: "vault-1".to_string(),
            code_hash: hash_code("ABC123"),
            generated_at: Utc::now(),
            used_at: None,
        };

        assert!(code.used_at.is_none());

        let mut used_code = code.clone();
        used_code.used_at = Some(Utc::now());
        assert!(used_code.used_at.is_some());
    }

    #[test]
    fn test_multiple_passkeys_per_owner() {
        let vault_id = "vault-1";
        let owner = "owner1";

        let pk1 = Passkey {
            passkey_id: "pk-1".to_string(),
            owner: owner.to_string(),
            vault_id: vault_id.to_string(),
            credential_id: "cred-1".to_string(),
            device_name: "Primary Phone".to_string(),
            registered_at: Utc::now(),
            last_used: Some(Utc::now()),
            is_backup: false,
        };

        let pk2 = Passkey {
            passkey_id: "pk-2".to_string(),
            owner: owner.to_string(),
            vault_id: vault_id.to_string(),
            credential_id: "cred-2".to_string(),
            device_name: "Security Key".to_string(),
            registered_at: Utc::now(),
            last_used: None,
            is_backup: true,
        };

        let mut passkeys = vec![pk1, pk2];
        let primary_count = passkeys.iter().filter(|p| !p.is_backup).count();
        let backup_count = passkeys.iter().filter(|p| p.is_backup).count();

        assert_eq!(primary_count, 1);
        assert_eq!(backup_count, 1);
        assert_eq!(passkeys.len(), 2);

        // Test filtering by backup status
        let backups: Vec<_> = passkeys
            .iter()
            .filter(|p| p.is_backup && p.owner == owner)
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].device_name, "Security Key");
    }

    #[test]
    fn test_recovery_code_expiry_tracking() {
        let now = Utc::now();
        let code_set = RecoveryCodeSet {
            set_id: "rcs-1".to_string(),
            owner: "owner1".to_string(),
            vault_id: "vault-1".to_string(),
            codes: vec!["ABC123".to_string(), "DEF456".to_string()],
            generated_at: now,
            codes_used: 0,
            total_codes: 2,
        };

        assert_eq!(code_set.total_codes, 2);
        assert_eq!(code_set.codes_used, 0);
        assert!((Utc::now() - code_set.generated_at).num_seconds() < 2);
    }

    #[test]
    fn test_lost_authenticator_recovery_scenario() {
        // Scenario: Owner loses primary authenticator, needs to recover with backup passkey
        let vault_id = "vault-1";
        let owner = "owner1";

        // Original primary passkey (now lost)
        let _primary = Passkey {
            passkey_id: "pk-primary".to_string(),
            owner: owner.to_string(),
            vault_id: vault_id.to_string(),
            credential_id: "lost-device".to_string(),
            device_name: "Lost iPhone".to_string(),
            registered_at: Utc::now(),
            last_used: None,
            is_backup: false,
        };

        // Backup passkey available for recovery
        let backup = Passkey {
            passkey_id: "pk-backup".to_string(),
            owner: owner.to_string(),
            vault_id: vault_id.to_string(),
            credential_id: "security-key-123".to_string(),
            device_name: "YubiKey 5".to_string(),
            registered_at: Utc::now(),
            last_used: None,
            is_backup: true,
        };

        assert!(backup.is_backup);
        assert_eq!(backup.credential_id, "security-key-123");
    }

    #[test]
    fn test_recovery_code_generation_for_new_vault() {
        // Scenario: During vault creation, generate recovery codes
        let codes = generate_recovery_codes(10);

        assert_eq!(codes.len(), 10);

        // Simulate storage of recovery codes
        let mut stored_codes: Vec<RecoveryCode> = Vec::new();
        for (i, code) in codes.iter().enumerate() {
            stored_codes.push(RecoveryCode {
                code_id: format!("rc-{}", i),
                owner: "owner1".to_string(),
                vault_id: "vault-1".to_string(),
                code_hash: hash_code(code),
                generated_at: Utc::now(),
                used_at: None,
            });
        }

        assert_eq!(stored_codes.len(), 10);

        // All codes should be marked as unused
        let unused = stored_codes
            .iter()
            .filter(|rc| rc.used_at.is_none())
            .count();
        assert_eq!(unused, 10);
    }

    #[test]
    fn test_recovery_code_consumption() {
        // Scenario: User uses recovery codes one at a time
        let code1 = "ABC123";
        let code2 = "DEF456";

        let mut code_entry1 = RecoveryCode {
            code_id: "rc-1".to_string(),
            owner: "owner1".to_string(),
            vault_id: "vault-1".to_string(),
            code_hash: hash_code(code1),
            generated_at: Utc::now(),
            used_at: None,
        };

        let mut code_entry2 = RecoveryCode {
            code_id: "rc-2".to_string(),
            owner: "owner1".to_string(),
            vault_id: "vault-1".to_string(),
            code_hash: hash_code(code2),
            generated_at: Utc::now(),
            used_at: None,
        };

        // First use
        code_entry1.used_at = Some(Utc::now());
        assert!(verify_code(code1, &code_entry1.code_hash));
        assert!(code_entry1.used_at.is_some());

        // Second code still available
        assert!(code_entry2.used_at.is_none());
        assert!(verify_code(code2, &code_entry2.code_hash));

        // Second use
        code_entry2.used_at = Some(Utc::now());
        assert!(code_entry2.used_at.is_some());
    }
}

// --- Issue #1143: Vesting Bonus Backend API ---

/// Claim vesting bonus for a vault.
///
/// Validates that the caller is the beneficiary, records an audit log entry,
/// and returns a mock transaction hash for the claim.
#[instrument(skip(db, headers), fields(vault_id = %vault_id))]
pub fn claim_vesting_bonus_handler(
    db: Arc<Db>,
    headers: HeaderMap,
    vault_id: &str,
    req: crate::models::ClaimBonusRequest,
) -> Result<crate::models::ClaimBonusResponse, String> {
    let vault = db
        .get_vault(vault_id)
        .ok_or_else(|| format!("Vault {} not found", vault_id))?;

    if vault.beneficiary != req.beneficiary {
        return Err("Caller is not the beneficiary for this vault".to_string());
    }

    let bonus_config = db
        .get_vesting_bonus(vault_id)
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or_else(|| "No vesting bonus configured for this vault".to_string())?;

    let base_amount = vault.balance / 2;
    let bonus_amount = (base_amount * bonus_config.bonus_bps as i128) / 10_000;
    let claimed_amount = base_amount + bonus_amount;

    let tx_hash = uuid::Uuid::new_v4().to_string();

    crate::audit::log_state_modification(
        &db,
        "vesting_bonus_claim",
        &format!("/api/vaults/{}/vesting/claim-bonus", vault_id),
        "success",
        &headers,
        Some(serde_json::json!({
            "vault_id": vault_id,
            "beneficiary": req.beneficiary,
            "bonus_bps": bonus_config.bonus_bps,
            "claimed_amount": claimed_amount,
            "bonus_amount": bonus_amount,
            "transaction_hash": tx_hash,
        })),
    );

    Ok(crate::models::ClaimBonusResponse {
        vault_id: vault_id.to_string(),
        claimed_amount,
        bonus_amount,
        transaction_hash: tx_hash,
        claimed_at: Utc::now(),
    })
}

/// Retrieve the vesting bonus configuration for a vault.
pub fn get_vesting_bonus_handler(
    db: Arc<Db>,
    vault_id: &str,
) -> Result<crate::models::VestingBonusResponse, String> {
    let _vault = db
        .get_vault(vault_id)
        .ok_or_else(|| format!("Vault {} not found", vault_id))?;

    let bonus_config = db
        .get_vesting_bonus(vault_id)
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(crate::models::VestingBonusResponse {
        vault_id: vault_id.to_string(),
        configured: bonus_config.is_some(),
        bonus_bps: bonus_config.as_ref().map(|c| c.bonus_bps),
        on_time_window_seconds: bonus_config.as_ref().map(|c| c.on_time_window_seconds),
    })
}
