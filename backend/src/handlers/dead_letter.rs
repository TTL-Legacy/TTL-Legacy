//! Admin handler for the dead-letter queue endpoint — #2073.
//!
//! `GET /admin/dead-letter` returns every permanently-failed reminder dispatch
//! so an operator can review and retry manually.

use crate::dead_letter;
use crate::db::Db;
use serde::Serialize;
use std::sync::Arc;

/// Response body for `GET /admin/dead-letter`.
#[derive(Debug, Serialize)]
pub struct DeadLetterResponse {
    /// Current number of entries in the queue.
    pub depth: usize,
    /// Whether the depth is at or above the alert threshold.
    pub alert: bool,
    /// All entries, newest first.
    pub entries: Vec<dead_letter::DeadLetterEntry>,
}

/// Handles `GET /admin/dead-letter`.
///
/// Returns the full dead-letter queue so operators can triage permanently
/// failed reminder dispatches.  The response includes a convenience `alert`
/// flag that mirrors the scheduler-side threshold check.
pub fn get_dead_letter_queue(db: &Arc<Db>) -> Result<DeadLetterResponse, String> {
    let conn = db
        .conn
        .lock()
        .map_err(|e| format!("db lock poisoned: {e}"))?;

    let entries = dead_letter::list_all(&conn).map_err(|e| e.to_string())?;
    let depth = entries.len();
    let alert = depth >= dead_letter::DEAD_LETTER_ALERT_THRESHOLD;

    Ok(DeadLetterResponse {
        depth,
        alert,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn open_test_db() -> Arc<Db> {
        let db = Db::open(":memory:").expect("in-memory db");
        {
            let conn = db.conn.lock().unwrap();
            dead_letter::create_table(&conn).unwrap();
        }
        Arc::new(db)
    }

    #[test]
    fn test_empty_queue_response() {
        let db = open_test_db();
        let resp = get_dead_letter_queue(&db).unwrap();
        assert_eq!(resp.depth, 0);
        assert!(!resp.alert);
        assert!(resp.entries.is_empty());
    }

    #[test]
    fn test_queue_with_entries() {
        let db = open_test_db();
        let now = chrono::Utc::now();
        {
            let conn = db.conn.lock().unwrap();
            dead_letter::insert(&conn, 1, "email", "smtp timeout", 3, now, now).unwrap();
            dead_letter::insert(&conn, 2, "sms", "twilio 503", 3, now, now).unwrap();
        }
        let resp = get_dead_letter_queue(&db).unwrap();
        assert_eq!(resp.depth, 2);
        assert!(!resp.alert);
    }

    #[test]
    fn test_alert_flag_set_at_threshold() {
        let db = open_test_db();
        let now = chrono::Utc::now();
        {
            let conn = db.conn.lock().unwrap();
            for i in 0..dead_letter::DEAD_LETTER_ALERT_THRESHOLD {
                dead_letter::insert(&conn, i as u64, "email", "err", 3, now, now).unwrap();
            }
        }
        let resp = get_dead_letter_queue(&db).unwrap();
        assert!(resp.alert);
    }
}
