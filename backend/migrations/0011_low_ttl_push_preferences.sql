-- Issue #2058: per-vault mobile push opt-in preferences for the low-TTL
-- warning (fires when TTL < 48 h and no check-in in the last 24 h).
--
-- low_ttl_push_sent_records tracks when the notification was last fired so
-- we can suppress duplicate alerts within a cooldown window.

CREATE TABLE IF NOT EXISTS low_ttl_push_preferences (
    vault_id        TEXT    NOT NULL,
    owner           TEXT    NOT NULL,
    android_enabled INTEGER NOT NULL DEFAULT 0,
    ios_enabled     INTEGER NOT NULL DEFAULT 0,
    updated_at      TEXT    NOT NULL,
    PRIMARY KEY (vault_id, owner)
);

CREATE TABLE IF NOT EXISTS low_ttl_push_sent_records (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    vault_id    TEXT    NOT NULL,
    owner       TEXT    NOT NULL,
    sent_at     TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_low_ttl_push_sent_vault_id
    ON low_ttl_push_sent_records (vault_id);
