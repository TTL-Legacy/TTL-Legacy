use crate::models::{
    Vault, VaultEvent, AuditEntry, SearchQuery, SearchResult, VaultStatus,
    VaultBackup, VaultShare, VaultNotificationPreferences,
    ReminderPreferences, Channel, Frequency,
};

use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type VaultStore = Arc<Mutex<HashMap<String, Vault>>>;
pub type EventStore = Arc<Mutex<Vec<VaultEvent>>>;
pub type AuditStore = Arc<Mutex<Vec<AuditEntry>>>;
pub type BackupStore = Arc<Mutex<HashMap<String, VaultBackup>>>;
pub type ShareStore = Arc<Mutex<Vec<VaultShare>>>;
pub type NotificationStore = Arc<Mutex<HashMap<String, VaultNotificationPreferences>>>;


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
    prefs: crate::models::VaultNotificationPreferences,

) {
    notif_store
        .lock()
        .unwrap()
        .insert(prefs.owner.clone(), prefs);
}

pub fn get_notification_preferences(
    notif_store: &NotificationStore,
    owner: &str,
) -> Option<crate::models::VaultNotificationPreferences> {

    notif_store.lock().unwrap().get(owner).cloned()
}

// ── TTL Insurance persistence (SQLite) ───────────────────────────────────────

use crate::models::{OwnerActivity, PurchaseTtlInsuranceRequest, TtlInsurancePolicy};

impl Db {
    pub fn upsert_insurance_policy(
        &self,
        policy: &TtlInsurancePolicy,
    ) -> Result<(), rusqlite::Error> {
        // Store DateTimes as RFC3339 strings.
        let purchased_at = policy.purchased_at.to_rfc3339();
        let last_extended_at = policy
            .last_extended_at
            .map(|d| d.to_rfc3339());

        let enabled_i = if policy.enabled { 1i64 } else { 0i64 };

        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO ttl_insurance_policies (
                vault_id,
                extension_seconds,
                inactivity_threshold_seconds,
                enabled,
                purchased_at,
                last_extended_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(vault_id) DO UPDATE SET
                extension_seconds = excluded.extension_seconds,
                inactivity_threshold_seconds = excluded.inactivity_threshold_seconds,
                enabled = excluded.enabled,
                purchased_at = excluded.purchased_at,
                last_extended_at = excluded.last_extended_at
            "#,
            params![
                policy.vault_id as i64,
                policy.extension_seconds as i64,
                policy.inactivity_threshold_seconds as i64,
                enabled_i,
                purchased_at,
                last_extended_at,
            ],
        )?;

        Ok(())
    }

