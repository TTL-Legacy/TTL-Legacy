# Disaster Recovery Runbook

This runbook covers emergency procedures for TTL-Legacy operators. Follow each section in order during an incident.

---

## Severity Levels

| Level | Description | Response Time |
|-------|-------------|---------------|
| P0 — Critical | Active fund loss or contract exploit | Immediate |
| P1 — High | Contract frozen, funds inaccessible | < 1 hour |
| P2 — Medium | Degraded functionality, no fund risk | < 4 hours |
| P3 — Low | Minor issues, monitoring alerts | < 24 hours |

---

## 1. Emergency Contract Pause

Use when an exploit or critical bug is detected.

**Who can act**: Admin key holder only.

```bash
# Pause the contract immediately
stellar contract invoke \
  --id $CONTRACT_TTL_VAULT \
  --network $STELLAR_NETWORK \
  --source $DEPLOYER_IDENTITY \
  -- pause
```

**Verify pause is active:**

```bash
stellar contract invoke \
  --id $CONTRACT_TTL_VAULT \
  --network $STELLAR_NETWORK \
  -- is_paused
# Expected: true
```

**Effects of pause**: `deposit`, `withdraw`, `check_in`, and `trigger_release` all revert. Vault state is preserved. Funds are not moved.

**To unpause** (only after root cause is resolved):

```bash
stellar contract invoke \
  --id $CONTRACT_TTL_VAULT \
  --network $STELLAR_NETWORK \
  --source $DEPLOYER_IDENTITY \
  -- unpause
```

---

## 2. Emergency Withdrawal

Use when a specific vault's funds must be recovered by the owner before a release is triggered.

**Who can act**: Vault owner only (requires owner auth).

```bash
stellar contract invoke \
  --id $CONTRACT_TTL_VAULT \
  --network $STELLAR_NETWORK \
  --source <owner-identity> \
  -- withdraw \
  --vault_id <VAULT_ID> \
  --to <owner-address> \
  --amount <amount-in-stroops>
```

> The contract must **not** be paused for this to succeed. If the contract is paused, unpause first (admin action), perform the withdrawal, then re-pause if needed.

---

## 3. Archived Vault Recovery

Soroban archives persistent entries when their TTL expires. If a vault is archived before `trigger_release` is called, use the following procedure.

**Step 1 — Detect archival:**

```bash
stellar contract invoke \
  --id $CONTRACT_TTL_VAULT \
  --network $STELLAR_NETWORK \
  -- get_archived_vault_info \
  --vault_id <VAULT_ID>
# Returns Some(ArchivedVaultInfo) if archived, None if live
```

**Step 2 — Restore the vault:**

```bash
stellar contract invoke \
  --id $CONTRACT_TTL_VAULT \
  --network $STELLAR_NETWORK \
  --source <any-identity> \
  -- restore_vault \
  --vault_id <VAULT_ID>
```

`restore_vault` re-extends the persistent entry TTL so the vault becomes accessible again. `trigger_release` automatically attempts restoration before transferring funds.

**Step 3 — Verify restoration:**

```bash
stellar contract invoke \
  --id $CONTRACT_TTL_VAULT \
  --network $STELLAR_NETWORK \
  -- get_vault \
  --vault_id <VAULT_ID>
# Should return vault data without error
```

---

## 4. Data Recovery

If contract state is suspected to be corrupted or inconsistent:

1. **Do not unpause** until the state is verified.
2. Query all affected vaults using `get_vault` and compare against off-chain records.
3. Use `get_release_status` to confirm vault statuses.
4. If a vault shows `Released` but funds were not transferred, check the Stellar transaction history for the contract address.
5. Contact the Stellar Development Foundation support if ledger-level data recovery is needed.

---

## 5. Admin Key Rotation

If the admin key is compromised:

```bash
# Step 1: Propose new admin (current admin must sign)
stellar contract invoke \
  --id $CONTRACT_TTL_VAULT \
  --network $STELLAR_NETWORK \
  --source $DEPLOYER_IDENTITY \
  -- propose_admin \
  --new_admin <new-admin-address>

# Step 2: New admin accepts (new admin must sign)
stellar contract invoke \
  --id $CONTRACT_TTL_VAULT \
  --network $STELLAR_NETWORK \
  --source <new-admin-identity> \
  -- accept_admin
```

If the current admin key is already compromised and cannot sign, the contract must be redeployed. Coordinate with vault owners to migrate funds via `withdraw` before redeployment.

---

## 6. Contact Escalation

| Role | Contact | When to Escalate |
|------|---------|-----------------|
| On-call operator | Internal team channel | Any P0/P1 incident |
| Security team | security@ttl-legacy.example.com | Suspected exploit or key compromise |
| Stellar SDF support | https://stellar.org/developers | Ledger-level or network issues |
| External auditor | (contract-specific contact) | Unresolved critical findings |

Escalate to the next level if no response within 30 minutes for P0 incidents.

---

## 7. User Communication Templates

### Planned Maintenance

> **[TTL-Legacy] Scheduled Maintenance — [DATE] [TIME] UTC**
>
> We will be performing scheduled maintenance on the TTL-Legacy contract. During this window, vault operations (deposit, withdraw, check-in, release) will be temporarily unavailable.
>
> Expected duration: [DURATION]
> Your vault state and funds are safe and will be fully accessible after maintenance.

