-- Previously shipped under a duplicate "5" version key in the old ad-hoc
-- Rust-array migration runner (Db::migrate), which meant this table was
-- silently never created once the first "5" entry had already been applied
-- and recorded in schema_migrations — Issue #1176 fixes exactly this class
-- of bug by giving every migration a unique, ordered file.
CREATE TABLE IF NOT EXISTS vesting_bonus_config (
    vault_id              TEXT PRIMARY KEY,
    bonus_bps             INTEGER NOT NULL,
    on_time_window_seconds INTEGER NOT NULL
);
