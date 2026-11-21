CREATE TABLE IF NOT EXISTS reminder_preferences (
    vault_id             INTEGER PRIMARY KEY,
    channels             TEXT NOT NULL,
    hours_before_expiry  INTEGER NOT NULL,
    frequency            TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ttl_insurance_policies (
    vault_id                      INTEGER PRIMARY KEY,
    extension_seconds             INTEGER NOT NULL,
    inactivity_threshold_seconds  INTEGER NOT NULL,
    enabled                        INTEGER NOT NULL,
    purchased_at                   TEXT NOT NULL,
    last_extended_at               TEXT
);

CREATE TABLE IF NOT EXISTS owner_activity (
    owner_id       INTEGER PRIMARY KEY,
    last_active_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS idempotency_keys (
    key           TEXT PRIMARY KEY,
    status_code   INTEGER NOT NULL,
    response_body TEXT NOT NULL,
    created_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS unsubscribe_tokens (
    token      TEXT PRIMARY KEY,
    owner      TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS unsubscribed_users (
    owner TEXT PRIMARY KEY
);
