-- Issue #1177: refresh-token rotation. Each row is one issued refresh
-- token; `family_id` groups every token descended from the same original
-- login so that presenting an already-rotated (revoked) token can revoke
-- the whole family as a stolen-token countermeasure.
CREATE TABLE IF NOT EXISTS refresh_tokens (
    jti         TEXT PRIMARY KEY,
    family_id   TEXT NOT NULL,
    sub         TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    expires_at  TEXT NOT NULL,
    revoked_at  TEXT
);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_family_id ON refresh_tokens(family_id);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_sub        ON refresh_tokens(sub);
