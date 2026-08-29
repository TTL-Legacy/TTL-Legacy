use crate::models::{
    Vault, VaultEvent, AuditEntry, SearchQuery, SearchResult, VaultStatus,
    VaultBackup, VaultShare, VaultNotificationPreferences, AuditLogEntry, AuditLogQuery,
    ReminderPreferences, Channel, Frequency,
    Subscription, SubscriptionChannel, SubscriptionFrequency,
};

use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type VaultStore = Arc<Mutex<HashMap<String, Vault>>>;
pub type EventStore = Arc<Mutex<Vec<VaultEvent>>>;
pub type AuditStore = Arc<Mutex<Vec<AuditEntry>>>;
pub type BackupStore = Arc<Mutex<HashMap<String, VaultBackup>>>;
pub type ShareStore = Arc<Mutex<Vec<VaultShare>>>;
pub type ShareTokenStore = Arc<Mutex<HashMap<String, ShareToken>>>;
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

pub fn create_share_token_store() -> ShareTokenStore {
    Arc::new(Mutex::new(HashMap::new()))
}

pub fn create_notification_store() -> NotificationStore {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Runs the versioned migrations under `backend/migrations/` against the
/// SQLite file at `path` — Issue #1176.
///
/// This is intentionally independent of `Db::migrate`'s older ad-hoc
/// in-Rust migration list (kept as-is, for backward compatibility with
/// existing deployments' `schema_migrations` bookkeeping): sqlx tracks its
/// own applied-migration state in a separate `_sqlx_migrations` table, so
/// the two systems can coexist against the same file without conflict.
/// Every table introduced going forward should be added as a new file
/// under `migrations/` and applied via this function instead of extending
/// the old list.
pub async fn run_sqlx_migrations(path: &str) -> Result<(), sqlx::Error> {
    use sqlx::sqlite::SqlitePoolOptions;

    let url = format!("sqlite://{path}?mode=rwc");
    let pool = SqlitePoolOptions::new().connect(&url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    pool.close().await;
    Ok(())
}

// ── Shared application state for axum routes ─────────────────────────────────

pub struct AppState {
    pub db: Arc<Db>,
    pub vault_store: VaultStore,
    pub event_store: EventStore,
    pub audit_store: AuditStore,
    pub share_store: ShareStore,
    pub share_token_store: ShareTokenStore,
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

// ── Share token persistence ──────────────────────────────────────────────────

pub fn add_share_token(store: &ShareTokenStore, token: ShareToken) {
    store.lock().unwrap().insert(token.token.clone(), token);
}

pub fn get_share_token(store: &ShareTokenStore, token: &str) -> Option<ShareToken> {
    store.lock().unwrap().get(token).cloned()
}

pub fn get_vault_share_tokens(store: &ShareTokenStore, vault_id: &str) -> Vec<ShareToken> {
    store
        .lock()
        .unwrap()
        .values()
        .filter(|t| t.vault_id == vault_id)
        .cloned()
        .collect()
}

pub fn revoke_share_token(store: &ShareTokenStore, token: &str) -> Option<ShareToken> {
    let mut lock = store.lock().unwrap();
    if let Some(t) = lock.get_mut(token) {
        t.revoked = true;
        Some(t.clone())
    } else {
        None
    }
}

// ── Audit helper ─────────────────────────────────────────────────────────────

pub fn append_audit_entry(audit_store: &AuditStore, action: &str, actor: &str, details: serde_json::Value) {
    audit_store.lock().unwrap().push(AuditEntry {
        timestamp: Utc::now(),
        action: action.to_string(),
        actor: actor.to_string(),
        details,
    });
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
    /// In-memory vault store shared across the application.
    pub vault_store: VaultStore,
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
            vault_store: create_vault_store(),
        })
    }

    /// Insert or replace a vault in the in-memory store.
    pub fn insert_vault(&self, vault: crate::models::Vault) {
        self.vault_store.lock().unwrap().insert(vault.id.clone(), vault);
    }

    /// Retrieve a vault from the in-memory store by string ID.
    pub fn get_vault(&self, vault_id: &str) -> Option<crate::models::Vault> {
        self.vault_store.lock().unwrap().get(vault_id).cloned()
    }

    pub fn check_connectivity(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("SELECT 1")?;
        Ok(())
    }


    pub fn migrate(&self) -> Result<(), rusqlite::Error> {
        // Bootstrap the migration tracking table before anything else.
        self.conn.lock().unwrap().execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version    TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
        )?;

        const MIGRATIONS: &[(&str, &str)] = &[
            (
                "1",
                r#"
                CREATE TABLE IF NOT EXISTS reminder_preferences (
                    vault_id             INTEGER PRIMARY KEY,
                    channels             TEXT NOT NULL,
                    hours_before_expiry  INTEGER NOT NULL,
                    frequency            TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS ttl_insurance_policies (
                    vault_id                      INTEGER PRIMARY KEY,
                    extension_seconds             INTEGER NOT NULL,
                    inactivity_threshold_seconds  INTEGER NOT NULL,
                    enabled                        INTEGER NOT NULL,
                    purchased_at                   TEXT NOT NULL,
                    last_extended_at               TEXT
                );
                CREATE TABLE IF NOT EXISTS owner_activity (
                    owner_id       INTEGER PRIMARY KEY,
                    last_active_at TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS idempotency_keys (
                    key           TEXT PRIMARY KEY,
                    status_code   INTEGER NOT NULL,
                    response_body TEXT NOT NULL,
                    created_at    TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS unsubscribe_tokens (
                    token      TEXT PRIMARY KEY,
                    owner      TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS unsubscribed_users (
                    owner TEXT PRIMARY KEY
                );
                CREATE TABLE IF NOT EXISTS reminder_tokens (
                    token      TEXT PRIMARY KEY,
                    vault_id   TEXT NOT NULL,
                    owner      TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );
                "#,
            ),
            (
                "2",
                "ALTER TABLE reminder_preferences ADD COLUMN deleted_at TEXT;",
            ),
            (
                "3",
                r#"
                CREATE TABLE IF NOT EXISTS audit_logs (
                    id         INTEGER PRIMARY KEY AUTOINCREMENT,
                    timestamp  TEXT NOT NULL,
                    user_id    TEXT NOT NULL DEFAULT '',
                    action     TEXT NOT NULL,
                    resource   TEXT NOT NULL DEFAULT '',
                    result     TEXT NOT NULL DEFAULT 'success',
                    ip_address TEXT NOT NULL DEFAULT '',
                    details    TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_audit_logs_timestamp ON audit_logs(timestamp);
                CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id   ON audit_logs(user_id);
                CREATE INDEX IF NOT EXISTS idx_audit_logs_action    ON audit_logs(action);
                "#,
            ),
            (
                "5",
                r#"
                CREATE TABLE IF NOT EXISTS escalation_states (
                    vault_id                INTEGER PRIMARY KEY,
                    last_escalation_tier    TEXT,
                    escalated_at            TEXT
                );
                CREATE TABLE IF NOT EXISTS escalation_events (
                    id              TEXT PRIMARY KEY,
                    vault_id        INTEGER NOT NULL,
                    tier            TEXT NOT NULL,
                    dispatched_at   TEXT NOT NULL,
                    channels        TEXT NOT NULL DEFAULT '[]'
                );
                CREATE INDEX IF NOT EXISTS idx_escalation_events_vault_id ON escalation_events(vault_id);
                CREATE TABLE IF NOT EXISTS webhook_deliveries (
                    id              TEXT PRIMARY KEY,
                    vault_id        TEXT NOT NULL,
                    event_type      TEXT NOT NULL,
                    payload         TEXT NOT NULL DEFAULT '{}',
                    endpoint_url    TEXT NOT NULL,
                    status          TEXT NOT NULL DEFAULT 'pending',
                    attempt_count   INTEGER NOT NULL DEFAULT 0,
                    next_retry_at   TEXT,
                    created_at      TEXT NOT NULL,
                    attempts        TEXT NOT NULL DEFAULT '[]'
                );
                CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_vault_id ON webhook_deliveries(vault_id);
                CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_status   ON webhook_deliveries(status);
                CREATE TABLE IF NOT EXISTS webhook_subscriptions (
                    id              TEXT PRIMARY KEY,
                    vault_id        TEXT NOT NULL,
                    endpoint_url    TEXT NOT NULL,
                    event_types     TEXT NOT NULL DEFAULT '[]',
                    created_at      TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_webhook_subscriptions_vault_id ON webhook_subscriptions(vault_id);
                CREATE TABLE IF NOT EXISTS vault_timeline_events (
                    id          TEXT PRIMARY KEY,
                    vault_id    TEXT NOT NULL,
                    kind        TEXT NOT NULL,
                    timestamp   TEXT NOT NULL,
                    description TEXT NOT NULL DEFAULT '',
                    amount      INTEGER,
                    metadata    TEXT NOT NULL DEFAULT '{}'
                );
                CREATE INDEX IF NOT EXISTS idx_vault_timeline_events_vault_id  ON vault_timeline_events(vault_id);
                CREATE INDEX IF NOT EXISTS idx_vault_timeline_events_kind      ON vault_timeline_events(kind);
                CREATE TABLE IF NOT EXISTS vault_subscriptions (
                    vault_id  INTEGER PRIMARY KEY,
                    owner     TEXT NOT NULL,
                    channels  TEXT NOT NULL,
                    frequency TEXT NOT NULL
                );
                "#,
            ),
            (
                "4",
                r#"
                CREATE TABLE IF NOT EXISTS sponsored_releases (
                    tx_id             TEXT PRIMARY KEY,
                    vault_id          TEXT NOT NULL,
                    beneficiary       TEXT NOT NULL,
                    released_amount   INTEGER NOT NULL,
                    protocol_fee      INTEGER NOT NULL,
                    net_amount        INTEGER NOT NULL,
                    fee_bump_tx_hash  TEXT NOT NULL,
                    sponsor_account   TEXT NOT NULL,
                    sponsorship_fee   INTEGER NOT NULL,
                    status            TEXT NOT NULL,
                    created_at        TEXT NOT NULL,
                    executed_at       TEXT,
                    ledger_sequence   INTEGER,
                    error             TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_sponsored_releases_vault_id ON sponsored_releases(vault_id);
                CREATE INDEX IF NOT EXISTS idx_sponsored_releases_beneficiary ON sponsored_releases(beneficiary);
                CREATE INDEX IF NOT EXISTS idx_sponsored_releases_status ON sponsored_releases(status);
                CREATE INDEX IF NOT EXISTS idx_sponsored_releases_created_at ON sponsored_releases(created_at);
                "#,
            ),
            (
                "5",
                r#"
                CREATE TABLE IF NOT EXISTS vesting_bonus_config (
                    vault_id              TEXT PRIMARY KEY,
                    bonus_bps             INTEGER NOT NULL,
                    on_time_window_seconds INTEGER NOT NULL
                );
                "#,
            ),
        ];

        for (version, sql) in MIGRATIONS {
            let already_applied: bool = {
                let conn = self.conn.lock().unwrap();
                conn.query_row(
                    "SELECT 1 FROM schema_migrations WHERE version = ?1",
                    params![version],
                    |_| Ok(true),
                )
                .unwrap_or(false)
            };

            if !already_applied {
                tracing::info!(version = version, "applying migration");
                self.conn.lock().unwrap().execute_batch(sql)?;
                self.conn.lock().unwrap().execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                    params![version, chrono::Utc::now().to_rfc3339()],
                )?;
                tracing::info!(version = version, "migration applied successfully");
            } else {
                tracing::debug!(version = version, "migration already applied, skipping");
            }
        }

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
              frequency = excluded.frequency,
              deleted_at = NULL
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
            r#"SELECT vault_id, channels, hours_before_expiry, frequency, deleted_at
               FROM reminder_preferences
               WHERE vault_id = ?1 AND deleted_at IS NULL"#,
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
                deleted_at: None,
            })
        })?;
        Ok(row)
    }

    pub fn all(&self) -> Result<Vec<ReminderPreferences>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"SELECT vault_id, channels, hours_before_expiry, frequency, deleted_at
               FROM reminder_preferences
               WHERE deleted_at IS NULL"#,
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
                deleted_at: None,
            })
        })?;

        let mut out = Vec::new();
        for item in iter {
            out.push(item?);
        }
        Ok(out)
    }

    pub fn soft_delete_reminder(&self, vault_id: u64) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "UPDATE reminder_preferences SET deleted_at = ?1 WHERE vault_id = ?2 AND deleted_at IS NULL",
            params![chrono::Utc::now().to_rfc3339(), vault_id as i64],
        )?;
        Ok(())
    }

    pub fn all_reminders_including_deleted(
        &self,
        vault_id: u64,
    ) -> Result<Vec<ReminderPreferences>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"SELECT vault_id, channels, hours_before_expiry, frequency, deleted_at
               FROM reminder_preferences
               WHERE vault_id = ?1"#,
        )?;
        let iter = stmt.query_map(params![vault_id as i64], |r| {
            let channels_str: String = r.get(1)?;
            let frequency_str: String = r.get(3)?;
            let channels: Vec<Channel> = serde_json::from_str(&channels_str).unwrap_or_default();
            let frequency: Frequency = serde_json::from_str(&frequency_str).unwrap();
            let deleted_at_str: Option<String> = r.get(4)?;
            let deleted_at = deleted_at_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            });
            Ok(ReminderPreferences {
                vault_id: r.get::<_, i64>(0)? as u64,
                channels,
                hours_before_expiry: r.get::<_, i64>(2)? as u32,
                frequency,
                deleted_at,
            })
        })?;

        let mut out = Vec::new();
        for item in iter {
            out.push(item?);
        }
        Ok(out)
    }

    pub fn upsert_subscription(&self, sub: &Subscription) -> Result<(), rusqlite::Error> {
        let channels_json = serde_json::to_string(&sub.channels).unwrap();
        let frequency_json = serde_json::to_string(&sub.frequency).unwrap();
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO vault_subscriptions (vault_id, owner, channels, frequency)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(vault_id) DO UPDATE SET
              owner = excluded.owner,
              channels = excluded.channels,
              frequency = excluded.frequency
            "#,
            params![
                sub.vault_id as i64,
                sub.owner,
                channels_json,
                frequency_json,
            ],
        )?;
        Ok(())
    }

    pub fn delete_subscription(&self, vault_id: u64) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "DELETE FROM vault_subscriptions WHERE vault_id = ?1",
            params![vault_id as i64],
        )?;
        Ok(())
    }

    pub fn get_subscription(&self, vault_id: u64) -> Result<Option<Subscription>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"SELECT vault_id, owner, channels, frequency
               FROM vault_subscriptions
               WHERE vault_id = ?1"#,
        )?;
        let row = stmt.query_row(params![vault_id as i64], |r| {
            let channels_str: String = r.get(2)?;
            let frequency_str: String = r.get(3)?;
            let channels: Vec<SubscriptionChannel> = serde_json::from_str(&channels_str).unwrap_or_default();
            let frequency: SubscriptionFrequency = serde_json::from_str(&frequency_str).unwrap();
            Ok(Subscription {
                vault_id: r.get::<_, i64>(0)? as u64,
                owner: r.get(1)?,
                channels,
                frequency,
            })
        });
        match row {
            Ok(sub) => Ok(Some(sub)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
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

    // ── Token-based reminder links (#1286) ──────────────────────────────────

    pub fn store_reminder_token(&self, token: &str, vault_id: &str, owner: &str) {
        let _ = self.conn.lock().unwrap().execute(
            r#"INSERT OR REPLACE INTO reminder_tokens (token, vault_id, owner, created_at)
               VALUES (?1, ?2, ?3, ?4)"#,
            params![token, vault_id, owner, chrono::Utc::now().to_rfc3339()],
        );
    }

    pub fn generate_reminder_token(&self, vault_id: &str, owner: &str) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        self.store_reminder_token(&token, vault_id, owner);
        token
    }

    pub fn resolve_reminder_token(&self, token: &str) -> Result<(String, String), String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT vault_id, owner FROM reminder_tokens WHERE token = ?1",
            params![token],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "invalid or expired reminder token".to_string())
    }

    pub fn generate_reminder_url(&self, vault_id: &str, owner: &str, base_url: &str) -> String {
        let token = self.generate_reminder_token(vault_id, owner);
        format!("{base_url}/reminders/check-in?token={token}")
    }

    // ── Audit Log persistence (#961) ─────────────────────────────────────────

    pub fn insert_audit_log(&self, entry: &AuditLogEntry) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO audit_logs (timestamp, user_id, action, resource, result, ip_address, details)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                entry.timestamp.to_rfc3339(),
                entry.user_id,
                entry.action,
                entry.resource,
                entry.result,
                entry.ip_address,
                entry.details.as_ref().map(|d| d.to_string()),
            ],
        )?;
        Ok(())
    }

    pub fn query_audit_logs(&self, query: &AuditLogQuery) -> Result<Vec<AuditLogEntry>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT id, timestamp, user_id, action, resource, result, ip_address, details FROM audit_logs WHERE 1=1"
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref user_id) = query.user_id {
            sql.push_str(" AND user_id = ?");
            param_values.push(Box::new(user_id.clone()));
        }
        if let Some(ref action) = query.action {
            sql.push_str(" AND action = ?");
            param_values.push(Box::new(action.clone()));
        }
        if let Some(ref resource) = query.resource {
            sql.push_str(" AND resource = ?");
            param_values.push(Box::new(resource.clone()));
        }
        if let Some(ref result_val) = query.result {
            sql.push_str(" AND result = ?");
            param_values.push(Box::new(result_val.clone()));
        }
        if let Some(after) = query.after {
            sql.push_str(" AND timestamp >= ?");
            param_values.push(Box::new(after.to_rfc3339()));
        }
        if let Some(before) = query.before {
            sql.push_str(" AND timestamp <= ?");
            param_values.push(Box::new(before.to_rfc3339()));
        }

        sql.push_str(" ORDER BY timestamp DESC");

        let limit = query.limit.unwrap_or(100);
        let offset = query.offset.unwrap_or(0);
        sql.push_str(" LIMIT ? OFFSET ?");
        param_values.push(Box::new(limit));
        param_values.push(Box::new(offset));

        let params: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;

        let rows = stmt.query_map(params.as_slice(), |r| {
            let timestamp_str: String = r.get(1)?;
            let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    1, rusqlite::types::Type::Text, Box::new(e),
                ))?;
            let details_str: Option<String> = r.get(7)?;
            let details = details_str.and_then(|s| serde_json::from_str(&s).ok());

            Ok(AuditLogEntry {
                id: r.get(0)?,
                timestamp,
                user_id: r.get(2)?,
                action: r.get(3)?,
                resource: r.get(4)?,
                result: r.get(5)?,
                ip_address: r.get(6)?,
                details,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn purge_old_audit_logs(&self, retention_days: i64) -> Result<u64, rusqlite::Error> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days)).to_rfc3339();
        let count = self.conn.lock().unwrap().execute(
            "DELETE FROM audit_logs WHERE timestamp < ?1",
            params![cutoff],
        )?;
        Ok(count as u64)
    }

    /// Audit-log entries for release-related requests against a given vault
    /// (e.g. POST /api/vaults/{id}/sponsored-release, GET .../simulate-release)
    /// — Issue #1173.
    pub fn get_vault_release_audit_logs(
        &self,
        vault_id: &str,
    ) -> Result<Vec<crate::models::AuditLogEntry>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%/vaults/{}/%release%", vault_id);
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, user_id, action, resource, result, ip_address, details
             FROM audit_logs WHERE resource LIKE ?1 ORDER BY timestamp DESC",
        )?;
        let rows = stmt.query_map(params![pattern], |r| {
            let timestamp_str: String = r.get(1)?;
            let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
                })?;
            let details_str: Option<String> = r.get(7)?;
            let details = details_str.and_then(|s| serde_json::from_str(&s).ok());
            Ok(crate::models::AuditLogEntry {
                id: r.get(0)?,
                timestamp,
                user_id: r.get(2)?,
                action: r.get(3)?,
                resource: r.get(4)?,
                result: r.get(5)?,
                ip_address: r.get(6)?,
                details,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    // ── Refresh token rotation (Issue #1177) ─────────────────────────────────

    pub fn insert_refresh_token(
        &self,
        jti: &str,
        family_id: &str,
        sub: &str,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"INSERT INTO refresh_tokens (jti, family_id, sub, created_at, expires_at, revoked_at)
               VALUES (?1, ?2, ?3, ?4, ?5, NULL)"#,
            params![
                jti,
                family_id,
                sub,
                chrono::Utc::now().to_rfc3339(),
                expires_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Returns (family_id, sub, revoked) for a refresh token, or None if it
    /// was never issued (unknown / already garbage-collected).
    pub fn get_refresh_token(
        &self,
        jti: &str,
    ) -> Result<Option<(String, String, bool)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT family_id, sub, revoked_at FROM refresh_tokens WHERE jti = ?1",
        )?;
        let row = stmt.query_row(params![jti], |r| {
            let revoked_at: Option<String> = r.get(2)?;
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, revoked_at.is_some()))
        });
        match row {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn revoke_refresh_token(&self, jti: &str) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "UPDATE refresh_tokens SET revoked_at = ?1 WHERE jti = ?2 AND revoked_at IS NULL",
            params![chrono::Utc::now().to_rfc3339(), jti],
        )?;
        Ok(())
    }

    /// Revokes every token in a rotation family — used when a refresh token
    /// that was already rotated out gets presented again, which indicates
    /// the token may have been stolen and replayed.
    pub fn revoke_refresh_token_family(&self, family_id: &str) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "UPDATE refresh_tokens SET revoked_at = ?1 WHERE family_id = ?2 AND revoked_at IS NULL",
            params![chrono::Utc::now().to_rfc3339(), family_id],
        )?;
        Ok(())
    }

    // ── Sponsored Release tracking (#1122) ───────────────────────────────────

    pub fn insert_sponsored_release(&self, release: &crate::fee_sponsorship::SponsoredRelease) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO sponsored_releases (
                tx_id, vault_id, beneficiary, released_amount, protocol_fee, net_amount,
                fee_bump_tx_hash, sponsor_account, sponsorship_fee, status, created_at,
                executed_at, ledger_sequence, error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            "#,
            params![
                release.tx_id,
                release.vault_id,
                release.beneficiary,
                release.released_amount,
                release.protocol_fee,
                release.net_amount,
                release.fee_bump_tx_hash,
                release.sponsor_account,
                release.sponsorship_fee,
                release.status.to_string(),
                release.created_at.to_rfc3339(),
                release.executed_at.map(|dt| dt.to_rfc3339()),
                release.ledger_sequence.map(|seq| seq as i64),
                release.error.as_deref(),
            ],
        )?;
        Ok(())
    }

    pub fn get_sponsored_release(&self, tx_id: &str) -> Result<Option<crate::fee_sponsorship::SponsoredRelease>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"
            SELECT tx_id, vault_id, beneficiary, released_amount, protocol_fee, net_amount,
                   fee_bump_tx_hash, sponsor_account, sponsorship_fee, status, created_at,
                   executed_at, ledger_sequence, error
            FROM sponsored_releases
            WHERE tx_id = ?1
            "#,
        )?;

        let row = stmt.query_row(params![tx_id], |r| {
            let status_str: String = r.get(9)?;
            let status = match status_str.as_str() {
                "pending" => crate::fee_sponsorship::SponsoredReleaseStatus::Pending,
                "submitted" => crate::fee_sponsorship::SponsoredReleaseStatus::Submitted,
                "confirmed" => crate::fee_sponsorship::SponsoredReleaseStatus::Confirmed,
                "failed" => crate::fee_sponsorship::SponsoredReleaseStatus::Failed,
                "cancelled" => crate::fee_sponsorship::SponsoredReleaseStatus::Cancelled,
                _ => crate::fee_sponsorship::SponsoredReleaseStatus::Pending,
            };

            let created_at_str: String = r.get(10)?;
            let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, Box::new(e)))?;

            let executed_at_str: Option<String> = r.get(11)?;
            let executed_at = executed_at_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            });

            let ledger_sequence: Option<i64> = r.get(12)?;

            Ok(crate::fee_sponsorship::SponsoredRelease {
                tx_id: r.get(0)?,
                vault_id: r.get(1)?,
                beneficiary: r.get(2)?,
                released_amount: r.get(3)?,
                protocol_fee: r.get(4)?,
                net_amount: r.get(5)?,
                fee_bump_tx_hash: r.get(6)?,
                sponsor_account: r.get(7)?,
                sponsorship_fee: r.get(8)?,
                status,
                created_at,
                executed_at,
                ledger_sequence: ledger_sequence.map(|s| s as u64),
                error: r.get(13)?,
            })
        });

        match row {
            Ok(release) => Ok(Some(release)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn list_sponsored_releases_for_vault(&self, vault_id: &str) -> Result<Vec<crate::fee_sponsorship::SponsoredRelease>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"
            SELECT tx_id, vault_id, beneficiary, released_amount, protocol_fee, net_amount,
                   fee_bump_tx_hash, sponsor_account, sponsorship_fee, status, created_at,
                   executed_at, ledger_sequence, error
            FROM sponsored_releases
            WHERE vault_id = ?1
            ORDER BY created_at DESC
            "#,
        )?;

        let iter = stmt.query_map(params![vault_id], |r| {
            let status_str: String = r.get(9)?;
            let status = match status_str.as_str() {
                "pending" => crate::fee_sponsorship::SponsoredReleaseStatus::Pending,
                "submitted" => crate::fee_sponsorship::SponsoredReleaseStatus::Submitted,
                "confirmed" => crate::fee_sponsorship::SponsoredReleaseStatus::Confirmed,
                "failed" => crate::fee_sponsorship::SponsoredReleaseStatus::Failed,
                "cancelled" => crate::fee_sponsorship::SponsoredReleaseStatus::Cancelled,
                _ => crate::fee_sponsorship::SponsoredReleaseStatus::Pending,
            };

            let created_at_str: String = r.get(10)?;
            let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, Box::new(e)))?;

            let executed_at_str: Option<String> = r.get(11)?;
            let executed_at = executed_at_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            });

            let ledger_sequence: Option<i64> = r.get(12)?;

            Ok(crate::fee_sponsorship::SponsoredRelease {
                tx_id: r.get(0)?,
                vault_id: r.get(1)?,
                beneficiary: r.get(2)?,
                released_amount: r.get(3)?,
                protocol_fee: r.get(4)?,
                net_amount: r.get(5)?,
                fee_bump_tx_hash: r.get(6)?,
                sponsor_account: r.get(7)?,
                sponsorship_fee: r.get(8)?,
                status,
                created_at,
                executed_at,
                ledger_sequence: ledger_sequence.map(|s| s as u64),
                error: r.get(13)?,
            })
        })?;

        let mut out = Vec::new();
        for item in iter {
            out.push(item?);
        }
        Ok(out)
    }

    pub fn update_sponsored_release_status(
        &self,
        tx_id: &str,
        status: crate::fee_sponsorship::SponsoredReleaseStatus,
    ) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"
            UPDATE sponsored_releases
            SET status = ?1
            WHERE tx_id = ?2
            "#,
            params![status.to_string(), tx_id],
        )?;
        Ok(())
    }

    pub fn confirm_sponsored_release(
        &self,
        tx_id: &str,
        ledger_sequence: u64,
    ) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"
            UPDATE sponsored_releases
            SET status = ?, executed_at = ?, ledger_sequence = ?
            WHERE tx_id = ?
            "#,
            params![
                crate::fee_sponsorship::SponsoredReleaseStatus::Confirmed.to_string(),
                chrono::Utc::now().to_rfc3339(),
                ledger_sequence as i64,
                tx_id,
            ],
        )?;
        Ok(())
    }

    pub fn fail_sponsored_release(
        &self,
        tx_id: &str,
        error: &str,
    ) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"
            UPDATE sponsored_releases
            SET status = ?, error = ?, executed_at = ?
            WHERE tx_id = ?
            "#,
            params![
                crate::fee_sponsorship::SponsoredReleaseStatus::Failed.to_string(),
                error,
                chrono::Utc::now().to_rfc3339(),
                tx_id,
            ],
        )?;
        Ok(())
    }

    pub fn get_vesting_bonus(&self, vault_id: &str) -> Result<Option<crate::models::VestingBonusConfig>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT vault_id, bonus_bps, on_time_window_seconds
               FROM vesting_bonus_config
               WHERE vault_id = ?1"#,
        )?;
        let result = stmt.query_row(params![vault_id], |r| {
            Ok(crate::models::VestingBonusConfig {
                vault_id: r.get(0)?,
                bonus_bps: r.get(1)?,
                on_time_window_seconds: r.get(2)?,
            })
        });
        match result {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn upsert_vesting_bonus(&self, config: &crate::models::VestingBonusConfig) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO vesting_bonus_config (vault_id, bonus_bps, on_time_window_seconds)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(vault_id) DO UPDATE SET
              bonus_bps = excluded.bonus_bps,
              on_time_window_seconds = excluded.on_time_window_seconds
            "#,
            params![
                config.vault_id,
                config.bonus_bps as i64,
                config.on_time_window_seconds as i64,
            ],
        )?;
        Ok(())
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

