#!/usr/bin/env bash
set -e

source "$(dirname "$0")/deploy_utils.sh"

NETWORK="testnet"
DEPLOYER="deployer"
FORCE_DEPLOY=false

# Parse --force flag
if [[ "$1" == "--force" ]]; then
  FORCE_DEPLOY=true
fi

echo "Deploying TTL-Legacy to $NETWORK..."

# Check for existing deployment
EXISTING_CONTRACT=$(get_contract_address "$NETWORK")

if [[ -n "$EXISTING_CONTRACT" && "$EXISTING_CONTRACT" != "<your-contract-id>" && "$FORCE_DEPLOY" == "false" ]]; then
  echo "✓ Existing contract found: $EXISTING_CONTRACT"
  if confirm "Re-deploy to $NETWORK? This will deploy a new contract instance."; then
    echo "Proceeding with deployment..."
  else
    echo "Deployment cancelled."
    exit 0
  fi
fi

# Build first
./scripts/build.sh

WASM="target/wasm32-unknown-unknown/release/ttl_vault.wasm"

echo "Deploying contract to $NETWORK..."
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM" \
  --source "$DEPLOYER" \
  --network "$NETWORK")

echo "✓ Contract deployed: $CONTRACT_ID"

# Update environments.toml
set_contract_address "$NETWORK" "$CONTRACT_ID"
echo "✓ Updated environments.toml with new contract address"
echo "Add to .env: CONTRACT_TTL_VAULT=$CONTRACT_ID"
