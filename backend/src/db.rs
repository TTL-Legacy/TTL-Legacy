use crate::models::{Vault, VaultEvent, AuditEntry, SearchQuery, SearchResult, VaultStatus};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type VaultStore = Arc<Mutex<HashMap<String, Vault>>>;
pub type EventStore = Arc<Mutex<Vec<VaultEvent>>>;
pub type AuditStore = Arc<Mutex<Vec<AuditEntry>>>;

pub fn create_vault_store() -> VaultStore {
    Arc::new(Mutex::new(HashMap::new()))
}

pub fn create_event_store() -> EventStore {
    Arc::new(Mutex::new(Vec::new()))
}

pub fn create_audit_store() -> AuditStore {
    Arc::new(Mutex::new(Vec::new()))
}

pub fn search_vaults(
    store: &VaultStore,
    query: &SearchQuery,
) -> SearchResult {
    let vaults = store.lock().unwrap();
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(10);
    let offset = ((page - 1) * limit) as usize;

    let filtered: Vec<Vault> = vaults
        .values()
        .filter(|v| {
            if let Some(ref owner) = query.owner {
                if v.owner != *owner {
                    return false;
                }
            }
            if let Some(ref beneficiary) = query.beneficiary {
                if v.beneficiary != *beneficiary {
                    return false;
                }
            }
            if let Some(ref status) = query.status {
                if v.status != *status {
                    return false;
                }
            }
            if let Some(after) = query.created_after {
                if v.created_at < after {
                    return false;
                }
            }
            if let Some(before) = query.created_before {
                if v.created_at > before {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect();

    let total = filtered.len() as u32;
    let paginated: Vec<Vault> = filtered
        .into_iter()
        .skip(offset)
        .take(limit as usize)
        .collect();

    SearchResult {
        vaults: paginated,
        total,
        page,
        limit,
    }
}

pub fn get_vault_history(
    event_store: &EventStore,
    vault_id: &str,
) -> Vec<VaultEvent> {
    event_store
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.vault_id == vault_id)
        .cloned()
        .collect()
}

pub fn get_vault_audit_log(
    audit_store: &AuditStore,
    vault_id: &str,
) -> Vec<AuditEntry> {
    audit_store
        .lock()
        .unwrap()
        .iter()
        .filter(|a| a.details.get("vault_id").map_or(false, |v| v.as_str() == Some(vault_id)))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_vaults_by_owner() {
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

        let result = search_vaults(&store, &query);
        assert_eq!(result.vaults.len(), 1);
        assert_eq!(result.total, 1);
    }

    #[test]
    fn test_search_vaults_pagination() {
        let store = create_vault_store();
        for i in 0..25 {
            let vault = Vault {
                id: format!("v{}", i),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 1000,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(100000),
            };
            store.lock().unwrap().insert(format!("v{}", i), vault);
        }

        let query = SearchQuery {
            owner: Some("owner1".to_string()),
            beneficiary: None,
            status: None,
            created_after: None,
            created_before: None,
            page: Some(2),
            limit: Some(10),
        };

        let result = search_vaults(&store, &query);
        assert_eq!(result.vaults.len(), 10);
        assert_eq!(result.total, 25);
        assert_eq!(result.page, 2);
    }
}
