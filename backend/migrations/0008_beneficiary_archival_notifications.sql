-- Migration 0008: beneficiary archival notification tables (Issue #1337)
--
-- Stores beneficiary contact info (email/phone) and a log of dispatched
-- TTL-expiry notifications so beneficiaries know when to claim funds.

CREATE TABLE IF NOT EXISTS beneficiary_contacts (
    vault_id              TEXT    NOT NULL,
    beneficiary_address   TEXT    NOT NULL,
    email                 TEXT,
    phone                 TEXT,
    opted_in              INTEGER NOT NULL DEFAULT 1,  -- 1 = opted in, 0 = opted out
    updated_at            TEXT    NOT NULL,
    PRIMARY KEY (vault_id, beneficiary_address)
);

CREATE INDEX IF NOT EXISTS idx_beneficiary_contacts_vault_id
    ON beneficiary_contacts(vault_id);

CREATE INDEX IF NOT EXISTS idx_beneficiary_contacts_beneficiary
    ON beneficiary_contacts(beneficiary_address);

CREATE TABLE IF NOT EXISTS beneficiary_archival_notifications (
    id                    TEXT    NOT NULL PRIMARY KEY,
    vault_id              TEXT    NOT NULL,
    beneficiary_address   TEXT    NOT NULL,
    channel               TEXT    NOT NULL,  -- "email" | "sms"
    dispatched_at         TEXT    NOT NULL,
    status                TEXT    NOT NULL DEFAULT 'pending',  -- "pending" | "sent" | "failed"
    error                 TEXT
);

CREATE INDEX IF NOT EXISTS idx_ben_archival_notif_vault_id
    ON beneficiary_archival_notifications(vault_id);

CREATE INDEX IF NOT EXISTS idx_ben_archival_notif_beneficiary
    ON beneficiary_archival_notifications(beneficiary_address);
