CREATE TABLE IF NOT EXISTS sponsored_releases (
    tx_id             TEXT PRIMARY KEY,
    vault_id          TEXT NOT NULL,
    beneficiary       TEXT NOT NULL,
    released_amount   INTEGER NOT NULL,
    protocol_fee      INTEGER NOT NULL,
    net_amount        INTEGER NOT NULL,
    fee_bump_tx_hash  TEXT NOT NULL,
    sponsor_account   TEXT NOT NULL,
    sponsorship_fee   INTEGER NOT NULL,
    status            TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    executed_at       TEXT,
    ledger_sequence   INTEGER,
    error             TEXT
);
CREATE INDEX IF NOT EXISTS idx_sponsored_releases_vault_id ON sponsored_releases(vault_id);
CREATE INDEX IF NOT EXISTS idx_sponsored_releases_beneficiary ON sponsored_releases(beneficiary);
CREATE INDEX IF NOT EXISTS idx_sponsored_releases_status ON sponsored_releases(status);
CREATE INDEX IF NOT EXISTS idx_sponsored_releases_created_at ON sponsored_releases(created_at);
