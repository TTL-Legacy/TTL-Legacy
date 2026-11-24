-- Issue #2056: per-vault withdrawal rate-limit configuration.
--
-- withdrawal_rate_limit_configs stores the cap and window per vault.
-- withdrawal_rate_limit_records is the rolling ledger used to sum amounts
-- within any given window; the enforcement code deletes rows older than
-- window_seconds before computing the sum.

CREATE TABLE IF NOT EXISTS withdrawal_rate_limit_configs (
    vault_id              TEXT    PRIMARY KEY,
    max_amount_per_window INTEGER NOT NULL,
    window_seconds        INTEGER NOT NULL,
    created_at            TEXT    NOT NULL,
    updated_at            TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS withdrawal_rate_limit_records (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    vault_id    TEXT    NOT NULL,
    amount      INTEGER NOT NULL,
    recorded_at TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_wrl_records_vault_id_recorded_at
    ON withdrawal_rate_limit_records (vault_id, recorded_at);
