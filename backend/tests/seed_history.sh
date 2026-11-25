#!/usr/bin/env bash
# seed_history.sh — Pre-seed a vault with 10,000 check-in records for the
# #2080 load test scenario (check_in_history in k6_load_test.js).
#
# Usage:
#   BASE_URL=http://localhost:3000 HISTORY_VAULT_ID=history-load-test-vault bash seed_history.sh
#
# The script posts check-ins in parallel batches to keep seeding time under
# two minutes on a standard developer laptop.
#
# Environment variables:
#   BASE_URL          Backend base URL.           Default: http://localhost:3000
#   HISTORY_VAULT_ID  Vault to seed.              Default: history-load-test-vault
#   SEED_COUNT        Number of records to create. Default: 10000
#   BATCH_SIZE        Requests per parallel batch. Default: 50

set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:3000}"
HISTORY_VAULT_ID="${HISTORY_VAULT_ID:-history-load-test-vault}"
SEED_COUNT="${SEED_COUNT:-10000}"
BATCH_SIZE="${BATCH_SIZE:-50}"

echo "Seeding vault '${HISTORY_VAULT_ID}' with ${SEED_COUNT} check-in records…"
echo "Target: ${BASE_URL}/api/vaults/${HISTORY_VAULT_ID}/check-in"

success=0
failure=0

for ((i = 1; i <= SEED_COUNT; i += BATCH_SIZE)); do
  pids=()
  for ((j = i; j < i + BATCH_SIZE && j <= SEED_COUNT; j++)); do
    payload="{\"signedPayload\":\"seed-record-${j}\"}"
    curl -sf -X POST \
      -H "Content-Type: application/json" \
      -d "${payload}" \
      "${BASE_URL}/api/vaults/${HISTORY_VAULT_ID}/check-in" \
      -o /dev/null &
    pids+=("$!")
  done

  for pid in "${pids[@]}"; do
    if wait "${pid}"; then
      ((success++)) || true
    else
      ((failure++)) || true
    fi
  done

  echo "  seeded ${i} / ${SEED_COUNT} (${failure} failures so far)…"
done

echo ""
echo "Done. ${success} records created, ${failure} failures."
if ((failure > 0)); then
  echo "WARNING: ${failure} requests failed. The load test may not have exactly ${SEED_COUNT} records." >&2
  exit 1
fi
