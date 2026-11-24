use crate::models::{
    AuditEntry, AuditLogEntry, AuditLogQuery, BeneficiaryPayoutAddress, Channel, Frequency,
    Passkey, RecoveryCode, RecoveryCodeSet, ReminderPreferences, SearchQuery, SearchResult,
    Subscription, SubscriptionChannel, SubscriptionFrequency, Vault, VaultBackup, VaultEvent,
    VaultNotificationPreferences, VaultShare, VaultStatus, WebhookSubscription,
};

use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod auth;
mod notifications;
mod vault;

pub use notifications::*;
pub use vault::*;

pub type VaultStore = Arc<Mutex<HashMap<String, Vault>>>;
pub type EventStore = Arc<Mutex<Vec<VaultEvent>>>;
pub type AuditStore = Arc<Mutex<Vec<AuditEntry>>>;
pub type BackupStore = Arc<Mutex<HashMap<String, VaultBackup>>>;
pub type ShareStore = Arc<Mutex<Vec<VaultShare>>>;
pub type ShareTokenStore = Arc<Mutex<HashMap<String, ShareToken>>>;
pub type NotificationStore = Arc<Mutex<HashMap<String, VaultNotificationPreferences>>>;
pub type PasskeyStore = Arc<Mutex<Vec<Passkey>>>;
pub type RecoveryCodeStore = Arc<Mutex<Vec<RecoveryCode>>>;
pub type RecoveryCodeSetStore = Arc<Mutex<Vec<RecoveryCodeSet>>>;

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

pub type PayoutAddressStore = Arc<Mutex<HashMap<String, BeneficiaryPayoutAddress>>>;

pub fn create_payout_address_store() -> PayoutAddressStore {
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
    pub passkey_store: PasskeyStore,
    pub recovery_code_store: RecoveryCodeStore,
    pub recovery_code_set_store: RecoveryCodeSetStore,
}

// ── Audit helper ─────────────────────────────────────────────────────────────

