//! Disaster Recovery (DR) status tracking and admin-only endpoint.
//!
//! Exposes `GET /admin/dr-status` returning:
//! - the timestamp of the last successful backup,
//! - the last restore verification result, and
//! - any active DR alerts (including automatically derived alerts for missing
//!   or stale backups and failed restore verifications).

use std::sync::{Arc, RwLock};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::audit::{authorize_admin, ApiError};

/// Default maximum age (in hours) before a missing or stale backup triggers a DR alert.
pub const DEFAULT_BACKUP_MAX_AGE_HOURS: i64 = 24;

/// Outcome of the most recent DR restore verification drill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RestoreVerificationStatus {
    Passed,
    Failed,
    Unknown,
}

/// Summary of the most recent restore verification run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreVerificationResult {
    pub status: RestoreVerificationStatus,
    pub verified_at: Option<DateTime<Utc>>,
    pub backup_prefix: Option<String>,
    pub details: Option<String>,
}

impl Default for RestoreVerificationResult {
    fn default() -> Self {
        Self {
            status: RestoreVerificationStatus::Unknown,
            verified_at: None,
            backup_prefix: None,
            details: None,
        }
    }
}

/// Severity level for an active DR alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DrAlertSeverity {
    Warning,
    Critical,
}

/// An active disaster-recovery alert requiring operator attention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrAlert {
    pub id: String,
    pub severity: DrAlertSeverity,
    pub message: String,
    pub triggered_at: DateTime<Utc>,
}

/// Response payload returned by `GET /admin/dr-status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrStatusResponse {
    pub last_successful_backup_at: Option<DateTime<Utc>>,
    pub last_restore_verification: RestoreVerificationResult,
    pub active_alerts: Vec<DrAlert>,
    pub checked_at: DateTime<Utc>,
}

/// Internal mutable state persisted in memory (or hydrated from `DR_STATUS_FILE`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DrStatusState {
    pub last_successful_backup_at: Option<DateTime<Utc>>,
    pub last_restore_verification: RestoreVerificationResult,
    pub active_alerts: Vec<DrAlert>,
}

/// Thread-safe store backing the `GET /admin/dr-status` endpoint.
#[derive(Debug, Default)]
pub struct DrStatusStore {
    state: RwLock<DrStatusState>,
    backup_max_age_hours: i64,
}

impl DrStatusStore {
    /// Creates an empty store using the default 24-hour backup freshness threshold.
    pub fn new() -> Self {
        Self::with_max_age_hours(DEFAULT_BACKUP_MAX_AGE_HOURS)
    }

    /// Creates an empty store with a custom backup freshness threshold (in hours).
    pub fn with_max_age_hours(backup_max_age_hours: i64) -> Self {
        Self {
            state: RwLock::new(DrStatusState::default()),
            backup_max_age_hours: backup_max_age_hours.max(1),
        }
    }

