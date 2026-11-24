-- Issue #2068: persist check-in delegation grant/revoke events.
--
-- `check_in_delegations` holds the current delegate per vault (the source of
-- truth the TTL watcher diffs against its last-seen state); the append-only
-- `check_in_delegation_audit_log` is populated by the watcher on every
-- observed grant/revoke transition and exposed via
-- `GET /api/vaults/{vault_id}/delegation-audit`.

CREATE TABLE IF NOT EXISTS check_in_delegations (
    vault_id    TEXT PRIMARY KEY,
    owner       TEXT NOT NULL,
    delegate    TEXT,            -- address authorised to check in on the owner's behalf
    granted_at  TEXT,            -- RFC3339 timestamp of the current grant (NULL after revoke)
    updated_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_check_in_delegations_delegate ON check_in_delegations(delegate);

CREATE TABLE IF NOT EXISTS check_in_delegation_audit_log (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    vault_id   TEXT NOT NULL,
    event_kind TEXT NOT NULL,    -- 'grant' | 'revoke'
    delegate   TEXT,             -- address delegated to (NULL on revoke)
    actor      TEXT NOT NULL DEFAULT '',
    timestamp  TEXT NOT NULL,
    details    TEXT
);
CREATE INDEX IF NOT EXISTS idx_check_in_delegation_audit_vault_id  ON check_in_delegation_audit_log(vault_id);
CREATE INDEX IF NOT EXISTS idx_check_in_delegation_audit_timestamp ON check_in_delegation_audit_log(timestamp);