### Unplanned Outage

> **[TTL-Legacy] Service Disruption — [DATE] [TIME] UTC**
>
> We are currently investigating an issue affecting vault operations. As a precaution, the contract has been paused to protect user funds.
>
> No funds have been lost. We will provide an update within [TIMEFRAME].
> For urgent inquiries: security@ttl-legacy.example.com

### All-Clear

> **[TTL-Legacy] Service Restored — [DATE] [TIME] UTC**
>
> The issue has been resolved and the contract is fully operational. All vault operations are available.
>
> A full post-incident report will be published within 72 hours.

---

## 8. Post-Incident Review

Complete within 72 hours of incident resolution.

**Required sections:**

1. **Timeline** — Chronological log of events from detection to resolution
2. **Root Cause** — Technical description of what went wrong
3. **Impact** — Vaults affected, funds at risk, user impact
4. **Response Actions** — Steps taken and who took them
5. **Lessons Learned** — What worked, what didn't
6. **Action Items** — Concrete follow-up tasks with owners and due dates

Publish the report as a GitHub Security Advisory or in the project's incident log.

---

## 9. Automated Disaster Recovery Scripts

> Issue #1147: Automate Disaster Recovery Runbook Steps as CI/CD Scripts

The manual steps in sections 1–4 are complemented by two automation scripts
and a nightly CI/CD workflow that reduce human error during incidents.

### `scripts/dr_backup.sh`

Snapshots the contract state and backend database to an S3-compatible bucket.

```bash
# Required environment variables
export DR_S3_BUCKET=my-ttl-legacy-dr
export DR_S3_ENDPOINT=https://s3.amazonaws.com
export CONTRACT_TTL_VAULT=<contract-id>
export STELLAR_NETWORK=mainnet
export DR_DB_PATH=/var/lib/ttl-legacy/backend.db

# Dry-run (prints actions without executing)
./scripts/dr_backup.sh --dry-run

# Live backup
./scripts/dr_backup.sh
```

**What it does:**
1. Exports contract state via `stellar contract read`
2. Creates a hot SQLite snapshot using `.backup` command
3. Uploads contract state JSON, database snapshot, and WASM artefact to
   `s3://<DR_S3_BUCKET>/backups/<TIMESTAMP>/`
4. Writes a `manifest.json` with artefact paths and metadata

### `scripts/dr_restore.sh`

Restores the database from a snapshot and re-deploys the contract from a
backed-up WASM.

```bash
# Required environment variables (same as backup, plus DEPLOYER_IDENTITY)
export DEPLOYER_IDENTITY=deployer-mainnet

# Dry-run first to verify what will happen
./scripts/dr_restore.sh --backup-prefix backups/20260727T020000Z --dry-run

# Live restore
./scripts/dr_restore.sh --backup-prefix backups/20260727T020000Z
```

**What it does:**
1. Downloads the backup manifest to confirm the target network
2. Restores the database (backs up the existing file first)
3. Re-deploys the contract WASM via `stellar contract deploy`
4. Verifies the restored database is readable

### Nightly Backup Workflow

The nightly backup is automated via `.github/workflows/dr-backup.yml` which runs at
02:00 UTC every day. It can also be triggered manually via the GitHub Actions UI
with an optional dry-run flag.

**Required repository secrets:**

| Secret | Description |
|---|---|
| `DR_AWS_ACCESS_KEY_ID` | AWS access key with S3 write permissions |
| `DR_AWS_SECRET_ACCESS_KEY` | AWS secret key |
| `DR_AWS_REGION` | AWS region (e.g. `us-east-1`) |
| `DR_S3_BUCKET` | S3 bucket name |
| `DR_S3_ENDPOINT` | S3 endpoint URL |
| `CONTRACT_TTL_VAULT` | Deployed contract address |
| `DR_DB_PATH` | Path to the backend database on the runner |
| `DR_GPG_PRIVATE_KEY` | ASCII-armored GPG private key used to sign every artefact (`gpg --armor --export-secret-keys <id>`). Must not be passphrase-protected, because `dr_backup.sh` signs with `gpg --batch`. Use a dedicated DR signing key. |
| `DR_GPG_KEY_ID` | Key ID or fingerprint of that signing key |
| `DR_STELLAR_RPC_URL` | Soroban RPC URL for mainnet. The Stellar CLI ships no default mainnet RPC (see https://developers.stellar.org/docs/data/rpc/rpc-providers) |

**Optional secrets:** `DR_REPLICA_BUCKET` + `DR_REPLICA_REGION` (cross-region
replica), `DR_VAULT_SNAPSHOTS` (comma-separated `vault_id:timestamp` pairs).

The first step, "Check required secrets", fails before anything else runs if any
required secret is missing. It lists the missing names in the run summary.
The workflow pins the Stellar CLI with `STELLAR_CLI_VERSION`. Release assets are
versioned, so the unversioned `releases/latest/download/` URL does not work.

If the backup fails, the workflow automatically opens a GitHub issue labelled
`disaster-recovery` and `bug` for visibility.
