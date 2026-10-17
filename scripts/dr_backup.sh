#!/usr/bin/env bash
# dr_backup.sh — Disaster Recovery Backup Script
#
# Snapshots the TTL-Legacy contract state and backend database to a
# configurable S3-compatible bucket.
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
#
# Every uploaded artefact gets a detached GPG signature (<key>.sig) so that
# dr_restore.sh can verify it before restoring.
#   DR_GPG_KEY_ID         GPG key ID or email used to sign artefacts (required)

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
: "${DR_GPG_KEY_ID:?DR_GPG_KEY_ID is required}"

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
SIG_DIR=$(mktemp -d /tmp/dr_sig_XXXXXX)
trap 'rm -f "${CONTRACT_STATE_FILE}" "${DB_SNAPSHOT_FILE:-}"; rm -rf "${SIG_DIR}"' EXIT

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

# Sign the artefact with a detached GPG signature, then upload both files.
upload_signed() {
    local local_path="$1"
    local s3_key="$2"
    local sig_path="${SIG_DIR}/$(basename "${local_path}").sig"

    run gpg --batch --yes --local-user "${DR_GPG_KEY_ID}" \
        --detach-sign --output "${sig_path}" "${local_path}"
    upload_to_s3 "${local_path}" "${s3_key}"
    upload_to_s3 "${sig_path}" "${s3_key}.sig"
}

upload_signed "${CONTRACT_STATE_FILE}" "${BACKUP_KEY_PREFIX}/contract_state.json"

if ! $SKIP_DB; then
    upload_signed "${DB_SNAPSHOT_FILE}" "${BACKUP_KEY_PREFIX}/backend_db.sqlite3"
fi

if ! $SKIP_WASM; then
    upload_signed "${WASM_PATH}" "${BACKUP_KEY_PREFIX}/ttl_vault.wasm"
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
    "wasm": "$(if $SKIP_WASM; then echo null; else echo "\"${BACKUP_KEY_PREFIX}/ttl_vault.wasm\""; fi)"
  }
}
EOF

upload_signed "${MANIFEST_FILE}" "${BACKUP_KEY_PREFIX}/manifest.json"
rm -f "${MANIFEST_FILE}"

log "Backup complete — prefix: s3://${DR_S3_BUCKET}/${BACKUP_KEY_PREFIX}/"