    /// Creates a store hydrated from `DR_STATUS_FILE` (if set and readable) and
    /// `DR_BACKUP_MAX_AGE_HOURS` (if set and valid).
    pub fn from_env() -> Self {
        let max_age_hours = std::env::var("DR_BACKUP_MAX_AGE_HOURS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(DEFAULT_BACKUP_MAX_AGE_HOURS);

        let initial_state = std::env::var("DR_STATUS_FILE")
            .ok()
            .filter(|p| !p.trim().is_empty())
            .and_then(|path| match std::fs::read_to_string(&path) {
                Ok(raw) => match serde_json::from_str::<DrStatusState>(&raw) {
                    Ok(parsed) => Some(parsed),
                    Err(e) => {
                        tracing::warn!(path = %path, error = %e, "failed to parse DR_STATUS_FILE");
                        None
                    }
                },
                Err(e) => {
                    tracing::warn!(path = %path, error = %e, "failed to read DR_STATUS_FILE");
                    None
                }
            })
            .unwrap_or_default();

        Self {
            state: RwLock::new(initial_state),
            backup_max_age_hours: max_age_hours,
        }
    }

    /// Records the timestamp of a completed backup.
    pub fn record_successful_backup(&self, timestamp: DateTime<Utc>) {
        if let Ok(mut guard) = self.state.write() {
            guard.last_successful_backup_at = Some(timestamp);
        }
    }

    /// Records the outcome of a restore verification run.
    pub fn record_restore_verification(&self, result: RestoreVerificationResult) {
        if let Ok(mut guard) = self.state.write() {
            guard.last_restore_verification = result;
        }
    }

    /// Adds or replaces an active DR alert by `id`.
    pub fn upsert_alert(&self, alert: DrAlert) {
        if let Ok(mut guard) = self.state.write() {
            if let Some(existing) = guard.active_alerts.iter_mut().find(|a| a.id == alert.id) {
                *existing = alert;
            } else {
                guard.active_alerts.push(alert);
            }
        }
    }

    /// Removes an active DR alert by `id`.
    pub fn clear_alert(&self, alert_id: &str) {
        if let Ok(mut guard) = self.state.write() {
            guard.active_alerts.retain(|a| a.id != alert_id);
        }
    }

    /// Computes the current DR status snapshot, including any derived alerts.
    pub fn snapshot(&self, now: DateTime<Utc>) -> DrStatusResponse {
        let state = self
            .state
            .read()
            .map(|g| g.clone())
            .unwrap_or_default();

        let mut alerts = state.active_alerts;

        match state.last_successful_backup_at {
            None => {
                if !alerts.iter().any(|a| a.id == "backup-missing") {
                    alerts.push(DrAlert {
                        id: "backup-missing".to_string(),
                        severity: DrAlertSeverity::Critical,
                        message: "No successful backup has been recorded yet.".to_string(),
                        triggered_at: now,
                    });
                }
            }
            Some(ts) if now - ts > Duration::hours(self.backup_max_age_hours) => {
                if !alerts.iter().any(|a| a.id == "backup-stale") {
                    alerts.push(DrAlert {
                        id: "backup-stale".to_string(),
                        severity: DrAlertSeverity::Warning,
                        message: format!(
                            "Last successful backup is older than {} hours.",
                            self.backup_max_age_hours
                        ),
                        triggered_at: now,
                    });
                }
            }
            Some(_) => {}
        }

        if state.last_restore_verification.status == RestoreVerificationStatus::Failed
            && !alerts.iter().any(|a| a.id == "restore-verification-failed")
        {
            alerts.push(DrAlert {
                id: "restore-verification-failed".to_string(),
                severity: DrAlertSeverity::Critical,
                message: state
                    .last_restore_verification
                    .details
                    .clone()
                    .unwrap_or_else(|| "Latest restore verification failed.".to_string()),
                triggered_at: state
                    .last_restore_verification
                    .verified_at
                    .unwrap_or(now),
            });
        }

        DrStatusResponse {
            last_successful_backup_at: state.last_successful_backup_at,
            last_restore_verification: state.last_restore_verification,
            active_alerts: alerts,
            checked_at: now,
        }
    }
}

/// Checks admin authorization against an explicit expected API key.
///
/// When `expected_key` is `Some(key)` and non-empty, the request must carry
/// `Authorization: Bearer <key>`. Otherwise returns `401 Unauthorized`.
pub fn authorize_admin_key(
    headers: &HeaderMap,
    expected_key: Option<&str>,
) -> Result<(), ApiError> {
    let Some(expected) = expected_key.filter(|k| !k.is_empty()) else {
        return Ok(());
    };

    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    match auth_header.strip_prefix("Bearer ") {
        Some(token) if token == expected => Ok(()),
        _ => Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "valid admin API key required",
        )),
    }
}

