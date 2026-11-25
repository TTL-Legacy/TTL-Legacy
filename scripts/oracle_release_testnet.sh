#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

: "${TESTNET_ADMIN_IDENTITY:?Set TESTNET_ADMIN_IDENTITY to a funded Stellar CLI identity}"
: "${TESTNET_OWNER_IDENTITY:?Set TESTNET_OWNER_IDENTITY to a separate funded identity}"
: "${TESTNET_BENEFICIARY_ADDRESS:?Set TESTNET_BENEFICIARY_ADDRESS to a Stellar account address}"
: "${TESTNET_TOKEN_ID:?Set TESTNET_TOKEN_ID to a testnet SEP-41 token contract ID}"

NETWORK="testnet"
ADMIN_ADDRESS="$(stellar keys address "$TESTNET_ADMIN_IDENTITY")"
OWNER_ADDRESS="$(stellar keys address "$TESTNET_OWNER_IDENTITY")"
PROOF="0x$(printf '01%.0s' {1..256})"
CLAIM="0x$(printf '02%.0s' {1..32})"

cd "$ROOT_DIR"
cargo build --release --target wasm32-unknown-unknown \
  --manifest-path contracts/ttl_vault/Cargo.toml
cargo build --release --target wasm32-unknown-unknown \
  --manifest-path contracts/zk_verifier/Cargo.toml

VERIFIER_ID="$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/zk_verifier.wasm \
  --source "$TESTNET_ADMIN_IDENTITY" --network "$NETWORK")"
VAULT_ID="$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/ttl_vault.wasm \
  --source "$TESTNET_ADMIN_IDENTITY" --network "$NETWORK")"

invoke() {
  local contract_id="$1"
  local source_identity="$2"
  shift 2
  stellar contract invoke --id "$contract_id" --source "$source_identity" \
    --network "$NETWORK" -- "$@"
}

invoke "$VERIFIER_ID" "$TESTNET_ADMIN_IDENTITY" initialize --admin "$ADMIN_ADDRESS"
invoke "$VERIFIER_ID" "$TESTNET_ADMIN_IDENTITY" register_oracle --oracle "$OWNER_ADDRESS"
invoke "$VERIFIER_ID" "$TESTNET_OWNER_IDENTITY" attest \
  --oracle "$OWNER_ADDRESS" --proof "$PROOF" --claim "$CLAIM"

invoke "$VAULT_ID" "$TESTNET_ADMIN_IDENTITY" initialize \
  --xlm_token "$TESTNET_TOKEN_ID" --admin "$ADMIN_ADDRESS"
CREATE_RESULT="$(invoke "$VAULT_ID" "$TESTNET_OWNER_IDENTITY" create_vault \
  --owner "$OWNER_ADDRESS" --beneficiary "$TESTNET_BENEFICIARY_ADDRESS" \
  --check_in_interval 3600 --token_address null)"
VAULT_NUMBER="$(printf '%s\n' "$CREATE_RESULT" | tail -n 1 | tr -d '"[:space:]')"
[[ "$VAULT_NUMBER" =~ ^[0-9]+$ ]] || {
  printf 'Could not parse vault ID from create_vault output: %s\n' "$CREATE_RESULT" >&2
  exit 1
}

invoke "$VAULT_ID" "$TESTNET_OWNER_IDENTITY" deposit \
  --vault_id "$VAULT_NUMBER" --from "$OWNER_ADDRESS" --amount 1
invoke "$VAULT_ID" "$TESTNET_OWNER_IDENTITY" set_release_condition \
  --vault_id "$VAULT_NUMBER" --caller "$OWNER_ADDRESS" \
  --condition "{\"Oracle\":\"$VERIFIER_ID\"}"

VERIFY_RESULT="$(invoke "$VERIFIER_ID" "$TESTNET_OWNER_IDENTITY" verify_claim \
  --proof "$PROOF" --claim "$CLAIM")"
[[ "$VERIFY_RESULT" == *true* ]] || {
  printf 'Verifier did not accept the attested proof: %s\n' "$VERIFY_RESULT" >&2
  exit 1
}

RELEASE_RESULT="$(invoke "$VAULT_ID" "$TESTNET_OWNER_IDENTITY" trigger_release \
  --vault_id "$VAULT_NUMBER")"
STATUS_RESULT="$(invoke "$VAULT_ID" "$TESTNET_OWNER_IDENTITY" get_release_status \
  --vault_id "$VAULT_NUMBER")"
[[ "$STATUS_RESULT" == *Released* ]] || {
  printf 'Vault did not reach Released status: %s\n%s\n' \
    "$STATUS_RESULT" "$RELEASE_RESULT" >&2
  exit 1
}

printf 'Oracle release succeeded on testnet.\nVerifier: %s\nVault contract: %s\nVault ID: %s\n' \
  "$VERIFIER_ID" "$VAULT_ID" "$VAULT_NUMBER"