use crate::models::{
    Vault, VaultEvent, AuditEntry, SearchQuery, SearchResult, VaultStatus,
    VaultBackup, VaultShare, NotificationPreferences,
};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type VaultStore = Arc<Mutex<HashMap<String, Vault>>>;
pub type EventStore = Arc<Mutex<Vec<VaultEvent>>>;
pub type AuditStore = Arc<Mutex<Vec<AuditEntry>>>;
pub type BackupStore = Arc<Mutex<HashMap<String, VaultBackup>>>;
pub type ShareStore = Arc<Mutex<Vec<VaultShare>>>;
pub type NotificationStore = Arc<Mutex<HashMap<String, NotificationPreferences>>>;

pub fn create_vault_store() -> VaultStore {
    Arc::new(Mutex::new(HashMap::new()))
}

pub fn create_event_store() -> EventStore {
    Arc::new(Mutex::new(Vec::new()))
}

pub fn create_audit_store() -> AuditStore {
    Arc::new(Mutex::new(Vec::new()))
}

pub fn create_backup_store() -> BackupStore {
    Arc::new(Mutex::new(HashMap::new()))
}

pub fn create_share_store() -> ShareStore {
    Arc::new(Mutex::new(Vec::new()))
}

pub fn create_notification_store() -> NotificationStore {
    Arc::new(Mutex::new(HashMap::new()))
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

// ── Task 1: Analytics ────────────────────────────────────────────────────────

pub fn compute_vault_analytics(store: &VaultStore) -> crate::models::VaultAnalytics {
    use crate::models::{VaultAnalytics, TimeSeriesPoint, VaultStatus};
    use std::collections::BTreeMap;

    let vaults = store.lock().unwrap();
    let total_vaults = vaults.len() as u64;
    let active_vaults = vaults.values().filter(|v| v.status == VaultStatus::Active).count() as u64;
    let released_vaults = vaults.values().filter(|v| v.status == VaultStatus::Released).count() as u64;

    let avg_ttl = if total_vaults > 0 {
        vaults.values().map(|v| v.check_in_interval as f64).sum::<f64>() / total_vaults as f64
    } else {
        0.0
    };

    let release_rate = if total_vaults > 0 {
        released_vaults as f64 / total_vaults as f64
    } else {
        0.0
    };

    // Build daily time-series bucketed by creation date
    let mut created_by_day: BTreeMap<String, u64> = BTreeMap::new();
    let mut released_by_day: BTreeMap<String, u64> = BTreeMap::new();
    for v in vaults.values() {
        let day = v.created_at.format("%Y-%m-%d").to_string();
        *created_by_day.entry(day.clone()).or_insert(0) += 1;
        if v.status == VaultStatus::Released {
            *released_by_day.entry(day).or_insert(0) += 1;
        }
    }

    let all_days: std::collections::BTreeSet<String> = created_by_day
        .keys()
        .chain(released_by_day.keys())
        .cloned()
        .collect();

    let time_series = all_days
        .into_iter()
        .map(|date| TimeSeriesPoint {
            vaults_created: *created_by_day.get(&date).unwrap_or(&0),
            vaults_released: *released_by_day.get(&date).unwrap_or(&0),
            date,
        })
        .collect();

    VaultAnalytics {
        total_vaults,
        active_vaults,
        average_ttl_seconds: avg_ttl,
        release_rate,
        time_series,
    }
}

// ── Task 2: Backup & Recovery ─────────────────────────────────────────────────

pub fn store_backup(backup_store: &BackupStore, backup: crate::models::VaultBackup) {
    backup_store.lock().unwrap().insert(backup.backup_id.clone(), backup);
}

pub fn get_backup(backup_store: &BackupStore, backup_id: &str) -> Option<crate::models::VaultBackup> {
    backup_store.lock().unwrap().get(backup_id).cloned()
}

// ── Task 3: Sharing ───────────────────────────────────────────────────────────

pub fn add_vault_share(share_store: &ShareStore, share: crate::models::VaultShare) {
    share_store.lock().unwrap().push(share);
}

pub fn get_vault_shares(share_store: &ShareStore, vault_id: &str) -> Vec<crate::models::VaultShare> {
    share_store
        .lock()
        .unwrap()
        .iter()
        .filter(|s| s.vault_id == vault_id)
        .cloned()
        .collect()
}

// ── Task 4: Notification Preferences ─────────────────────────────────────────

pub fn set_notification_preferences(
    notif_store: &NotificationStore,
    prefs: crate::models::NotificationPreferences,
) {
    notif_store.lock().unwrap().insert(prefs.vault_id.clone(), prefs);
}

pub fn get_notification_preferences(
    notif_store: &NotificationStore,
    vault_id: &str,
) -> Option<crate::models::NotificationPreferences> {
    notif_store.lock().unwrap().get(vault_id).cloned()
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
