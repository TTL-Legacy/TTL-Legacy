//! Dead-letter queue for permanently failed reminder dispatches — #2073.
//!
//! When a reminder email exceeds `MAX_RETRY_ATTEMPTS` it is written to this
//! table instead of being dropped silently.  The admin endpoint
//! `GET /admin/dead-letter` reads from here, and an alert fires when the
//! queue depth crosses `DEAD_LETTER_ALERT_THRESHOLD`.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Number of dispatch attempts before a reminder is moved to the dead-letter
/// queue.  The scheduler retries up to this many times before giving up.
pub const MAX_RETRY_ATTEMPTS: u32 = 3;

/// When the dead-letter queue depth reaches this count, an alert is emitted
/// (Slack / webhook).
pub const DEAD_LETTER_ALERT_THRESHOLD: usize = 10;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// A single permanently-failed reminder dispatch entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    /// Auto-assigned row identifier.
    pub id: i64,
    /// The vault whose owner should have been reminded.
    pub vault_id: u64,
    /// Delivery channel that failed (e.g. `"email"`, `"sms"`).
    pub channel: String,
    /// Human-readable error message from the last attempt.
    pub last_error: String,
    /// How many delivery attempts were made before giving up.
    pub attempt_count: u32,
    /// Wall-clock time of the first failed attempt.
    pub first_attempted_at: DateTime<Utc>,
    /// Wall-clock time of the final (exhausted) attempt.
    pub failed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Schema bootstrap
// ---------------------------------------------------------------------------

/// Creates the `dead_letter_reminders` table if it does not already exist.
///
/// Call this during the DB migration phase before any reads or writes.
pub fn create_table(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS dead_letter_reminders (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            vault_id            INTEGER NOT NULL,
            channel             TEXT    NOT NULL,
            last_error          TEXT    NOT NULL DEFAULT '',
            attempt_count       INTEGER NOT NULL DEFAULT 0,
            first_attempted_at  TEXT    NOT NULL,
            failed_at           TEXT    NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_dlr_vault_id  ON dead_letter_reminders(vault_id);
        CREATE INDEX IF NOT EXISTS idx_dlr_failed_at ON dead_letter_reminders(failed_at);
        "#,
    )
}

// ---------------------------------------------------------------------------
// Write path
// ---------------------------------------------------------------------------