    pub fn get_insurance_policy(&self, vault_id: u64) -> Result<Option<TtlInsurancePolicy>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"
            SELECT vault_id, extension_seconds, inactivity_threshold_seconds, enabled, purchased_at, last_extended_at
            FROM ttl_insurance_policies
            WHERE vault_id = ?1
            "#,
        )?;

        let row_res = stmt.query_row(params![vault_id as i64], |r| {
            let purchased_at_str: String = r.get(4)?;
            let purchased_at = chrono::DateTime::parse_from_rfc3339(&purchased_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;

            let last_extended_at: Option<String> = r.get(5)?;
            let last_extended_at_dt = match last_extended_at {
                Some(s) => {
                    let dt = chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
                    Some(dt)
                }
                None => None,
            };

            let enabled_i: i64 = r.get(3)?;

            Ok(TtlInsurancePolicy {
                vault_id: r.get::<_, i64>(0)? as u64,
                extension_seconds: r.get::<_, i64>(1)? as u64,
                inactivity_threshold_seconds: r.get::<_, i64>(2)? as u64,
                enabled: enabled_i != 0,
                purchased_at,
                last_extended_at: last_extended_at_dt,
            })
        });

        match row_res {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn upsert_owner_activity(&self, owner_id: u64, last_active_at: chrono::DateTime<chrono::Utc>) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO owner_activity (owner_id, last_active_at)
            VALUES (?1, ?2)
            ON CONFLICT(owner_id) DO UPDATE SET
                last_active_at = excluded.last_active_at
            "#,
            params![
                owner_id as i64,
                last_active_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_owner_last_active_at(
        &self,
        owner_id: u64,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"
            SELECT last_active_at
            FROM owner_activity
            WHERE owner_id = ?1
            "#,
        )?;

        let row_res: Result<String, rusqlite::Error> = stmt.query_row(params![owner_id as i64], |r| r.get(0));

        match row_res {
            Ok(s) => {
                let dt = chrono::DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
                Ok(Some(dt))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn all_enabled_insurance_policies(&self) -> Result<Vec<TtlInsurancePolicy>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"
            SELECT vault_id, extension_seconds, inactivity_threshold_seconds, enabled, purchased_at, last_extended_at
            FROM ttl_insurance_policies
            WHERE enabled = 1
            "#,
        )?;

        let iter = stmt.query_map([], |r| {
            let purchased_at_str: String = r.get(4)?;
            let purchased_at = chrono::DateTime::parse_from_rfc3339(&purchased_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;

            let last_extended_at: Option<String> = r.get(5)?;
            let last_extended_at_dt = match last_extended_at {
                Some(s) => {
                    let dt = chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
                    Some(dt)
                }
                None => None,
            };

            let enabled_i: i64 = r.get(3)?;

            Ok(TtlInsurancePolicy {
                vault_id: r.get::<_, i64>(0)? as u64,
                extension_seconds: r.get::<_, i64>(1)? as u64,
                inactivity_threshold_seconds: r.get::<_, i64>(2)? as u64,
                enabled: enabled_i != 0,
                purchased_at,
                last_extended_at: last_extended_at_dt,
            })
        })?;

        let mut out = Vec::new();
        for item in iter {
            out.push(item?);
        }
        Ok(out)
    }
}


use rusqlite::{params, Connection};

pub struct PoolConfig {
    pub min: u32,
    pub max: u32,
    pub timeout_secs: u32,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min: 2,
            max: 10,
            timeout_secs: 30,
        }
    }
}

impl PoolConfig {
    pub fn from_env() -> Self {
        Self {
            min: std::env::var("DB_POOL_MIN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2),
            max: std::env::var("DB_POOL_MAX")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            timeout_secs: std::env::var("DB_POOL_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
        }
    }
}

pub struct Db {
    conn: std::sync::Mutex<Connection>,
    pool_config: PoolConfig,
}

impl Db {
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        Self::open_with_pool_config(path, &PoolConfig::default())
    }

    pub fn open_with_pool_config(path: &str, config: &PoolConfig) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(config.timeout_secs as u64))?;
        Ok(Self {
            conn: std::sync::Mutex::new(conn),
            pool_config: PoolConfig {
                min: config.min,
                max: config.max,
                timeout_secs: config.timeout_secs,
            },
        })
    }

    pub fn check_connectivity(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("SELECT 1")?;
        Ok(())
    }


    pub fn migrate(&self) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS reminder_preferences (
                vault_id              INTEGER PRIMARY KEY,
                channels             TEXT NOT NULL,
                hours_before_expiry  INTEGER NOT NULL,
                frequency            TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS ttl_insurance_policies (
                vault_id                      INTEGER PRIMARY KEY,
                extension_seconds            INTEGER NOT NULL,
                inactivity_threshold_seconds INTEGER NOT NULL,
                enabled                       INTEGER NOT NULL,
                purchased_at                  TEXT NOT NULL,
                last_extended_at              TEXT
            );

            CREATE TABLE IF NOT EXISTS owner_activity (
                owner_id        INTEGER PRIMARY KEY,
                last_active_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS idempotency_keys (
                key          TEXT PRIMARY KEY,
                status_code  INTEGER NOT NULL,
                response_body TEXT NOT NULL,
                created_at   TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS unsubscribe_tokens (
                token      TEXT PRIMARY KEY,
                owner      TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS unsubscribed_users (
                owner TEXT PRIMARY KEY
            );
            "#,
        )?;
        Ok(())
    }


    pub fn upsert(&self, prefs: &ReminderPreferences) -> Result<(), rusqlite::Error> {
        let channels_json = serde_json::to_string(&prefs.channels).unwrap();
self.conn.lock().unwrap().execute(

            r#"
            INSERT INTO reminder_preferences (vault_id, channels, hours_before_expiry, frequency)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(vault_id) DO UPDATE SET
              channels = excluded.channels,
              hours_before_expiry = excluded.hours_before_expiry,
              frequency = excluded.frequency
            "#,
            params![
                prefs.vault_id as i64,
                channels_json,
                prefs.hours_before_expiry as i64,
                serde_json::to_string(&prefs.frequency).unwrap(),
            ],
        )?;
        Ok(())
    }

    pub fn get(&self, vault_id: u64) -> Result<ReminderPreferences, rusqlite::Error> {
let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            "SELECT vault_id, channels, hours_before_expiry, frequency FROM reminder_preferences WHERE vault_id = ?1",


        )?;
        let row = stmt.query_row(params![vault_id as i64], |r| {
            let channels_str: String = r.get(1)?;
            let frequency_str: String = r.get(3)?;
            let channels: Vec<Channel> = serde_json::from_str(&channels_str).unwrap_or_default();
            let frequency: Frequency = serde_json::from_str(&frequency_str).unwrap();
            Ok(ReminderPreferences {
                vault_id: r.get::<_, i64>(0)? as u64,
                channels,
                hours_before_expiry: r.get::<_, i64>(2)? as u32,
                frequency,
            })
        })?;
        Ok(row)
    }

    pub fn all(&self) -> Result<Vec<ReminderPreferences>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            "SELECT vault_id, channels, hours_before_expiry, frequency FROM reminder_preferences",
        )?;
        let iter = stmt.query_map([], |r| {
            let channels_str: String = r.get(1)?;
            let frequency_str: String = r.get(3)?;
            let channels: Vec<Channel> = serde_json::from_str(&channels_str).unwrap_or_default();
            let frequency: Frequency = serde_json::from_str(&frequency_str).unwrap();
            Ok(ReminderPreferences {
                vault_id: r.get::<_, i64>(0)? as u64,
                channels,
                hours_before_expiry: r.get::<_, i64>(2)? as u32,
                frequency,
            })
        })?;

        let mut out = Vec::new();
        for item in iter {
            out.push(item?);
        }
        Ok(out)
    }

    // ── Idempotency (#825) ──────────────────────────────────────────────────

    pub fn store_idempotency(&self, key: &str, status_code: u16, response_body: &str) {
        let _ = self.conn.lock().unwrap().execute(
            r#"INSERT OR REPLACE INTO idempotency_keys (key, status_code, response_body, created_at)
               VALUES (?1, ?2, ?3, ?4)"#,
            params![key, status_code as i64, response_body, chrono::Utc::now().to_rfc3339()],
        );
    }

    pub fn check_idempotency(&self, key: &str) -> Option<crate::models::IdempotencyRecord> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding
            .prepare("SELECT key, status_code, response_body, created_at FROM idempotency_keys WHERE key = ?1")
            .ok()?;
        stmt.query_row(params![key], |r| {
            let created_str: String = r.get(3)?;
            let created_at = chrono::DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
            let age = chrono::Utc::now().signed_duration_since(created_at).num_seconds();
            if age > 86_400 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(crate::models::IdempotencyRecord {
                key: r.get(0)?,
                status_code: r.get::<_, i64>(1)? as u16,
                response_body: r.get(2)?,
                created_at,
            })
        })
        .ok()
    }

    // ── Unsubscribe (#828) ──────────────────────────────────────────────────

    pub fn store_unsubscribe_token(&self, token: &str, owner: &str) {
        let _ = self.conn.lock().unwrap().execute(
            r#"INSERT OR REPLACE INTO unsubscribe_tokens (token, owner, created_at)
               VALUES (?1, ?2, ?3)"#,
            params![token, owner, chrono::Utc::now().to_rfc3339()],
        );
    }

    pub fn process_unsubscribe(&self, token: &str) -> Result<String, String> {
        let conn = self.conn.lock().unwrap();
        let owner: String = conn
            .query_row(
                "SELECT owner FROM unsubscribe_tokens WHERE token = ?1",
                params![token],
                |r| r.get(0),
            )
            .map_err(|_| "invalid or expired unsubscribe token".to_string())?;

        conn.execute(
            "INSERT OR IGNORE INTO unsubscribed_users (owner) VALUES (?1)",
            params![&owner],
        )
        .map_err(|e| e.to_string())?;

        Ok(owner)
    }

    pub fn is_unsubscribed(&self, owner: &str) -> bool {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT 1 FROM unsubscribed_users WHERE owner = ?1",
                params![owner],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn generate_unsubscribe_token(&self, owner: &str) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        self.store_unsubscribe_token(&token, owner);
        token
    }
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