pub fn append_audit_entry(
    audit_store: &AuditStore,
    action: &str,
    actor: &str,
    details: serde_json::Value,
) {
    audit_store.lock().unwrap().push(AuditEntry {
        timestamp: Utc::now(),
        action: action.to_string(),
        actor: actor.to_string(),
        details,
    });
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
        let last_extended_at = policy.last_extended_at.map(|d| d.to_rfc3339());

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

    pub fn get_insurance_policy(
        &self,
        vault_id: u64,
    ) -> Result<Option<TtlInsurancePolicy>, rusqlite::Error> {
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
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

            let last_extended_at: Option<String> = r.get(5)?;
            let last_extended_at_dt = match last_extended_at {
                Some(s) => {
                    let dt = chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                0,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })?;
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

    pub fn upsert_owner_activity(
        &self,
        owner_id: u64,
        last_active_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO owner_activity (owner_id, last_active_at)
            VALUES (?1, ?2)
            ON CONFLICT(owner_id) DO UPDATE SET
                last_active_at = excluded.last_active_at
            "#,
            params![owner_id as i64, last_active_at.to_rfc3339(),],
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

        let row_res: Result<String, rusqlite::Error> =
            stmt.query_row(params![owner_id as i64], |r| r.get(0));

        match row_res {
            Ok(s) => {
                let dt = chrono::DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                Ok(Some(dt))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn all_enabled_insurance_policies(
        &self,
    ) -> Result<Vec<TtlInsurancePolicy>, rusqlite::Error> {
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
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

            let last_extended_at: Option<String> = r.get(5)?;
            let last_extended_at_dt = match last_extended_at {
                Some(s) => {
                    let dt = chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                0,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })?;
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
            (
                "6",
                r#"
                CREATE TABLE IF NOT EXISTS withdrawal_alert_preferences (
                    vault_id      INTEGER PRIMARY KEY,
                    owner         TEXT    NOT NULL,
                    email_enabled INTEGER NOT NULL DEFAULT 0,
                    push_enabled  INTEGER NOT NULL DEFAULT 0
                );
                CREATE INDEX IF NOT EXISTS idx_withdrawal_alert_prefs_owner
                    ON withdrawal_alert_preferences(owner);
                "#,
            ),
            (
                "7",
                r#"
                CREATE TABLE IF NOT EXISTS state_transitions (
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    vault_id        TEXT NOT NULL,
                    from_status     TEXT NOT NULL,
                    to_status       TEXT NOT NULL,
                    timestamp       TEXT NOT NULL,
                    triggered_by    TEXT NOT NULL,
                    trigger_reason  TEXT,
                    details         TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_state_transitions_vault_id ON state_transitions(vault_id);
                CREATE INDEX IF NOT EXISTS idx_state_transitions_timestamp ON state_transitions(timestamp);
                CREATE INDEX IF NOT EXISTS idx_state_transitions_triggered_by ON state_transitions(triggered_by);
                "#,
            ),
            // Issue #2069: vault full-text search index (FTS5). Mirrors
            // migrations/0010_vault_search_fulltext.sql so db.migrate() creates
            // the tables even when only the in-Rust runner is invoked.
            (
                "10",
                r#"
                CREATE TABLE IF NOT EXISTS vault_metadata (
                    vault_id   TEXT PRIMARY KEY,
                    owner      TEXT NOT NULL,
                    tags       TEXT NOT NULL DEFAULT '[]',
                    notes      TEXT NOT NULL DEFAULT '',
                    updated_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_vault_metadata_owner ON vault_metadata(owner);
                CREATE VIRTUAL TABLE IF NOT EXISTS vault_search_fts USING fts5(
                    vault_id UNINDEXED,
                    owner    UNINDEXED,
                    tags,
                    notes,
                    tokenize = 'unicode61'
                );
                "#,
            ),
            // Issue #2068: check-in delegation audit log. Mirrors
            // migrations/0011_check_in_delegation_audit.sql.
            (
                "11",
                r#"
                CREATE TABLE IF NOT EXISTS check_in_delegations (
                    vault_id    TEXT PRIMARY KEY,
                    owner       TEXT NOT NULL,
                    delegate    TEXT,
                    granted_at  TEXT,
                    updated_at  TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_check_in_delegations_delegate ON check_in_delegations(delegate);
                CREATE TABLE IF NOT EXISTS check_in_delegation_audit_log (
                    id         INTEGER PRIMARY KEY AUTOINCREMENT,
                    vault_id   TEXT NOT NULL,
                    event_kind TEXT NOT NULL,
                    delegate   TEXT,
                    actor      TEXT NOT NULL DEFAULT '',
                    timestamp  TEXT NOT NULL,
                    details    TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_check_in_delegation_audit_vault_id  ON check_in_delegation_audit_log(vault_id);
                CREATE INDEX IF NOT EXISTS idx_check_in_delegation_audit_timestamp ON check_in_delegation_audit_log(timestamp);
                "#,
            ),
            // Issue #2070: multi-party withdrawal approval workflow. Mirrors
            // migrations/0012_withdrawal_approval_workflow.sql.
            (
                "12",
                r#"
                CREATE TABLE IF NOT EXISTS withdrawal_approval_workflows (
                    withdrawal_id          TEXT PRIMARY KEY,
                    vault_id               TEXT NOT NULL,
                    amount                 INTEGER NOT NULL,
                    requested_by           TEXT NOT NULL,
                    required_approvers     TEXT NOT NULL DEFAULT '[]',
                    approval_ttl_seconds   INTEGER NOT NULL,
                    created_at              TEXT NOT NULL,
                    expires_at              TEXT NOT NULL,
                    status                  TEXT NOT NULL DEFAULT 'pending',
                    executed_tx_hash        TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_withdrawal_approval_workflows_vault_id ON withdrawal_approval_workflows(vault_id);
                CREATE INDEX IF NOT EXISTS idx_withdrawal_approval_workflows_status   ON withdrawal_approval_workflows(status);
                CREATE INDEX IF NOT EXISTS idx_withdrawal_approval_workflows_expires  ON withdrawal_approval_workflows(expires_at);
                CREATE TABLE IF NOT EXISTS withdrawal_approvals (
                    withdrawal_id  TEXT NOT NULL,
                    approver       TEXT NOT NULL,
                    approved_at    TEXT NOT NULL,
                    PRIMARY KEY (withdrawal_id, approver)
                );
                CREATE INDEX IF NOT EXISTS idx_withdrawal_approvals_approver ON withdrawal_approvals(approver);
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

    pub fn get_webhook_subscriptions_for_vault(
        &self,
        vault_id: &str,
    ) -> Result<Vec<WebhookSubscription>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"SELECT id, vault_id, endpoint_url, event_types, created_at
               FROM webhook_subscriptions
               WHERE vault_id = ?1"#,
        )?;
        let rows = stmt.query_map(params![vault_id], |r| {
            let event_types_str: String = r.get(3)?;
            let created_at_str: String = r.get(4)?;
            Ok(WebhookSubscription {
                id: r.get(0)?,
                vault_id: r.get(1)?,
                endpoint_url: r.get(2)?,
                event_types: serde_json::from_str(&event_types_str).unwrap_or_default(),
                created_at: created_at_str.parse().unwrap_or_else(|_| Utc::now()),
            })
        })?;
        rows.collect()
    }

    // ── Idempotency (#825) ──────────────────────────────────────────────────

    pub fn store_idempotency(&self, key: &str, status_code: u16, response_body: &str) {
        let _ = self.conn.lock().unwrap().execute(
            r#"INSERT OR REPLACE INTO idempotency_keys (key, status_code, response_body, created_at)
               VALUES (?1, ?2, ?3, ?4)"#,
            params![
                key,
                status_code as i64,
                response_body,
                chrono::Utc::now().to_rfc3339()
            ],
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
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
            let age = chrono::Utc::now()
                .signed_duration_since(created_at)
                .num_seconds();
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

    pub fn query_audit_logs(
        &self,
        query: &AuditLogQuery,
    ) -> Result<Vec<AuditLogEntry>, rusqlite::Error> {
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

        let params: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;

        let rows = stmt.query_map(params.as_slice(), |r| {
            let timestamp_str: String = r.get(1)?;
            let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
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
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
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

    /// Record a vault state transition for compliance reporting — Issue #1854.
    pub fn insert_state_transition(
        &self,
        entry: &crate::models::StateTransitionEntry,
    ) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO state_transitions (vault_id, from_status, to_status, timestamp, triggered_by, trigger_reason, details)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                entry.vault_id,
                format!("{:?}", entry.from_status),
                format!("{:?}", entry.to_status),
                entry.timestamp.to_rfc3339(),
                entry.triggered_by,
                entry.trigger_reason.as_deref().unwrap_or(""),
                entry.details.as_ref().map(|d| d.to_string()),
            ],
        )?;
        Ok(())
    }

    /// Query state transitions for a vault — Issue #1854.
    pub fn query_state_transitions(
        &self,
        query: &crate::models::StateTransitionQuery,
    ) -> Result<Vec<crate::models::StateTransitionEntry>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT id, vault_id, from_status, to_status, timestamp, triggered_by, trigger_reason, details FROM state_transitions WHERE 1=1"
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref vault_id) = query.vault_id {
            sql.push_str(" AND vault_id = ?");
            param_values.push(Box::new(vault_id.clone()));
        }
        if let Some(ref from_status) = query.from_status {
            sql.push_str(" AND from_status = ?");
            param_values.push(Box::new(format!("{:?}", from_status)));
        }
        if let Some(ref to_status) = query.to_status {
            sql.push_str(" AND to_status = ?");
            param_values.push(Box::new(format!("{:?}", to_status)));
        }
        if let Some(ref triggered_by) = query.triggered_by {
            sql.push_str(" AND triggered_by = ?");
            param_values.push(Box::new(triggered_by.clone()));
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

        let params: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;

        let rows = stmt.query_map(params.as_slice(), |r| {
            let timestamp_str: String = r.get(4)?;
            let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
            let from_status_str: String = r.get(2)?;
            let to_status_str: String = r.get(3)?;
            let trigger_reason: Option<String> = r.get(6)?;
            let details_str: Option<String> = r.get(7)?;
            let details = details_str.and_then(|s| serde_json::from_str(&s).ok());

            Ok(crate::models::StateTransitionEntry {
                id: r.get(0)?,
                vault_id: r.get(1)?,
                from_status: match from_status_str.as_str() {
                    "Active" => crate::models::VaultStatus::Active,
                    "Expired" => crate::models::VaultStatus::Expired,
                    "Released" => crate::models::VaultStatus::Released,
                    "Paused" => crate::models::VaultStatus::Paused,
                    _ => crate::models::VaultStatus::Active,
                },
                to_status: match to_status_str.as_str() {
                    "Active" => crate::models::VaultStatus::Active,
                    "Expired" => crate::models::VaultStatus::Expired,
                    "Released" => crate::models::VaultStatus::Released,
                    "Paused" => crate::models::VaultStatus::Paused,
                    _ => crate::models::VaultStatus::Active,
                },
                timestamp,
                triggered_by: r.get(5)?,
                trigger_reason: if trigger_reason.is_empty() { None } else { trigger_reason },
                details,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Get state transition history for a specific vault — Issue #1854.
    pub fn get_vault_state_transitions(
        &self,
        vault_id: &str,
    ) -> Result<crate::models::StateTransitionHistory, rusqlite::Error> {
        let transitions = self.query_state_transitions(&crate::models::StateTransitionQuery {
            vault_id: Some(vault_id.to_string()),
            ..Default::default()
        })?;
        Ok(crate::models::StateTransitionHistory {
            vault_id: vault_id.to_string(),
            transitions,
        })
    }

    // ── Vault pause/resume history (Issue #2053) ─────────────────────────────

    /// Record a vault pause or resume event emitted on-chain.
    ///
    /// `event_type` must be `"pause"` or `"resume"`.
    pub fn insert_pause_event(
        &self,
        vault_id: &str,
        actor: &str,
        event_type: &str,
        reason: &str,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO vault_pause_history (vault_id, actor, event_type, reason, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                vault_id,
                actor,
                event_type,
                reason,
                timestamp.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Return paginated pause/resume events for a vault, newest first.
    ///
    /// Returns `(events, total_count)`.
    pub fn get_pause_history(
        &self,
        vault_id: &str,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<crate::models::PauseEvent>, u32), rusqlite::Error> {
        let page_size = page_size.clamp(1, 100);
        let offset = page * page_size;

        let conn = self.conn.lock().unwrap();

        let total: u32 = conn.query_row(
            "SELECT COUNT(*) FROM vault_pause_history WHERE vault_id = ?1",
            params![vault_id],
            |row| row.get(0),
        )?;

        let mut stmt = conn.prepare(
            "SELECT id, vault_id, actor, event_type, reason, timestamp
               FROM vault_pause_history
              WHERE vault_id = ?1
              ORDER BY timestamp DESC
              LIMIT ?2 OFFSET ?3",
        )?;

        let events = stmt
            .query_map(params![vault_id, page_size, offset], |row| {
                let ts_str: String = row.get(5)?;
                let timestamp = chrono::DateTime::parse_from_rfc3339(&ts_str)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());
                Ok(crate::models::PauseEvent {
                    id: row.get(0)?,
                    vault_id: row.get(1)?,
                    actor: row.get(2)?,
                    event_type: row.get(3)?,
                    reason: row.get(4)?,
                    timestamp,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok((events, total))
    }

    // ── Refresh token rotation (Issue #1177) ─────────────────────────────────

    pub fn insert_refresh_token(        &self,
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
        let mut stmt =
            conn.prepare("SELECT family_id, sub, revoked_at FROM refresh_tokens WHERE jti = ?1")?;
        let row = stmt.query_row(params![jti], |r| {
            let revoked_at: Option<String> = r.get(2)?;
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                revoked_at.is_some(),
            ))
        });
        match row {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Atomically revokes a single refresh token. Returns `false` if it was
    /// already revoked (e.g. by a concurrent refresh), so callers can treat
    /// the loser of a rotation race as a reuse.
    pub fn revoke_refresh_token(&self, jti: &str) -> Result<bool, rusqlite::Error> {
        let updated = self.conn.lock().unwrap().execute(
            "UPDATE refresh_tokens SET revoked_at = ?1 WHERE jti = ?2 AND revoked_at IS NULL",
            params![chrono::Utc::now().to_rfc3339(), jti],
        )?;
        Ok(updated > 0)
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

    pub fn insert_sponsored_release(
        &self,
        release: &crate::fee_sponsorship::SponsoredRelease,
    ) -> Result<(), rusqlite::Error> {
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

    pub fn get_sponsored_release(
        &self,
        tx_id: &str,
    ) -> Result<Option<crate::fee_sponsorship::SponsoredRelease>, rusqlite::Error> {
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
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        10,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

    pub fn list_sponsored_releases_for_vault(
        &self,
        vault_id: &str,
    ) -> Result<Vec<crate::fee_sponsorship::SponsoredRelease>, rusqlite::Error> {
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
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        10,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

    pub fn fail_sponsored_release(&self, tx_id: &str, error: &str) -> Result<(), rusqlite::Error> {
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

    pub fn get_vesting_bonus(
        &self,
        vault_id: &str,
    ) -> Result<Option<crate::models::VestingBonusConfig>, rusqlite::Error> {
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

    pub fn upsert_vesting_bonus(
        &self,
        config: &crate::models::VestingBonusConfig,
    ) -> Result<(), rusqlite::Error> {
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

    // ── Issue #1594: Escalation tier transitions ────────────────────────────

    pub fn get_escalation_state(
        &self,
        vault_id: u64,
    ) -> Result<Option<crate::models::EscalationState>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"
            SELECT vault_id, last_escalation_tier, escalated_at
            FROM escalation_states
            WHERE vault_id = ?1
            "#,
        )?;

        let row_res = stmt.query_row(params![vault_id as i64], |r| {
            let tier_str: Option<String> = r.get(1)?;
            let tier = tier_str.as_ref().map(|t| {
                match t.as_str() {
                    "T1" => crate::models::EscalationTier::T1,
                    "T2" => crate::models::EscalationTier::T2,
                    "T3" => crate::models::EscalationTier::T3,
                    _ => crate::models::EscalationTier::T1,
                }
            });

            let escalated_at_str: Option<String> = r.get(2)?;
            let escalated_at = escalated_at_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .ok()
            });

            Ok(crate::models::EscalationState {
                vault_id: r.get(0)?,
                last_escalation_tier: tier,
                escalated_at,
            })
        });

        match row_res {
            Ok(state) => Ok(Some(state)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn upsert_escalation_state(
        &self,
        state: &crate::models::EscalationState,
    ) -> Result<(), rusqlite::Error> {
        let tier_str = state
            .last_escalation_tier
            .map(|t| format!("{:?}", t));
        let escalated_at_str = state.escalated_at.map(|dt| dt.to_rfc3339());

        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO escalation_states (vault_id, last_escalation_tier, escalated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(vault_id) DO UPDATE SET
              last_escalation_tier = excluded.last_escalation_tier,
              escalated_at = excluded.escalated_at
            "#,
            params![state.vault_id as i64, tier_str, escalated_at_str],
        )?;
        Ok(())
    }

    pub fn insert_escalation_event(
        &self,
        event: &crate::models::EscalationEvent,
    ) -> Result<(), rusqlite::Error> {
        let channels_json = serde_json::to_string(&event.channels).unwrap_or_else(|_| "[]".to_string());
        let tier_str = format!("{:?}", event.tier);

        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO escalation_events (id, vault_id, tier, dispatched_at, channels)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                event.id,
                event.vault_id as i64,
                tier_str,
                event.dispatched_at.to_rfc3339(),
                channels_json,
            ],
        )?;
        Ok(())
    }

    pub fn get_escalation_events(
        &self,
        vault_id: u64,
    ) -> Result<Vec<crate::models::EscalationEvent>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"
            SELECT id, vault_id, tier, dispatched_at, channels
            FROM escalation_events
            WHERE vault_id = ?1
            ORDER BY dispatched_at DESC
            "#,
        )?;

        let iter = stmt.query_map(params![vault_id as i64], |r| {
            let tier_str: String = r.get(2)?;
            let tier = match tier_str.as_str() {
                "T1" => crate::models::EscalationTier::T1,
                "T2" => crate::models::EscalationTier::T2,
                "T3" => crate::models::EscalationTier::T3,
                _ => crate::models::EscalationTier::T1,
            };

            let channels_json_str: String = r.get(4)?;
            let channels: Vec<String> = serde_json::from_str(&channels_json_str).unwrap_or_default();

            let dispatched_at_str: String = r.get(3)?;
            let dispatched_at = chrono::DateTime::parse_from_rfc3339(&dispatched_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            Ok(crate::models::EscalationEvent {
                id: r.get(0)?,
                vault_id: r.get::<_, i64>(1)? as u64,
                tier,
                dispatched_at,
                channels,
            })
        })?;

        let mut out = Vec::new();
        for item in iter {
            out.push(item?);
        }
        Ok(out)
    }

    pub fn clear_escalation_state(&self, vault_id: u64) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "DELETE FROM escalation_states WHERE vault_id = ?1",
            params![vault_id as i64],
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
            limit: None,
            cursor: None,
        };

        let result = search_vaults(&store, &query);
        assert_eq!(result.vaults.len(), 1);
        assert!(result.next_cursor.is_none());
    }

    #[test]
    fn test_search_vaults_pagination() {
        let store = create_vault_store();
        for i in 0..25 {
            let vault = Vault {
                id: format!("v{:02}", i),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 1000,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(100000),
            };
            store.lock().unwrap().insert(format!("v{:02}", i), vault);
        }

        // First page — no cursor.
        let query = SearchQuery {
            owner: Some("owner1".to_string()),
            beneficiary: None,
            status: None,
            created_after: None,
            created_before: None,
            limit: Some(10),
            cursor: None,
        };

        let result = search_vaults(&store, &query);
        assert_eq!(result.vaults.len(), 10);
        assert!(result.next_cursor.is_some());

        // Second page — use the cursor from the first page.
        let query2 = SearchQuery {
            owner: Some("owner1".to_string()),
            beneficiary: None,
            status: None,
            created_after: None,
            created_before: None,
            limit: Some(10),
            cursor: result.next_cursor,
        };

        let result2 = search_vaults(&store, &query2);
        assert_eq!(result2.vaults.len(), 10);
        assert!(result2.next_cursor.is_some());

        // Third (last) page — no more results after this.
        let query3 = SearchQuery {
            owner: Some("owner1".to_string()),
            beneficiary: None,
            status: None,
            created_after: None,
            created_before: None,
            limit: Some(10),
            cursor: result2.next_cursor,
        };

        let result3 = search_vaults(&store, &query3);
        assert_eq!(result3.vaults.len(), 5);
        assert!(result3.next_cursor.is_none());
    }
}

// ── Issue #2069: vault full-text search over tags and notes ──────────────────
//
// `vault_metadata` holds owner-supplied tags/notes (JSON tags); `vault_search_fts`
// is an FTS5 virtual table kept in sync by DELETE + INSERT on every upsert. The
// `search_vaults_fulltext` query MATCHes the FTS index scoped to the caller's
// owner address and returns highlighted snippets via FTS5 `snippet()`.

/// Build a safe FTS5 MATCH expression from a free-text query.
///
/// Splits on whitespace, strips quotes, and emits each term as a quoted FTS5
/// phrase, joined with implicit-AND (whitespace). This avoids the query syntax
/// in FTS5 MATCH (`*`, `:`, `"`, `NEAR`, …) being interpreted from raw input,
/// which would otherwise let a stray `*` or `:` produce surprising results or
/// a parse error.
fn fts5_query(q: &str) -> String {
    q.split_whitespace()
        .map(|t| t.trim_matches(|c: char| c == '"'))
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse an RFC3339 timestamp stored in SQLite back into a UTC `DateTime`.
fn parse_rfc3339(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

impl Db {
    /// Insert or replace a vault's searchable metadata and refresh the FTS5
    /// index for it.
    pub fn upsert_vault_metadata(
        &self,
        vault_id: &str,
        owner: &str,
        tags: &[String],
        notes: &str,
    ) -> Result<(), rusqlite::Error> {
        let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
        // The FTS `tags` column is tokenised, so join with whitespace so each
        // tag is its own searchable token.
        let tags_text = tags.join(" ");
        let now = chrono::Utc::now().to_rfc3339();

        let conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            r#"INSERT INTO vault_metadata (vault_id, owner, tags, notes, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5)
               ON CONFLICT(vault_id) DO UPDATE SET
                   owner = excluded.owner,
                   tags = excluded.tags,
                   notes = excluded.notes,
                   updated_at = excluded.updated_at"#,
            params![vault_id, owner, tags_json, notes, now],
        )?;
        // Keep the FTS index in sync: drop any prior entry for this vault, then
        // insert the freshly indexed text.
        tx.execute(
            "DELETE FROM vault_search_fts WHERE vault_id = ?1",
            params![vault_id],
        )?;
        tx.execute(
            "INSERT INTO vault_search_fts (vault_id, owner, tags, notes) VALUES (?1, ?2, ?3, ?4)",
            params![vault_id, owner, tags_text, notes],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Retrieve a vault's stored metadata, if any.
    pub fn get_vault_metadata(
        &self,
        vault_id: &str,
    ) -> Result<Option<crate::models::VaultMetadata>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let res = conn.query_row(
            "SELECT vault_id, owner, tags, notes, updated_at FROM vault_metadata WHERE vault_id = ?1",
            params![vault_id],
            |r| {
                let tags_json: String = r.get(2)?;
                let notes: String = r.get(3)?;
                let updated_str: String = r.get(4)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                Ok(crate::models::VaultMetadata {
                    vault_id: r.get(0)?,
                    owner: r.get(1)?,
                    tags,
                    notes,
                    updated_at: parse_rfc3339(&updated_str).unwrap_or_else(chrono::Utc::now),
                })
            },
        );
        match res {
            Ok(m) => Ok(Some(m)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Remove a vault's metadata and its FTS index entry.
    pub fn delete_vault_metadata(&self, vault_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM vault_search_fts WHERE vault_id = ?1",
            params![vault_id],
        )?;
        tx.execute("DELETE FROM vault_metadata WHERE vault_id = ?1", params![vault_id])?;
        tx.commit()?;
        Ok(())
    }

    /// Full-text search over a single owner's vault tags and notes.
    ///
    /// Returns up to `limit` hits starting at `offset`, ordered by FTS5 rank
    /// (relevance). Each hit carries highlighted snippets from the matched
    /// tags and notes columns.
    pub fn search_vaults_fulltext(
        &self,
        owner: &str,
        q: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<crate::models::VaultSearchHit>, rusqlite::Error> {
        let fts = fts5_query(q);
        if fts.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"SELECT m.vault_id, m.owner, m.tags, m.notes,
                      snippet(vault_search_fts, 2, '<mark>', '</mark>', ' … ', 8),
                      snippet(vault_search_fts, 3, '<mark>', '</mark>', ' … ', 16)
               FROM vault_search_fts f
               JOIN vault_metadata m ON m.vault_id = f.vault_id
               WHERE f.vault_search_fts MATCH ?1 AND f.owner = ?2
               ORDER BY rank
               LIMIT ?3 OFFSET ?4"#,
        )?;
        let rows = stmt.query_map(params![fts, owner, limit as i64, offset as i64], |r| {
            let tags_json: String = r.get(2)?;
            let notes: String = r.get(3)?;
            let tag_snippet: Option<String> = r.get(4).ok();
            let note_snippet: Option<String> = r.get(5).ok();
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            // Prefer the notes snippet; fall back to the tags snippet.
            let snippet = note_snippet
                .filter(|s| !s.is_empty())
                .or(tag_snippet)
                .filter(|s| !s.is_empty());
            Ok(crate::models::VaultSearchHit {
                vault_id: r.get(0)?,
                owner: r.get(1)?,
                tags,
                notes,
                snippet,
                status: None,
                balance: None,
                ttl_remaining: None,
            })
        })?;
        rows.collect()
    }
}

// ── Issue #2068: check-in delegation audit log ───────────────────────────────

impl Db {
    /// Grant (or replace) the check-in delegate for a vault. Updates the
    /// current-state row only; the TTL watcher observes the transition and
    /// writes the audit-log entry.
    pub fn upsert_check_in_delegation(
        &self,
        vault_id: &str,
        owner: &str,
        delegate: &str,
    ) -> Result<(), rusqlite::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.lock().unwrap().execute(
            r#"INSERT INTO check_in_delegations (vault_id, owner, delegate, granted_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?4)
               ON CONFLICT(vault_id) DO UPDATE SET
                   owner = excluded.owner,
                   delegate = excluded.delegate,
                   granted_at = excluded.granted_at,
                   updated_at = excluded.updated_at"#,
            params![vault_id, owner, delegate, now],
        )?;
        Ok(())
    }

    /// Revoke the current check-in delegate for a vault (clears `delegate` and
    /// `granted_at` but keeps the row, so the watcher can emit a revoke event).
    pub fn clear_check_in_delegation(&self, vault_id: &str) -> Result<(), rusqlite::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.lock().unwrap().execute(
            "UPDATE check_in_delegations SET delegate = NULL, granted_at = NULL, updated_at = ?2 WHERE vault_id = ?1",
            params![vault_id, now],
        )?;
        Ok(())
    }

    /// Current check-in delegation state for a vault, if a row exists.
    pub fn get_check_in_delegation(
        &self,
        vault_id: &str,
    ) -> Result<Option<crate::models::CheckInDelegationState>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let res = conn.query_row(
            "SELECT vault_id, owner, delegate, granted_at FROM check_in_delegations WHERE vault_id = ?1",
            params![vault_id],
            |r| {
                let delegate: Option<String> = r.get(2)?;
                let granted_str: Option<String> = r.get(3)?;
                let granted_at = granted_str.and_then(|s| parse_rfc3339(&s));
                Ok(crate::models::CheckInDelegationState {
                    vault_id: r.get(0)?,
                    owner: r.get(1)?,
                    delegate,
                    granted_at,
                })
            },
        );
        match res {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Snapshot of every vault's current delegation state, used by the watcher
    /// to diff against its last-seen state.
    pub fn list_check_in_delegations(
        &self,
    ) -> Result<Vec<(String, String, Option<String>)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT vault_id, owner, delegate FROM check_in_delegations")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?;
        rows.collect()
    }

    /// Append a check-in delegation grant/revoke event to the audit log.
    pub fn insert_check_in_delegation_audit(
        &self,
        vault_id: &str,
        event_kind: crate::models::CheckInDelegationEventKind,
        delegate: Option<&str>,
        actor: &str,
        details: Option<serde_json::Value>,
    ) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"INSERT INTO check_in_delegation_audit_log
                   (vault_id, event_kind, delegate, actor, timestamp, details)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
            params![
                vault_id,
                event_kind.as_str(),
                delegate,
                actor,
                chrono::Utc::now().to_rfc3339(),
                details.as_ref().map(serde_json::Value::to_string),
            ],
        )?;
        Ok(())
    }

    /// Full audit trail of check-in delegation grant/revoke events for a vault
    /// (oldest first).
    pub fn get_check_in_delegation_audit(
        &self,
        vault_id: &str,
    ) -> Result<Vec<crate::models::CheckInDelegationAuditEntry>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, vault_id, event_kind, delegate, actor, timestamp, details
             FROM check_in_delegation_audit_log WHERE vault_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![vault_id], |r| {
            let event_kind_str: String = r.get(2)?;
            let ts: String = r.get(5)?;
            let details_str: Option<String> = r.get(6)?;
            let details = details_str.and_then(|s| serde_json::from_str(&s).ok());
            Ok(crate::models::CheckInDelegationAuditEntry {
                id: r.get(0)?,
                vault_id: r.get(1)?,
                event_kind: crate::models::CheckInDelegationEventKind::from_str(&event_kind_str),
                delegate: r.get(3)?,
                actor: r.get(4)?,
                timestamp: parse_rfc3339(&ts).unwrap_or_else(chrono::Utc::now),
                details,
            })
        })?;
        rows.collect()
    }
}

// ── Issue #2070: multi-party withdrawal approval workflow ───────────────────

impl Db {
    /// Persist a new pending withdrawal approval workflow.
    pub fn create_withdrawal_workflow(
        &self,
        w: &crate::models::WithdrawalApprovalWorkflow,
    ) -> Result<(), rusqlite::Error> {
        let approvers_json =
            serde_json::to_string(&w.required_approvers).unwrap_or_else(|_| "[]".to_string());
        self.conn.lock().unwrap().execute(
            r#"INSERT INTO withdrawal_approval_workflows
                   (withdrawal_id, vault_id, amount, requested_by, required_approvers,
                    approval_ttl_seconds, created_at, expires_at, status, executed_tx_hash)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
            params![
                w.withdrawal_id,
                w.vault_id,
                w.amount,
                w.requested_by,
                approvers_json,
                w.approval_ttl_seconds as i64,
                w.created_at.to_rfc3339(),
                w.expires_at.to_rfc3339(),
                w.status.as_str(),
                w.executed_tx_hash,
            ],
        )?;
        Ok(())
    }

    /// Load a withdrawal workflow by id, lazily expiring it if its TTL has
    /// elapsed, and attaching its recorded approvals.
    pub fn get_withdrawal_workflow(
        &self,
        withdrawal_id: &str,
    ) -> Result<Option<crate::models::WithdrawalApprovalWorkflow>, rusqlite::Error> {
        let wf_opt = {
            let conn = self.conn.lock().unwrap();
            let res = conn.query_row(
                r#"SELECT withdrawal_id, vault_id, amount, requested_by, required_approvers,
                          approval_ttl_seconds, created_at, expires_at, status, executed_tx_hash
                   FROM withdrawal_approval_workflows WHERE withdrawal_id = ?1"#,
                params![withdrawal_id],
                |r| {
                    let approvers_json: String = r.get(4)?;
                    let required: Vec<String> = serde_json::from_str(&approvers_json).unwrap_or_default();
                    let created = parse_rfc3339(&r.get::<_, String>(6)?).unwrap_or_else(chrono::Utc::now);
                    let expires = parse_rfc3339(&r.get::<_, String>(7)?).unwrap_or_else(chrono::Utc::now);
                    let status_str: String = r.get(8)?;
                    Ok(crate::models::WithdrawalApprovalWorkflow {
                        withdrawal_id: r.get(0)?,
                        vault_id: r.get(1)?,
                        amount: r.get(2)?,
                        requested_by: r.get(3)?,
                        required_approvers: required,
                        approval_ttl_seconds: r.get::<_, i64>(5)? as u64,
                        created_at: created,
                        expires_at: expires,
                        status: crate::models::WithdrawalApprovalStatus::from_str(&status_str),
                        approvals: Vec::new(),
                        executed_tx_hash: r.get(9)?,
                    })
                },
            );
            match res {
                Ok(w) => Some(w),
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(e) => return Err(e),
            }
        };

        let mut wf = wf_opt.unwrap();

        // Lazy expiry: a pending request whose TTL has elapsed auto-cancels.
        if wf.status == crate::models::WithdrawalApprovalStatus::Pending
            && wf.expires_at < chrono::Utc::now()
        {
            self.set_withdrawal_workflow_status(
                withdrawal_id,
                crate::models::WithdrawalApprovalStatus::Expired,
            )?;
            wf.status = crate::models::WithdrawalApprovalStatus::Expired;
        }

        wf.approvals = self.get_withdrawal_approvals(withdrawal_id)?;
        Ok(Some(wf))
    }

    /// Recorded approvals for a workflow, oldest first.
    pub fn get_withdrawal_approvals(
        &self,
        withdrawal_id: &str,
    ) -> Result<Vec<crate::models::WithdrawalApproval>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT withdrawal_id, approver, approved_at FROM withdrawal_approvals
             WHERE withdrawal_id = ?1 ORDER BY approved_at ASC",
        )?;
        let rows = stmt.query_map(params![withdrawal_id], |r| {
            let ts: String = r.get(2)?;
            Ok(crate::models::WithdrawalApproval {
                withdrawal_id: r.get(0)?,
                approver: r.get(1)?,
                approved_at: parse_rfc3339(&ts).unwrap_or_else(chrono::Utc::now),
            })
        })?;
        rows.collect()
    }

    /// Record an approval. Returns `true` if a new approval row was inserted,
    /// `false` if this approver had already approved (idempotent).
    pub fn add_withdrawal_approval(
        &self,
        withdrawal_id: &str,
        approver: &str,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO withdrawal_approvals (withdrawal_id, approver, approved_at) VALUES (?1, ?2, ?3)",
            params![withdrawal_id, approver, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(conn.changes() > 0)
    }

    /// Transition a workflow's status.
    pub fn set_withdrawal_workflow_status(
        &self,
        withdrawal_id: &str,
        status: crate::models::WithdrawalApprovalStatus,
    ) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "UPDATE withdrawal_approval_workflows SET status = ?2 WHERE withdrawal_id = ?1",
            params![withdrawal_id, status.as_str()],
        )?;
        Ok(())
    }

    /// Mark a workflow as executed, optionally recording the on-chain tx hash.
    pub fn record_withdrawal_execution(
        &self,
        withdrawal_id: &str,
        tx_hash: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "UPDATE withdrawal_approval_workflows SET status = 'executed', executed_tx_hash = ?2 WHERE withdrawal_id = ?1",
            params![withdrawal_id, tx_hash],
        )?;
        Ok(())
    }

    /// All workflows for a vault (lazily expired on read).
    pub fn list_withdrawal_workflows(
        &self,
        vault_id: &str,
    ) -> Result<Vec<crate::models::WithdrawalApprovalWorkflow>, rusqlite::Error> {
        let ids: Vec<String> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT withdrawal_id FROM withdrawal_approval_workflows WHERE vault_id = ?1 ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map(params![vault_id], |r| r.get::<_, String>(0))?;
            rows.filter_map(Result::ok).collect()
        };
        let mut out = Vec::new();
        for id in ids {
            if let Some(wf) = self.get_withdrawal_workflow(&id)? {
                out.push(wf);
            }
        }
        Ok(out)
    }

    /// Cancel every pending workflow whose TTL has elapsed. Returns the number
    /// of workflows transitioned to `expired`. Intended to be called by a
    /// background watcher (see `ttl_watch::WithdrawalApprovalExpiryWatcher`).
    pub fn sweep_expired_withdrawal_workflows(&self) -> Result<u64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE withdrawal_approval_workflows SET status = 'expired' WHERE status = 'pending' AND expires_at < ?1",
            params![chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(conn.changes() as u64)
    }
}
