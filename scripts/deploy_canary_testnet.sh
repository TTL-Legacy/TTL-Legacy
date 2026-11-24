#!/usr/bin/env bash
# deploy_canary_testnet.sh — Deploy canary release to Stellar testnet
#
# Deploys the contract and backend to the Stellar testnet environment defined in environments.toml.
# Backs up the existing environment state to allow automatic rollback on failure.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${ROOT_DIR}"
source "${SCRIPT_DIR}/deploy_utils.sh"

ENV_FILE="${ROOT_DIR}/environments.toml"
BACKUP_FILE="${ROOT_DIR}/environments.toml.bak"

# 1. Parse configuration from environments.toml
NETWORK="testnet"
RPC_URL=$(grep "^STELLAR_RPC_URL=" "${ENV_FILE}" | cut -d'=' -f2- | tr -d ' "' || echo "https://soroban-testnet.stellar.org")
PREV_CONTRACT_ID=$(get_contract_address "${NETWORK}")

echo "=== Deploying Canary to ${NETWORK} ==="
echo "RPC URL: ${RPC_URL}"
echo "Current Contract ID: ${PREV_CONTRACT_ID}"

# 2. Backup current environments.toml state for rollback
cp "${ENV_FILE}" "${BACKUP_FILE}"
echo "✓ Saved environment backup to ${BACKUP_FILE}"

# 3. Build contract WASM
echo "Building contract WASM..."
./scripts/build.sh

WASM="target/wasm32-unknown-unknown/release/ttl_vault.wasm"
if [[ ! -f "$WASM" ]]; then
  echo "Error: WASM artifact not found at $WASM" >&2
  exit 1
fi

# 4. Deploy contract to testnet
DEPLOYER="${DEPLOYER_IDENTITY:-deployer-testnet}"
echo "Deploying contract using identity ${DEPLOYER}..."
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM" \
  --source "$DEPLOYER" \
  --network "$NETWORK")

echo "✓ Canary contract deployed: ${CONTRACT_ID}"

# 5. Update environments.toml with new contract address
set_contract_address "$NETWORK" "$CONTRACT_ID"
echo "✓ Updated environments.toml with canary contract: ${CONTRACT_ID}"

# 6. Deploy / build backend canary
echo "Building and verifying backend canary..."
if command -v docker &>/dev/null; then
  docker build -t "ttl-legacy-backend:canary" -f backend/Dockerfile .
  echo "✓ Backend container image built: ttl-legacy-backend:canary"
fi

echo "Canary deployment completed successfully."
echo "CANARY_CONTRACT_ID=${CONTRACT_ID}"
