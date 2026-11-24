-- Issue #2069: full-text search over vault tags and notes.
--
-- The application keeps live vault records in-memory; owner-supplied tags and
-- notes are persisted here so they can be indexed for full-text search. The
-- codebase runs on SQLite (sqlx + rusqlite), so this uses SQLite FTS5 — the
-- SQLite equivalent of PostgreSQL tsvector/tsquery — rather than PostgreSQL
-- full-text search. (A future move to PostgreSQL would replace the FTS5
-- virtual table with a generated `tsvector` column + GIN index.)

CREATE TABLE IF NOT EXISTS vault_metadata (
    vault_id   TEXT PRIMARY KEY,
    owner      TEXT NOT NULL,
    tags       TEXT NOT NULL DEFAULT '[]',  -- JSON array of tag strings
    notes      TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_vault_metadata_owner ON vault_metadata(owner);

-- FTS5 index over the searchable text. `vault_id`/`owner` are UNINDEXED (stored
-- but not tokenised); `tags` and `notes` are tokenised for MATCH queries. The
-- application keeps this table in sync with vault_metadata via DELETE + INSERT
-- on every metadata upsert.
CREATE VIRTUAL TABLE IF NOT EXISTS vault_search_fts USING fts5(
    vault_id UNINDEXED,
    owner    UNINDEXED,
    tags,
    notes,
    tokenize = 'unicode61'
);
