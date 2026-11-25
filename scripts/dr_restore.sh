#!/usr/bin/env bash
# dr_restore.sh — Disaster Recovery Restore Script
#
# Restores the TTL-Legacy backend database from a snapshot stored in an
# S3-compatible bucket and re-deploys the contract from a backed-up WASM.
#
# Usage:
#   ./scripts/dr_restore.sh --backup-prefix <PREFIX> [--dry-run]
#
#   <PREFIX>  The timestamp-prefixed path inside the bucket
#             (e.g. backups/20260727T120000Z)
#
# Required environment variables:
#   DR_S3_BUCKET          S3-compatible bucket name
#   DR_S3_ENDPOINT        S3 endpoint URL
#   STELLAR_NETWORK       Stellar network (testnet | mainnet | standalone)
#   DEPLOYER_IDENTITY     Stellar CLI key name to sign the re-deployment
#   DR_DB_PATH            Destination path for the restored database
#
# Optional environment variables:
#   DR_S3_PREFIX          Key prefix inside the bucket  (default: backups)
#   AWS_ACCESS_KEY_ID     S3 credentials
#   AWS_SECRET_ACCESS_KEY S3 credentials
#   AWS_REGION            AWS region                    (default: us-east-1)
#
# Every artefact is verified against its detached GPG signature (<key>.sig)
# before it is used. The signing public key must be in the local GPG keyring.

set -euo pipefail

# Repository root (the parent of scripts/), used to locate backend/migrations
# for schema-version validation.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

log()  { echo "[dr_restore] $*"; }
warn() { echo "[dr_restore] WARNING: $*" >&2; }
die()  { echo "[dr_restore] ERROR: $*" >&2; exit 1; }

DRY_RUN=false
BACKUP_PREFIX=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --backup-prefix)
            BACKUP_PREFIX="$2"
            shift 2
            ;;
        *)
            die "Unknown argument: $1"
            ;;
    esac
done

if $DRY_RUN; then
    log "Running in DRY-RUN mode — no changes will be made."
fi

# ---------------------------------------------------------------------------
# Validate required variables
# ---------------------------------------------------------------------------

: "${DR_S3_BUCKET:?DR_S3_BUCKET is required}"
: "${DR_S3_ENDPOINT:?DR_S3_ENDPOINT is required}"
: "${STELLAR_NETWORK:?STELLAR_NETWORK is required}"
: "${DEPLOYER_IDENTITY:?DEPLOYER_IDENTITY is required}"
: "${DR_DB_PATH:?DR_DB_PATH is required}"

AWS_REGION="${AWS_REGION:-us-east-1}"

if [[ -z "${BACKUP_PREFIX}" ]]; then
    die "--backup-prefix is required. Run dr_backup.sh first and note the printed prefix."
fi

log "Starting disaster-recovery restore"
log "  Network         : ${STELLAR_NETWORK}"
log "  Backup prefix   : s3://${DR_S3_BUCKET}/${BACKUP_PREFIX}/"
log "  DB destination  : ${DR_DB_PATH}"

run() {
    if $DRY_RUN; then
        log "[DRY-RUN] Would run: $*"
    else
        "$@"
    fi
}

# ---------------------------------------------------------------------------
# Schema version verification (Issue #2149)
# ---------------------------------------------------------------------------

# Query the restored database for the latest applied migration version
# (sqlx's _sqlx_migrations.version column). Prints the version, or nothing
# when it cannot be read (e.g. missing migration table or invalid database).
latest_applied_migration_version() {
    local db_path="$1"
    local version=""

    [[ -f "${db_path}" ]] || return 0
    version=$(sqlite3 "${db_path}" "SELECT MAX(version) FROM _sqlx_migrations;" 2>/dev/null) || true
    [[ "${version}" =~ ^[0-9]+$ ]] || version=""
    printf '%s\n' "${version}"
}

