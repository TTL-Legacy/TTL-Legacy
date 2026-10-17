#!/usr/bin/env bash
# dr_backup.sh — Disaster Recovery Backup Script
#
# Snapshots the TTL-Legacy contract state, vault snapshots and backend
# database to a configurable S3-compatible bucket.
#
# Usage:
#   ./scripts/dr_backup.sh [--dry-run]
#
# Required environment variables:
#   DR_S3_BUCKET          S3-compatible bucket name  (e.g. my-ttl-legacy-dr)
#   DR_S3_ENDPOINT        S3 endpoint URL            (e.g. https://s3.amazonaws.com)
#   CONTRACT_TTL_VAULT    Soroban contract address
#   STELLAR_NETWORK       Stellar network (testnet | mainnet | standalone)
#   DR_DB_PATH            Path to the SQLite database file
#
# Optional environment variables:
#   DR_S3_PREFIX          Key prefix inside the bucket   (default: backups)
#   AWS_ACCESS_KEY_ID     S3 credentials
#   AWS_SECRET_ACCESS_KEY S3 credentials
#   AWS_REGION            AWS region                     (default: us-east-1)
#   DR_WASM_PATH          Path to compiled WASM          (default: auto-detected)
#   DR_VAULT_SNAPSHOTS    Comma-separated vault_id:timestamp pairs to back up
#                         (e.g. 1:1700000000,2:1700003600)

set -euo pipefail

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

log()  { echo "[dr_backup] $*"; }
warn() { echo "[dr_backup] WARNING: $*" >&2; }
die()  { echo "[dr_backup] ERROR: $*" >&2; exit 1; }

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
    log "Running in DRY-RUN mode — no changes will be made."
fi

run() {
    if $DRY_RUN; then
        log "[DRY-RUN] Would run: $*"
    else
        "$@"
    fi
}

# ---------------------------------------------------------------------------
# Validate required variables
# ---------------------------------------------------------------------------

: "${DR_S3_BUCKET:?DR_S3_BUCKET is required}"
: "${DR_S3_ENDPOINT:?DR_S3_ENDPOINT is required}"
: "${CONTRACT_TTL_VAULT:?CONTRACT_TTL_VAULT is required}"
: "${STELLAR_NETWORK:?STELLAR_NETWORK is required}"
: "${DR_DB_PATH:?DR_DB_PATH is required}"

DR_S3_PREFIX="${DR_S3_PREFIX:-backups}"
AWS_REGION="${AWS_REGION:-us-east-1}"
TIMESTAMP=$(date -u +"%Y%m%dT%H%M%SZ")
BACKUP_KEY_PREFIX="${DR_S3_PREFIX}/${TIMESTAMP}"

log "Starting disaster-recovery backup"
log "  Network   : ${STELLAR_NETWORK}"
log "  Contract  : ${CONTRACT_TTL_VAULT}"
log "  Bucket    : s3://${DR_S3_BUCKET}/${BACKUP_KEY_PREFIX}/"
log "  Timestamp : ${TIMESTAMP}"

# ---------------------------------------------------------------------------
# 1. Locate WASM artifact
# ---------------------------------------------------------------------------

if [[ -n "${DR_WASM_PATH:-}" ]]; then
    WASM_PATH="${DR_WASM_PATH}"
else
    WASM_PATH="target/wasm32-unknown-unknown/release/ttl_vault.wasm"
fi

if [[ ! -f "${WASM_PATH}" ]]; then
    warn "WASM file not found at ${WASM_PATH}. Skipping WASM backup."
    SKIP_WASM=true
else
    SKIP_WASM=false
    log "WASM artifact : ${WASM_PATH}"
fi

# ---------------------------------------------------------------------------
# 2. Export contract state snapshot via Stellar CLI
# ---------------------------------------------------------------------------

CONTRACT_STATE_FILE=$(mktemp /tmp/contract_state_XXXXXX.json)
trap 'rm -f "${CONTRACT_STATE_FILE}" "${DB_SNAPSHOT_FILE:-}"; rm -rf "${VAULT_SNAPSHOT_DIR:-}"' EXIT

log "Exporting contract state for contract ${CONTRACT_TTL_VAULT} on ${STELLAR_NETWORK}..."
if $DRY_RUN; then
    log "[DRY-RUN] Would run: stellar contract read --id ${CONTRACT_TTL_VAULT} --network ${STELLAR_NETWORK} > ${CONTRACT_STATE_FILE}"
else
    stellar contract read \
        --id "${CONTRACT_TTL_VAULT}" \
        --network "${STELLAR_NETWORK}" \
        > "${CONTRACT_STATE_FILE}" \
        || die "stellar contract read failed — is the Stellar CLI installed and authenticated?"
    log "Contract state exported ($(wc -c < "${CONTRACT_STATE_FILE}") bytes)"
fi

# ---------------------------------------------------------------------------
# 3. Snapshot the backend SQLite database
# ---------------------------------------------------------------------------

if [[ ! -f "${DR_DB_PATH}" ]]; then
    warn "Database file not found at ${DR_DB_PATH}. Skipping database backup."
    SKIP_DB=true
