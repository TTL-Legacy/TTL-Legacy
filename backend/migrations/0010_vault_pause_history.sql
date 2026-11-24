-- Issue #2053: index vault pause/resume events so they can be served by
-- GET /vaults/{id}/pause-history without round-tripping the Soroban RPC.
CREATE TABLE IF NOT EXISTS vault_pause_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    vault_id    TEXT    NOT NULL,
    actor       TEXT    NOT NULL,
    -- "pause" or "resume"
    event_type  TEXT    NOT NULL,
    reason      TEXT    NOT NULL DEFAULT '',
    timestamp   TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_vault_pause_history_vault_id
    ON vault_pause_history (vault_id);

CREATE INDEX IF NOT EXISTS idx_vault_pause_history_timestamp
    ON vault_pause_history (timestamp);