/// `GET /admin/dr-status`
///
/// Admin-only endpoint returning the timestamp of the last successful backup,
/// the last restore verification result, and any active DR alerts.
pub async fn get_dr_status(
    State(store): State<Arc<DrStatusStore>>,
    headers: HeaderMap,
) -> Result<Json<DrStatusResponse>, ApiError> {
    authorize_admin(&headers)?;
    Ok(Json(store.snapshot(Utc::now())))
}

/// Builds an Axum router mounting `GET /admin/dr-status`.
pub fn dr_status_router(store: Arc<DrStatusStore>) -> Router {
    Router::new()
        .route("/admin/dr-status", get(get_dr_status))
        .with_state(store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{header::AUTHORIZATION, HeaderValue};

    #[test]
    fn snapshot_reports_missing_backup_alert_when_empty() {
        let store = DrStatusStore::new();
        let now = Utc::now();
        let status = store.snapshot(now);

        assert!(status.last_successful_backup_at.is_none());
        assert_eq!(
            status.last_restore_verification.status,
            RestoreVerificationStatus::Unknown
        );
        assert_eq!(status.active_alerts.len(), 1);
        assert_eq!(status.active_alerts[0].id, "backup-missing");
        assert_eq!(status.active_alerts[0].severity, DrAlertSeverity::Critical);
    }

    #[test]
    fn snapshot_is_healthy_with_recent_backup_and_passed_verification() {
        let store = DrStatusStore::new();
        let now = Utc::now();
        let backup_ts = now - Duration::hours(2);

        store.record_successful_backup(backup_ts);
        store.record_restore_verification(RestoreVerificationResult {
            status: RestoreVerificationStatus::Passed,
            verified_at: Some(now - Duration::hours(1)),
            backup_prefix: Some("backups/20261123T210000Z".to_string()),
            details: Some("All checksums and GPG signatures verified.".to_string()),
        });

        let status = store.snapshot(now);
        assert_eq!(status.last_successful_backup_at, Some(backup_ts));
        assert_eq!(
            status.last_restore_verification.status,
            RestoreVerificationStatus::Passed
        );
        assert!(status.active_alerts.is_empty());
    }

    #[test]
    fn snapshot_raises_stale_backup_and_failed_restore_alerts() {
        let store = DrStatusStore::with_max_age_hours(24);
        let now = Utc::now();

        store.record_successful_backup(now - Duration::hours(30));
        store.record_restore_verification(RestoreVerificationResult {
            status: RestoreVerificationStatus::Failed,
            verified_at: Some(now - Duration::hours(1)),
            backup_prefix: Some("backups/20261122T120000Z".to_string()),
            details: Some("GPG signature verification failed.".to_string()),
        });

        let status = store.snapshot(now);
        let ids: Vec<&str> = status.active_alerts.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"backup-stale"));
        assert!(ids.contains(&"restore-verification-failed"));
    }

    #[test]
    fn upsert_and_clear_custom_alert() {
        let store = DrStatusStore::new();
        let now = Utc::now();
        store.record_successful_backup(now - Duration::minutes(10));

        store.upsert_alert(DrAlert {
            id: "replica-lag".to_string(),
            severity: DrAlertSeverity::Warning,
            message: "Cross-region replica is 15 minutes behind.".to_string(),
            triggered_at: now,
        });
        assert_eq!(store.snapshot(now).active_alerts.len(), 1);

        store.clear_alert("replica-lag");
        assert!(store.snapshot(now).active_alerts.is_empty());
    }

    #[test]
    fn authorize_admin_key_rejects_missing_or_wrong_token() {
        let empty_headers = HeaderMap::new();
        assert!(authorize_admin_key(&empty_headers, Some("secret-key")).is_err());

        let mut wrong_headers = HeaderMap::new();
        wrong_headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer wrong"));
        assert!(authorize_admin_key(&wrong_headers, Some("secret-key")).is_err());

        let mut valid_headers = HeaderMap::new();
        valid_headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret-key"));
        assert!(authorize_admin_key(&valid_headers, Some("secret-key")).is_ok());
    }
}
