-- Issue #2070: multi-party withdrawal approval workflow for high-value vaults.
--
-- A pending workflow auto-cancels (status -> 'expired') once `expires_at`
-- passes (the request's `withdrawal_approval_ttl_seconds`). `required_approvers`
-- is a JSON array of addresses (a subset of beneficiaries or designated
-- signers) whose approval is required; the workflow becomes 'approved' once
-- every one of them has recorded an approval row in `withdrawal_approvals`.

CREATE TABLE IF NOT EXISTS withdrawal_approval_workflows (
    withdrawal_id          TEXT PRIMARY KEY,
    vault_id               TEXT NOT NULL,
    amount                 INTEGER NOT NULL,
    requested_by           TEXT NOT NULL,
    required_approvers     TEXT NOT NULL DEFAULT '[]',  -- JSON array of addresses
    approval_ttl_seconds   INTEGER NOT NULL,
    created_at              TEXT NOT NULL,
    expires_at              TEXT NOT NULL,               -- RFC3339; lexicographic compare == chronological (UTC)
    status                  TEXT NOT NULL DEFAULT 'pending',  -- pending|approved|cancelled|expired|executed
    executed_tx_hash        TEXT
);
CREATE INDEX IF NOT EXISTS idx_withdrawal_approval_workflows_vault_id ON withdrawal_approval_workflows(vault_id);
CREATE INDEX IF NOT EXISTS idx_withdrawal_approval_workflows_status   ON withdrawal_approval_workflows(status);
CREATE INDEX IF NOT EXISTS idx_withdrawal_approval_workflows_expires  ON withdrawal_approval_workflows(expires_at);

CREATE TABLE IF NOT EXISTS withdrawal_approvals (
    withdrawal_id  TEXT NOT NULL,
    approver       TEXT NOT NULL,
    approved_at    TEXT NOT NULL,
    PRIMARY KEY (withdrawal_id, approver)
);
CREATE INDEX IF NOT EXISTS idx_withdrawal_approvals_approver ON withdrawal_approvals(approver);