# Resolve the migration version the current backend binary expects: the
# highest version among the migration files in backend/migrations. sqlx parses
# the leading number of each file name (e.g. 0009_... -> 9) and records it as
# the applied version in _sqlx_migrations, so the highest migration file
# version is the expected version.
expected_migration_version() {
    local migrations_dir="${REPO_ROOT}/backend/migrations"
    local expected=""
    local f base ver

    if [[ -d "${migrations_dir}" ]]; then
        for f in "${migrations_dir}"/*.sql; do
            [[ -f "${f}" ]] || continue
            base="${f##*/}"
            if [[ "${base}" =~ ^([0-9]+)_.*\.sql$ ]]; then
                ver=$((10#${BASH_REMATCH[1]}))
                if [[ -z "${expected}" ]] || [[ "${ver}" -gt "${expected}" ]]; then
                    expected="${ver}"
                fi
            fi
        done
    fi

    printf '%s\n' "${expected}"
}

# Verify that the restored database's applied migration version matches the
# migration version expected by the current backend binary (Issue #2149).
#
# Usage: verify_schema_version <db_path> [expected_version]
#
# Fails closed (non-zero exit) when either version is unreadable or the two
# diverge. Logs a confirmation when the versions match.
verify_schema_version() {
    local db_path="$1"
    local expected_version="${2:-}"
    local restored_version=""

    if ! command -v sqlite3 &>/dev/null; then
        die "Schema version verification failed: sqlite3 is not available in PATH."
    fi

    restored_version=$(latest_applied_migration_version "${db_path}")

    if [[ -z "${expected_version}" ]]; then
        expected_version=$(expected_migration_version)
    fi
    expected_version="${expected_version//[[:space:]]/}"

    if [[ -z "${restored_version}" ]]; then
        die "Schema version verification failed: could not read the applied migration version from the restored database (${db_path}). Missing _sqlx_migrations table or unreadable database."
    fi

    if [[ -z "${expected_version}" ]]; then
        die "Schema version verification failed: could not determine the migration version expected by the current backend (missing or unreadable backend/migrations)."
    fi

    if ! [[ "${restored_version}" =~ ^[0-9]+$ ]]; then
        die "Schema version verification failed: unreadable restored migration version ('${restored_version}')."
    fi
    if ! [[ "${expected_version}" =~ ^[0-9]+$ ]]; then
        die "Schema version verification failed: unreadable expected migration version ('${expected_version}')."
    fi

    # Normalize away leading zeros: [[ 08 -eq 8 ]] is invalid bash arithmetic,
    # so strip leading zeros through base-10 arithmetic instead.
    restored_version=$((10#${restored_version}))
    expected_version=$((10#${expected_version}))

    if [[ "${restored_version}" != "${expected_version}" ]]; then
        die "Schema version mismatch after restore (restored DB: ${restored_version}, expected backend: ${expected_version}). Aborting."
    fi

    log "Schema version verified: restored database version ${restored_version} matches the backend expectation (${expected_version})."
}

# ---------------------------------------------------------------------------
# 1. Download backup manifest
# ---------------------------------------------------------------------------

MANIFEST_FILE=$(mktemp /tmp/manifest_XXXXXX.json)
SIG_DIR=$(mktemp -d /tmp/dr_sig_XXXXXX)
trap 'rm -f "${MANIFEST_FILE}" "${WASM_FILE:-}" "${DB_FILE:-}"; rm -rf "${SIG_DIR}"' EXIT

# Download an artefact and its detached signature, then abort unless the
# signature verifies. Nothing is read or restored from an unverified file.
download_verified() {
    local s3_key="$1"
    local local_path="$2"
    local sig_path
    sig_path="${SIG_DIR}/$(basename "${local_path}").sig"

    aws s3 cp \
        "s3://${DR_S3_BUCKET}/${s3_key}" \
        "${local_path}" \
        --endpoint-url "${DR_S3_ENDPOINT}" \
        --region "${AWS_REGION}" \
        || die "Failed to download ${s3_key}"

    aws s3 cp \
        "s3://${DR_S3_BUCKET}/${s3_key}.sig" \
        "${sig_path}" \
        --endpoint-url "${DR_S3_ENDPOINT}" \
        --region "${AWS_REGION}" \
        || die "Failed to download signature for ${s3_key}"

    gpg --batch --verify "${sig_path}" "${local_path}" \
        || die "GPG signature verification failed for ${s3_key} — refusing to restore"
    log "Signature verified: ${s3_key}"
}

log "Downloading manifest from s3://${DR_S3_BUCKET}/${BACKUP_PREFIX}/manifest.json ..."
if $DRY_RUN; then
    log "[DRY-RUN] Would download manifest"
    MANIFEST_NETWORK="${STELLAR_NETWORK}"
else
    download_verified "${BACKUP_PREFIX}/manifest.json" "${MANIFEST_FILE}"

    MANIFEST_NETWORK=$(python3 -c "import json,sys; d=json.load(open('${MANIFEST_FILE}')); print(d['network'])" 2>/dev/null || echo "")
    log "Manifest network: ${MANIFEST_NETWORK}"

    if [[ -n "${MANIFEST_NETWORK}" && "${MANIFEST_NETWORK}" != "${STELLAR_NETWORK}" ]]; then
        warn "Manifest network (${MANIFEST_NETWORK}) differs from STELLAR_NETWORK (${STELLAR_NETWORK})."
        warn "Continuing — make sure you intended to restore across networks."
    fi
fi

# ---------------------------------------------------------------------------
# 2. Restore backend database
# ---------------------------------------------------------------------------

DB_FILE=$(mktemp /tmp/restore_db_XXXXXX.sqlite3)
log "Downloading database snapshot..."
if $DRY_RUN; then
    log "[DRY-RUN] Would download s3://${DR_S3_BUCKET}/${BACKUP_PREFIX}/backend_db.sqlite3"
    log "[DRY-RUN] Would restore to ${DR_DB_PATH}"
else
    download_verified "${BACKUP_PREFIX}/backend_db.sqlite3" "${DB_FILE}"

    log "Database snapshot downloaded ($(wc -c < "${DB_FILE}") bytes)"

    # Back up existing database before overwriting
    if [[ -f "${DR_DB_PATH}" ]]; then
        EXISTING_BACKUP="${DR_DB_PATH}.pre_restore_$(date -u +"%Y%m%dT%H%M%SZ")"
        log "Backing up existing database to ${EXISTING_BACKUP}"
        cp "${DR_DB_PATH}" "${EXISTING_BACKUP}"
    fi

    # Restore
    cp "${DB_FILE}" "${DR_DB_PATH}"
    log "Database restored to ${DR_DB_PATH}"

    # Verify the restored database's applied migration version matches the
    # version the current backend binary expects (Issue #2149).
    if ! $DRY_RUN; then
        verify_schema_version "${DR_DB_PATH}"
    fi
fi

# ---------------------------------------------------------------------------
# 3. Re-deploy contract from backed-up WASM
# ---------------------------------------------------------------------------

WASM_FILE=$(mktemp /tmp/restore_wasm_XXXXXX.wasm)
log "Downloading WASM from s3://${DR_S3_BUCKET}/${BACKUP_PREFIX}/ttl_vault.wasm ..."
if $DRY_RUN; then
    log "[DRY-RUN] Would download s3://${DR_S3_BUCKET}/${BACKUP_PREFIX}/ttl_vault.wasm"
    log "[DRY-RUN] Would run: stellar contract deploy --wasm <wasm> --source ${DEPLOYER_IDENTITY} --network ${STELLAR_NETWORK}"
else
    download_verified "${BACKUP_PREFIX}/ttl_vault.wasm" "${WASM_FILE}"

    log "WASM downloaded ($(wc -c < "${WASM_FILE}") bytes). Deploying contract..."

    NEW_CONTRACT_ID=$(stellar contract deploy \
        --wasm "${WASM_FILE}" \
        --source "${DEPLOYER_IDENTITY}" \
        --network "${STELLAR_NETWORK}") \
        || die "Contract re-deployment failed"

    log "Contract re-deployed: ${NEW_CONTRACT_ID}"
    log "Update CONTRACT_TTL_VAULT=${NEW_CONTRACT_ID} in your environment / environments.toml"
fi

# ---------------------------------------------------------------------------
# 4. Verify restored database is readable
# ---------------------------------------------------------------------------

if ! $DRY_RUN; then
    if command -v sqlite3 &>/dev/null; then
        TABLE_COUNT=$(sqlite3 "${DR_DB_PATH}" "SELECT COUNT(*) FROM sqlite_master WHERE type='table';" 2>/dev/null || echo "?")
        log "Post-restore DB check: ${TABLE_COUNT} table(s) found"
    else
        warn "sqlite3 not in PATH — skipping DB integrity check"
    fi
fi

log "Disaster-recovery restore complete."
if $DRY_RUN; then
    log "This was a dry run. Re-run without --dry-run to apply changes."
fi
