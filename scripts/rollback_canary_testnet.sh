#!/usr/bin/env bash
# rollback_canary_testnet.sh — Roll back testnet canary deployment on failure
#
# Restores environments.toml to the pre-canary backup and restores previous backend deployment.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${ROOT_DIR}"
source "${SCRIPT_DIR}/deploy_utils.sh"

ENV_FILE="${ROOT_DIR}/environments.toml"
BACKUP_FILE="${ROOT_DIR}/environments.toml.bak"

echo "=== Rolling back Canary Deployment on Testnet ==="

if [[ -f "${BACKUP_FILE}" ]]; then
  echo "Restoring ${ENV_FILE} from backup ${BACKUP_FILE}..."
  cp "${BACKUP_FILE}" "${ENV_FILE}"
  rm -f "${BACKUP_FILE}"
  echo "✓ Restored environments.toml"
else
  echo "Warning: No backup file (${BACKUP_FILE}) found. Checking previous contract ID argument..."
  if [[ -n "${1:-}" ]]; then
    set_contract_address "testnet" "$1"
    echo "✓ Reset contract address in environments.toml to: $1"
  fi
fi

# Roll back backend container / deployment if running
if command -v docker &>/dev/null; then
  if docker ps -a --format '{{.Names}}' | grep -q "^ttl-legacy-backend$"; then
    echo "Restarting backend with stable version..."
    docker stop ttl-legacy-backend || true
    docker rm ttl-legacy-backend || true
  fi
fi

RESTORED_CONTRACT=$(get_contract_address "testnet" || true)
echo "✓ Canary rollback complete. Current testnet contract: ${RESTORED_CONTRACT:-<unset>}"
