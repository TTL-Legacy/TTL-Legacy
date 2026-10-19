Looking at the current file, I don't see a `/checkin-calendar-export` handler or ICS generation code present. The issue references `checkin_calendar_export_tests.rs` and a `/checkin-calendar-export` endpoint, but neither the handler nor the ICS generation exists in this file.

Since the task requires me to implement the change and the handler doesn't exist, I need to add it. However, I must be careful — the issue says the endpoint "returns dates as ISO 8601 strings" implying it exists elsewhere. But since I can only edit this file and the requirement is to fix the handler here, I'll add a `checkin_calendar_export_handler` that returns Unix timestamps in milliseconds for JSON, while keeping ICS formatting separate.

Let me add the handler and tests. I'll place it near the other handlers and add tests in the test module.

```rust
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
use std::time::Duration;
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

// ── Issue #1816: Check-in Calendar Export ────────────────────────────────────

/// A single calendar event for the check-in calendar export.
///
/// `date` is a Unix timestamp in **milliseconds** (i64) so the frontend
/// `Calendar` component can render it directly. The ICS representation is
/// generated separately from the same underlying `DateTime<Utc>`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckinCalendarEvent {
    pub id: String,
    pub title: String,
    /// Unix timestamp in milliseconds.
    pub date: i64,
    pub event_type: String,
}

/// Response payload for `/checkin-calendar-export`.
///
/// All `date` fields are Unix timestamps in milliseconds.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckinCalendarExportResponse {
    pub vault_id: String,
    pub events: Vec<CheckinCalendarEvent>,
}

/// Builds the calendar export response for a vault.
///
/// Dates are emitted as Unix timestamps in milliseconds (i64) so the frontend
/// `Calendar` component can consume them directly.
pub fn checkin_calendar_export_handler(
    store: &VaultStore,
    event_store: &EventStore,
    vault_id: &str,
) -> Result<CheckinCalendarExportResponse, String> {
    let vault = store
        .lock()
        .unwrap()
        .get(vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    let history = get_vault_history(event_store, vault_id);

    let mut events: Vec<CheckinCalendarEvent> = Vec::new();

    // Vault creation event
    events.push(CheckinCalendarEvent {
        id: format!("{}:created", vault.id),
        title: "Vault created".to_string(),
        date: vault.created_at.timestamp_millis(),
        event_type: "created".to_string(),
    });

    // Check-in events from history
    for (idx, event) in history.iter().enumerate() {
        if matches!(event.event_type, EventType::CheckIn) {
            events.push(CheckinCalendarEvent {
                id: format!("{}:checkin:{}", vault.id, idx),
                title: "Check-in".to_string(),
                date: event.timestamp.timestamp_millis(),
                event_type: "checkin".to_string(),
            });
        }
    }

    // Next deadline event (derived from last_check_in + interval)
    let next_deadline =
        vault.last_check_in + chrono::Duration::seconds(vault.check_in_interval as i64);
    events.push(CheckinCalendarEvent {
        id: format!("{}:deadline", vault.id),
        title: "Next check-in deadline".to_string(),
        date: next_deadline.timestamp_millis(),
        event_type: "deadline".to_string(),
    });

    // Sort chronologically for stable output
    events.sort_by_key(|e| e.date);

    Ok(CheckinCalendarExportResponse {
        vault_id: vault.id,
        events,
    })
}

/// Renders the calendar export as an ICS (iCalendar) document.
///
/// ICS uses its own date formatting (`YYYYMMDDTHHMMSSZ`), independent of the
/// numeric millisecond timestamps used in the JSON response.
pub fn checkin_calendar_export_ics(
    store: &VaultStore,
    event_store: &EventStore,
    vault_id: &str,
) -> Result<String, String> {
    let vault = store
        .lock()
        .unwrap()
        .get(vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    let history = get_vault_history(event_store, vault_id);

    let mut ics = String::new();
    ics.push_str("BEGIN:VCALENDAR\r\n");
    ics.push_str("VERSION:2.0\r\n");
    ics.push_str("PRODID:-//Inheritance Vault//Check-in Calendar//EN\r\n");

    let mut push_event = |uid: &str, summary: &str, dt: DateTime<Utc>| {
        ics.push_str("BEGIN:VEVENT\r\n");
        ics.push_str(&format!("UID:{}\r\n", uid));
        ics.push_str(&format!(
            "DTSTAMP:{}\r\n",
            dt.format("%Y%m%dT%H%M%SZ")
        ));
        ics.push_str(&format!(
            "DTSTART:{}\r\n",
            dt.format("%Y%m%dT%H%M%SZ")
        ));
        ics.push_str(&format!("SUMMARY:{}\r\n", summary));
        ics.push_str("END:VEVENT\r\n");
    };

    push_event(
        &format!("{}:created", vault.id),
        "Vault created",
        vault.created_at,
    );

    for (idx, event) in history.iter().enumerate() {
        if matches!(event.event_type, EventType::CheckIn) {
            push_event(
                &format!("{}:checkin:{}", vault.id, idx),
                "Check-in",
                event.timestamp,
            );
        }
    }

    let next_deadline =
        vault.last_check_in + chrono::Duration::seconds(vault.check_in_interval as i64);
    push_event(
        &format!("{}:deadline", vault.id),
        "Next check-in deadline",
        next_deadline,
    );

    ics.push_str("END:VCALENDAR\r\n");
    Ok(ics)
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
        return Err("Share