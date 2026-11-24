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

> **On-call engineers:** if a nightly backup or restore verification has failed,
> jump straight to [Section 10 — Backup/Restore Failure Runbook](#10-backuprestore-failure-runbook).
> For vault-level (per-vault) export/import recovery, see
> [docs/disaster-recovery-vault-export-import.md](./disaster-recovery-vault-export-import.md).

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

### Restore Verification Workflow

`.github/workflows/dr-restore-verification.yml` runs after each nightly backup
and periodically on a schedule. It downloads the latest backup, verifies the
manifest and artefact checksums, decrypts the artefacts with GPG, and performs a
dry-run restore against a scratch environment. A failure here means the most
recent backup **cannot be trusted** — treat it as a P1 until proven otherwise.

---

## 10. Backup/Restore Failure Runbook

This section is the step-by-step procedure for an on-call engineer when a
nightly backup (`dr-backup.yml`) or restore verification
(`dr-restore-verification.yml`) fails.

### 10.1 Identify the failure type

1. Open the failed workflow run in GitHub Actions and read the **first** failing
   step. Later failures are usually cascading.
2. Read the run summary — the "Check required secrets" step lists missing
   secret names there.
3. Match the symptom against the decision table below.

| Symptom (where it fails / what you see) | Likely cause | First action |
|---|---|---|
| "Check required secrets" step fails; summary lists missing names | Missing or renamed repository secret | Re-add the secret(s) listed (see the secrets table in §9). Re-run the workflow. |
| Stellar CLI install step fails; 404 or checksum error on download | `STELLAR_CLI_VERSION` points at a non-existent release, or the unversioned `releases/latest/download/` URL was used | Pin `STELLAR_CLI_VERSION` to a real release tag; verify the asset URL is versioned. Re-run. |
| `stellar: command not found` or version mismatch after install | CLI install succeeded but PATH/version not applied | Check the install step output and `stellar --version`; fix the workflow's PATH export. |
| S3 upload fails (`AccessDenied`, `NoSuchBucket`, `403`, `SignatureDoesNotMatch`) | Bad/expired `DR_AWS_*` credentials, wrong `DR_S3_BUCKET`/`DR_S3_ENDPOINT`/region, or bucket policy | Validate credentials and bucket/endpoint/region. Confirm the key has `s3:PutObject`. Re-run. |
| S3 download fails during restore/verification (`NoSuchKey`, `404`) | Wrong `--backup-prefix`, backup never uploaded, or replica/bucket mismatch | List `s3://<DR_S3_BUCKET>/backups/` and confirm the prefix exists. Use the correct prefix. |
| GPG decrypt fails (`gpg: decryption failed: No secret key`, `Bad session key`) | `DR_GPG_PRIVATE_KEY`/`DR_GPG_KEY_ID` missing, wrong key, or key rotated since backup | Confirm the key ID matches the key that signed the artefact. Restore the correct private key secret. If the key is lost, the artefact is unrecoverable — escalate. |
| Manifest checksum mismatch (`checksum mismatch`, `sha256` differs) | Artefact corrupted in transit/storage, or manifest and artefact from different runs | Do **not** restore this backup. Re-run backup; if it recurs, treat storage as suspect and escalate to infra lead. |
| Invalid WASM magic bytes (`invalid wasm`, `magic` error) | Uploaded artefact is not a WASM (truncated download, wrong file, or HTML error page saved as `.wasm`) | Re-download and re-check size/hash. Re-run backup. Do not deploy this artefact. |
| Cross-network restore warning (`network mismatch`, manifest network ≠ target) | Restoring a testnet backup onto mainnet (or vice versa) | Stop. Confirm `STELLAR_NETWORK` and the manifest's recorded network match. Use the correct backup. |
| DB permission errors (`unable to open database file`, `readonly database`, `EACCES`) | `DR_DB_PATH` not writable by the runner, wrong path, or file owned by another user | Check path and ownership/permissions on the runner. Fix `DR_DB_PATH` or permissions. Re-run. |

If the symptom is not in this table, capture the failing step's full log, open a
`disaster-recovery` issue, and escalate (see §10.4).

### 10.2 Manual restore procedure

Use this when the automated restore/verification cannot proceed and you must
restore by hand. **Always dry-run first.**

**Step 1 — Pick the backup prefix.** List available backups and choose the most
recent one whose manifest checksums verify:

```bash
aws s3 ls "s3://${DR_S3_BUCKET}/backups/" --endpoint-url "$DR_S3_ENDPOINT"
# e.g. backups/20260727T020000Z
export BACKUP_PREFIX=backups/20260727T020000Z
```

**Step 2 — Dry-run.** This downloads the manifest, confirms the target network,
and prints every action without changing anything:

```bash
./scripts/dr_restore.sh --backup-prefix "$BACKUP_PREFIX" --dry-run
```

Confirm from the output:
- the manifest's recorded network matches `STELLAR_NETWORK`;
- the artefact checksums match;
- the GPG decrypt succeeds;
- the WASM magic bytes are valid.

If any of these fail, **stop** and go back to §10.1.

**Step 3 — Live restore.** Before overwriting the database, `dr_restore.sh`
backs up the existing file. The pre-restore backup is written next to the
target DB as:

```
<DR_DB_PATH>.pre-restore.<TIMESTAMP>
# e.g. /var/lib/ttl-legacy/backend.db.pre-restore.20260727T021500Z
```

Note this filename — you will need it if the restore must be rolled back.

```bash
./scripts/dr_restore.sh --backup-prefix "$BACKUP_PREFIX"
```

The script then re-deploys the contract WASM via `stellar contract deploy` and
verifies the restored database is readable.

**Step 4 — Post-restore verification.** Run all of these and record the output
in the incident issue:

```bash
# Database is readable and not corrupt
sqlite3 "$DR_DB_PATH" "PRAGMA integrity_check;"
# Expected: ok

# Contract is live and not paused
stellar contract invoke \
  --id "$CONTRACT_TTL_VAULT" \
  --network "$STELLAR_NETWORK" \
  -- is_paused
# Expected: false (or true if you intentionally re-paused)

# Spot-check a known vault
stellar contract invoke \
  --id "$CONTRACT_TTL_VAULT" \
  --network "$STELLAR_NETWORK" \
  -- get_vault \
  --vault_id <VAULT_ID>
```

**Rollback** (if verification fails): stop the contract if needed (§1), then
restore the pre-restore copy:

```bash
cp "$DR_DB_PATH.pre-restore.<TIMESTAMP>" "$DR_DB_PATH"
```

For per-vault recovery (a single vault's data is wrong or missing), use the
vault export/import procedure in
[docs/disaster-recovery-vault-export-import.md](./disaster-recovery-vault-export-import.md)
instead of a full restore.

### 10.3 If the backup itself is unrecoverable

If the latest backup fails checksum/GPG/WASM validation and no earlier verified
backup exists:

1. Do **not** deploy or restore the suspect artefact.
2. Preserve the failed artefacts and logs (do not delete the S3 prefix).
3. Escalate to the infra lead and security lead immediately (§10.4).
4. Open a `disaster-recovery` + `bug` issue with the failing run link.

### 10.4 Escalation contacts

> **Placeholders — fill these in before this runbook goes live.** Replace every
> `<...>` below with the real contact and keep this table current.

| Role | Name | Contact | When to escalate |
|---|---|---|---|
| Primary on-call | `<PRIMARY_ONCALL_NAME>` | `<PRIMARY_ONCALL_PHONE>` / `<PRIMARY_ONCALL_EMAIL>` | First responder for any failed backup/restore |
| Secondary on-call | `<SECONDARY_ONCALL_NAME>` | `<SECONDARY_ONCALL_PHONE>` / `<SECONDARY_ONCALL_EMAIL>` | Primary unresponsive within 15 min |
| Security lead | `<SECURITY_LEAD_NAME>` | `<SECURITY_LEAD_PHONE>` / security@ttl-legacy.example.com | Suspected key compromise, GPG key loss, or data integrity doubt |
| Infra lead | `<INFRA_LEAD_NAME>` | `<INFRA_LEAD_PHONE>` / `<INFRA_LEAD_EMAIL>` | S3/credentials/storage failures, repeated checksum mismatches |

**How to page:**

1. Post in the internal on-call channel with the failing run link and the
   failure type from §10.1.
2. If no acknowledgement within 15 minutes, page the secondary on-call via
   `<PAGING_TOOL>` (e.g. PagerDuty/Opsgenie) using the "Disaster Recovery"
   service.
3. For P0/P1 or suspected compromise, page the security lead and infra lead
   directly — do not wait for the on-call chain.

### 10.5 Post-incident review template

Complete within 72 hours of resolution. Copy the block below into the incident
issue or a GitHub Security Advisory.

```markdown
## Post-Incident Review — <short title>

**Incident ID:** <id>
**Severity:** <P0/P1/P2/P3>
**Date detected:** <YYYY-MM-DD HH:MM UTC>
**Date resolved:** <YYYY-MM-DD HH:MM UTC>
**Author:** <name>
**Failing run(s):** <workflow run links>

### Summary
<2–3 sentences: what failed, what was affected, how it was resolved.>

### Timeline
| Time (UTC) | Event |
|---|---|
| <HH:MM> | Failure detected (workflow run / alert) |
| <HH:MM> | On-call acknowledged |
| <HH:MM> | Failure type identified (§10.1) |
| <HH:MM> | Mitigation started |
| <HH:MM> | Restore verified / service restored |
| <HH:MM> | Incident closed |

### Impact
- Backups affected: <prefixes / none>
- Data at risk: <description / none>
- Vaults affected: <count / ids / none>
- User impact: <description / none>

### Response Actions
<What was done, in order, and by whom. Include commands run and their results.>

### Lessons Learned
- What worked: <...>
- What didn't: <...>
- Detection gaps: <...>

### Action Items
| Action | Owner | Due date | Tracking issue |
|---|---|---|---|
| <action> | <owner> | <YYYY-MM-DD> | <link> |
| <action> | <owner> | <YYYY-MM-DD> | <link> |
```