// ── Cache-aware vault accessors ───────────────────────────────────────────────

use crate::cache::VaultCache;
use crate::models::VaultSummary;

/// Retrieve a `Vault` from the in-memory store, consulting the cache first.
///
/// On a cache miss the vault is fetched from `store`, inserted into the cache
/// and then returned. Returns `None` if the vault does not exist in the store.
pub fn get_vault_cached(
    store: &VaultStore,
    cache: &VaultCache,
    vault_id: &str,
) -> Option<crate::models::Vault> {
    if let Some(v) = cache.get_vault(vault_id) {
        return Some(v);
    }
    let vault = store.lock().unwrap().get(vault_id).cloned()?;
    cache.set_vault(vault_id, vault.clone());
    Some(vault)
}

/// Retrieve the TTL-remaining value for a vault, consulting the cache first.
///
/// Returns `None` if the vault does not exist in the store.
pub fn get_ttl_remaining_cached(
    store: &VaultStore,
    cache: &VaultCache,
    vault_id: &str,
) -> Option<Option<u64>> {
    if let Some(ttl) = cache.get_ttl_remaining(vault_id) {
        return Some(ttl);
    }
    let vault = store.lock().unwrap().get(vault_id).cloned()?;
    let ttl = vault.ttl_remaining;
    cache.set_ttl_remaining(vault_id, ttl);
    Some(ttl)
}

/// Retrieve a lightweight `VaultSummary` for a vault, consulting the cache
/// first.
///
/// Returns `None` if the vault does not exist in the store.
pub fn get_vault_summary_cached(
    store: &VaultStore,
    cache: &VaultCache,
    vault_id: &str,
) -> Option<VaultSummary> {
    if let Some(s) = cache.get_vault_summary(vault_id) {
        return Some(s);
    }
    let vault = store.lock().unwrap().get(vault_id).cloned()?;
    let summary = VaultSummary::from(&vault);
    cache.set_vault_summary(vault_id, summary.clone());
    Some(summary)
}

/// Invalidate all cached entries for `vault_id`.  Must be called whenever
/// a check-in or state-change event modifies vault state.
pub fn invalidate_vault_cache(cache: &VaultCache, vault_id: &str) {
    cache.invalidate(vault_id);
}
