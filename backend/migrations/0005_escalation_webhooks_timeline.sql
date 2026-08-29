CREATE TABLE IF NOT EXISTS escalation_states (
    vault_id                INTEGER PRIMARY KEY,
    last_escalation_tier    TEXT,
    escalated_at            TEXT
);
CREATE TABLE IF NOT EXISTS escalation_events (
    id              TEXT PRIMARY KEY,
    vault_id        INTEGER NOT NULL,
    tier            TEXT NOT NULL,
    dispatched_at   TEXT NOT NULL,
    channels        TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS idx_escalation_events_vault_id ON escalation_events(vault_id);

CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id              TEXT PRIMARY KEY,
    vault_id        TEXT NOT NULL,
    event_type      TEXT NOT NULL,
    payload         TEXT NOT NULL DEFAULT '{}',
    endpoint_url    TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',
    attempt_count   INTEGER NOT NULL DEFAULT 0,
    next_retry_at   TEXT,
    created_at      TEXT NOT NULL,
    attempts        TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_vault_id ON webhook_deliveries(vault_id);
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_status   ON webhook_deliveries(status);

CREATE TABLE IF NOT EXISTS webhook_subscriptions (
    id              TEXT PRIMARY KEY,
    vault_id        TEXT NOT NULL,
    endpoint_url    TEXT NOT NULL,
    event_types     TEXT NOT NULL DEFAULT '[]',
    created_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_webhook_subscriptions_vault_id ON webhook_subscriptions(vault_id);

CREATE TABLE IF NOT EXISTS vault_timeline_events (
    id          TEXT PRIMARY KEY,
    vault_id    TEXT NOT NULL,
    kind        TEXT NOT NULL,
    timestamp   TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    amount      INTEGER,
    metadata    TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_vault_timeline_events_vault_id  ON vault_timeline_events(vault_id);
CREATE INDEX IF NOT EXISTS idx_vault_timeline_events_kind      ON vault_timeline_events(kind);

CREATE TABLE IF NOT EXISTS vault_subscriptions (
    vault_id  INTEGER PRIMARY KEY,
    owner     TEXT NOT NULL,
    channels  TEXT NOT NULL,
    frequency TEXT NOT NULL
);
