#!/usr/bin/env bash
set -e

source "$(dirname "$0")/deploy_utils.sh"

NETWORK="mainnet"
DEPLOYER="${DEPLOYER_IDENTITY:-deployer-mainnet}"
FORCE_DEPLOY=false

# Required env vars
: "${STELLAR_MAINNET_RPC_URL:?STELLAR_MAINNET_RPC_URL must be set}"

# Parse --force flag
if [[ "$1" == "--force" ]]; then
  FORCE_DEPLOY=true
fi

echo "⚠️  You are about to deploy TTL-Legacy to MAINNET."
echo "    Network : $NETWORK"
echo "    Identity: $DEPLOYER"
echo "    RPC URL : $STELLAR_MAINNET_RPC_URL"

# Check for existing deployment
EXISTING_CONTRACT=$(get_contract_address "$NETWORK")

if [[ -n "$EXISTING_CONTRACT" && "$EXISTING_CONTRACT" != "<your-contract-id>" && "$FORCE_DEPLOY" == "false" ]]; then
  echo "⚠️  Existing contract found: $EXISTING_CONTRACT"
  echo ""
fi

echo ""
read -r -p "Type 'mainnet' to confirm deployment: " CONFIRM
if [ "$CONFIRM" != "mainnet" ]; then
  echo "Aborted."
  exit 1
fi

./scripts/build.sh

WASM="target/wasm32-unknown-unknown/release/ttl_vault.wasm"

echo "Deploying contract to $NETWORK..."
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM" \
  --source "$DEPLOYER" \
  --network "$NETWORK" \
  --rpc-url "$STELLAR_MAINNET_RPC_URL")

echo "✓ Contract deployed: $CONTRACT_ID"

# Update environments.toml
set_contract_address "$NETWORK" "$CONTRACT_ID"
echo "✓ Updated environments.toml with new contract address"
echo "Add to .env: CONTRACT_TTL_VAULT=$CONTRACT_ID"
