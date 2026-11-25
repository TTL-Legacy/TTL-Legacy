-- Issue #2059: vault config export/import.
--
-- vault_config_exports records every export event for auditing.

CREATE TABLE IF NOT EXISTS vault_config_exports (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    vault_id    TEXT NOT NULL,
    owner       TEXT NOT NULL,
    exported_at TEXT NOT NULL,
    signature   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_vault_config_exports_vault_id
    ON vault_config_exports (vault_id);
