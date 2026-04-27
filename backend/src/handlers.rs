use crate::models::*;
use crate::db::*;
use chrono::Utc;
use serde_json::json;
use std::io::Write;

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
}