else
    SKIP_DB=false
    DB_SNAPSHOT_FILE=$(mktemp /tmp/ttl_legacy_db_XXXXXX.sqlite3)
    log "Snapshotting database: ${DR_DB_PATH} -> ${DB_SNAPSHOT_FILE}"
    if $DRY_RUN; then
        log "[DRY-RUN] Would copy ${DR_DB_PATH} to ${DB_SNAPSHOT_FILE}"
    else
        # Use SQLite .backup command for a hot, consistent snapshot
        sqlite3 "${DR_DB_PATH}" ".backup '${DB_SNAPSHOT_FILE}'" \
            || { warn "sqlite3 not available, falling back to cp"; cp "${DR_DB_PATH}" "${DB_SNAPSHOT_FILE}"; }
        log "Database snapshot created ($(wc -c < "${DB_SNAPSHOT_FILE}") bytes)"
    fi
fi

# ---------------------------------------------------------------------------
# 3b. Export vault snapshots
# ---------------------------------------------------------------------------
# The contract has no public index of snapshot timestamps, so the operator
# lists the vault_id:timestamp pairs to back up. --send no simulates the
# read-only get_vault_snapshot call without submitting a transaction.

SKIP_VAULT_SNAPSHOTS=true
VAULT_SNAPSHOT_FILES=()
if [[ -z "${DR_VAULT_SNAPSHOTS:-}" ]]; then
    warn "DR_VAULT_SNAPSHOTS not set. Skipping vault snapshot backup."
else
    SKIP_VAULT_SNAPSHOTS=false
    VAULT_SNAPSHOT_DIR=$(mktemp -d /tmp/vault_snapshots_XXXXXX)
    IFS=',' read -ra SNAPSHOT_REFS <<< "${DR_VAULT_SNAPSHOTS}"
    for ref in "${SNAPSHOT_REFS[@]}"; do
        [[ "${ref}" =~ ^[0-9]+:[0-9]+$ ]] || die "Invalid DR_VAULT_SNAPSHOTS entry '${ref}' (expected vault_id:timestamp)"
        vault_id="${ref%%:*}"
        timestamp="${ref##*:}"
        snapshot_file="${VAULT_SNAPSHOT_DIR}/vault_${vault_id}_${timestamp}.json"
        VAULT_SNAPSHOT_FILES+=("${snapshot_file}")

        log "Exporting snapshot for vault ${vault_id} at ${timestamp}..."
        if $DRY_RUN; then
            log "[DRY-RUN] Would run: stellar contract invoke --send no --id ${CONTRACT_TTL_VAULT} --network ${STELLAR_NETWORK} -- get_vault_snapshot --vault_id ${vault_id} --timestamp ${timestamp} > ${snapshot_file}"
        else
            stellar contract invoke \
                --id "${CONTRACT_TTL_VAULT}" \
                --network "${STELLAR_NETWORK}" \
                --send no \
                -- get_vault_snapshot \
                --vault_id "${vault_id}" \
                --timestamp "${timestamp}" \
                > "${snapshot_file}" \
                || die "Failed to export snapshot for vault ${vault_id} at ${timestamp}"
            log "Vault snapshot exported ($(wc -c < "${snapshot_file}") bytes)"
        fi
    done
fi

# ---------------------------------------------------------------------------
# 4. Upload artefacts to S3-compatible storage
# ---------------------------------------------------------------------------

upload_to_s3() {
    local local_path="$1"
    local s3_key="$2"
    log "Uploading ${local_path} -> s3://${DR_S3_BUCKET}/${s3_key}"
    run aws s3 cp "${local_path}" "s3://${DR_S3_BUCKET}/${s3_key}" \
        --endpoint-url "${DR_S3_ENDPOINT}" \
        --region "${AWS_REGION}"
}

upload_to_s3 "${CONTRACT_STATE_FILE}" "${BACKUP_KEY_PREFIX}/contract_state.json"

if ! $SKIP_DB; then
    upload_to_s3 "${DB_SNAPSHOT_FILE}" "${BACKUP_KEY_PREFIX}/backend_db.sqlite3"
fi

if ! $SKIP_WASM; then
    upload_to_s3 "${WASM_PATH}" "${BACKUP_KEY_PREFIX}/ttl_vault.wasm"
fi

if ! $SKIP_VAULT_SNAPSHOTS; then
    for snapshot_file in "${VAULT_SNAPSHOT_FILES[@]}"; do
        upload_to_s3 "${snapshot_file}" "${BACKUP_KEY_PREFIX}/vault_snapshots/$(basename "${snapshot_file}")"
    done
fi

# ---------------------------------------------------------------------------
# 5. Write backup manifest
# ---------------------------------------------------------------------------

MANIFEST_FILE=$(mktemp /tmp/manifest_XXXXXX.json)
cat > "${MANIFEST_FILE}" <<EOF
{
  "timestamp": "${TIMESTAMP}",
  "network": "${STELLAR_NETWORK}",
  "contract": "${CONTRACT_TTL_VAULT}",
  "artefacts": {
    "contract_state": "${BACKUP_KEY_PREFIX}/contract_state.json",
    "backend_db": "$(if $SKIP_DB; then echo null; else echo "\"${BACKUP_KEY_PREFIX}/backend_db.sqlite3\""; fi)",
    "wasm": "$(if $SKIP_WASM; then echo null; else echo "\"${BACKUP_KEY_PREFIX}/ttl_vault.wasm\""; fi)",
    "vault_snapshots": "$(if $SKIP_VAULT_SNAPSHOTS; then echo null; else echo "\"${BACKUP_KEY_PREFIX}/vault_snapshots/\""; fi)"
  }
}
EOF

upload_to_s3 "${MANIFEST_FILE}" "${BACKUP_KEY_PREFIX}/manifest.json"
rm -f "${MANIFEST_FILE}"

log "Backup complete — prefix: s3://${DR_S3_BUCKET}/${BACKUP_KEY_PREFIX}/"
