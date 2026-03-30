#!/usr/bin/env bash
set -e

NETWORK="mainnet"
DEPLOYER="${DEPLOYER_IDENTITY:-deployer-mainnet}"

# Required env vars
: "${STELLAR_MAINNET_RPC_URL:?STELLAR_MAINNET_RPC_URL must be set}"

echo "⚠️  You are about to deploy TTL-Legacy to MAINNET."
echo "    Network : $NETWORK"
echo "    Identity: $DEPLOYER"
echo "    RPC URL : $STELLAR_MAINNET_RPC_URL"
echo ""
read -r -p "Type 'mainnet' to confirm: " CONFIRM
if [ "$CONFIRM" != "mainnet" ]; then
  echo "Aborted."
  exit 1
fi

./scripts/build.sh

WASM="target/wasm32-unknown-unknown/release/ttl_vault.wasm"

CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM" \
  --source "$DEPLOYER" \
  --network "$NETWORK" \
  --rpc-url "$STELLAR_MAINNET_RPC_URL")

echo "Contract deployed: $CONTRACT_ID"
echo "Add to .env: CONTRACT_TTL_VAULT=$CONTRACT_ID"