/// Inserts a new dead-letter entry.
///
/// Called by the scheduler after all retry attempts are exhausted.
pub fn insert(
    conn: &Connection,
    vault_id: u64,
    channel: &str,
    last_error: &str,
    attempt_count: u32,
    first_attempted_at: DateTime<Utc>,
    failed_at: DateTime<Utc>,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        r#"
        INSERT INTO dead_letter_reminders
            (vault_id, channel, last_error, attempt_count, first_attempted_at, failed_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
        params![
            vault_id as i64,
            channel,
            last_error,
            attempt_count,
            first_attempted_at.to_rfc3339(),
            failed_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Read path
// ---------------------------------------------------------------------------

/// Returns all entries in the dead-letter queue, newest first.
pub fn list_all(conn: &Connection) -> Result<Vec<DeadLetterEntry>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, vault_id, channel, last_error, attempt_count,
               first_attempted_at, failed_at
        FROM   dead_letter_reminders
        ORDER  BY failed_at DESC
        "#,
    )?;

    let rows = stmt.query_map([], |row| {
        let first_attempted_at_str: String = row.get(5)?;
        let failed_at_str: String = row.get(6)?;

        let first_attempted_at = first_attempted_at_str
            .parse::<DateTime<Utc>>()
            .unwrap_or(Utc::now());
        let failed_at = failed_at_str
            .parse::<DateTime<Utc>>()
            .unwrap_or(Utc::now());

        Ok(DeadLetterEntry {
            id: row.get(0)?,
            vault_id: row.get::<_, i64>(1)? as u64,
            channel: row.get(2)?,
            last_error: row.get(3)?,
            attempt_count: row.get(4)?,
            first_attempted_at,
            failed_at,
        })
    })?;

    rows.collect()
}

/// Returns the current depth (row count) of the dead-letter queue.
pub fn queue_depth(conn: &Connection) -> Result<usize, rusqlite::Error> {
    conn.query_row(
        "SELECT COUNT(*) FROM dead_letter_reminders",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n as usize)
}

// ---------------------------------------------------------------------------
// Alert helper
// ---------------------------------------------------------------------------

/// Emits a structured log warning when the queue depth exceeds
/// [`DEAD_LETTER_ALERT_THRESHOLD`].
///
/// In production this log line is forwarded to Slack / PagerDuty via the
/// existing log-aggregation pipeline.  Replace the body of this function with
/// a real HTTP call to `SLACK_WEBHOOK_URL` once that env var is wired.
pub fn maybe_alert(depth: usize) {
    if depth >= DEAD_LETTER_ALERT_THRESHOLD {
        tracing::warn!(
            dead_letter_depth = depth,
            threshold = DEAD_LETTER_ALERT_THRESHOLD,
            "dead-letter queue depth exceeds threshold — manual intervention required"
        );
    }
}

// ---------------------------------------------------------------------------
// Scheduler integration helper
// ---------------------------------------------------------------------------

/// Records a permanently-failed dispatch and fires an alert if warranted.
///
/// Call this from `scheduler.rs` after `MAX_RETRY_ATTEMPTS` are exhausted:
///
/// ```rust,ignore
/// dead_letter::record_failure(
///     &conn,
///     vault_id,
///     "email",
///     &error_message,
///     MAX_RETRY_ATTEMPTS,
///     first_attempted_at,
/// );
/// ```
pub fn record_failure(
    conn: &Connection,
    vault_id: u64,
    channel: &str,
    last_error: &str,
    attempt_count: u32,
    first_attempted_at: DateTime<Utc>,
) -> Result<(), rusqlite::Error> {
    let failed_at = Utc::now();
    insert(
        conn,
        vault_id,
        channel,
        last_error,
        attempt_count,
        first_attempted_at,
        failed_at,
    )?;

    // Fire an alert if the queue has grown too large.
    if let Ok(depth) = queue_depth(conn) {
        maybe_alert(depth);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        create_table(&conn).unwrap();
        conn
    }

    #[test]
    fn test_insert_and_list() {
        let conn = open_test_db();
        let now = Utc::now();

        insert(&conn, 42, "email", "smtp timeout", 3, now, now).unwrap();

        let entries = list_all(&conn).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].vault_id, 42);
        assert_eq!(entries[0].channel, "email");
        assert_eq!(entries[0].attempt_count, 3);
    }

    #[test]
    fn test_queue_depth() {
        let conn = open_test_db();
        let now = Utc::now();

        assert_eq!(queue_depth(&conn).unwrap(), 0);
        insert(&conn, 1, "email", "err", 3, now, now).unwrap();
        insert(&conn, 2, "sms", "err", 3, now, now).unwrap();
        assert_eq!(queue_depth(&conn).unwrap(), 2);
    }

    #[test]
    fn test_record_failure_roundtrip() {
        let conn = open_test_db();
        let first = Utc::now();

        record_failure(&conn, 99, "sms", "twilio 503", MAX_RETRY_ATTEMPTS, first).unwrap();

        let entries = list_all(&conn).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].vault_id, 99);
        assert_eq!(entries[0].attempt_count, MAX_RETRY_ATTEMPTS);
    }

    #[test]
    fn test_maybe_alert_does_not_panic_below_threshold() {
        // Should not panic; just logs nothing.
        maybe_alert(DEAD_LETTER_ALERT_THRESHOLD - 1);
    }

    #[test]
    fn test_maybe_alert_fires_at_threshold() {
        // Should not panic; emits a tracing::warn.
        maybe_alert(DEAD_LETTER_ALERT_THRESHOLD);
        maybe_alert(DEAD_LETTER_ALERT_THRESHOLD + 5);
    }

    #[test]
    fn test_max_retry_attempts_constant() {
        assert_eq!(MAX_RETRY_ATTEMPTS, 3);
    }

    #[test]
    fn test_alert_threshold_constant() {
        assert_eq!(DEAD_LETTER_ALERT_THRESHOLD, 10);
    }
}
