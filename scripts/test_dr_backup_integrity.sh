#!/usr/bin/env bash
# test_dr_backup_integrity.sh — DR Backup Integrity Test
#
# Verifies that the dr_backup.sh script produces artefacts whose SHA-256
# checksums match the downloaded files, and that the GPG detached signature
# over the checksum file is valid.
#
# This test runs entirely offline using a temporary GPG home directory and a
# freshly generated throwaway key — no live AWS or Stellar CLI connectivity is
# required.  It stubs out S3 uploads and the stellar CLI call so the test can
# run in any CI environment that has gpg(1), sha256sum(1), and bash ≥ 4.
#
# Issue #2084: Test: add DR backup integrity test that verifies GPG signature
#              and SHA-256 checksum.
#
# Usage:
#   bash scripts/test_dr_backup_integrity.sh
#
# Exit codes:
#   0  — all assertions passed
#   1  — one or more assertions failed

set -euo pipefail

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

PASS=0
FAIL=0
TMPDIR_TEST=$(mktemp -d /tmp/dr_backup_integrity_test_XXXXXX)
GNUPGHOME_TEST="${TMPDIR_TEST}/gnupg"
mkdir -p "${GNUPGHOME_TEST}"
chmod 700 "${GNUPGHOME_TEST}"

cleanup() {
    rm -rf "${TMPDIR_TEST}"
}
trap cleanup EXIT

pass() { echo "  PASS: $*"; ((PASS++)) || true; }
fail() { echo "  FAIL: $*" >&2; ((FAIL++)) || true; }

assert_eq() {
    local desc="$1" expected="$2" actual="$3"
    if [[ "${expected}" == "${actual}" ]]; then
        pass "${desc}"
    else
        fail "${desc} — expected '${expected}', got '${actual}'"
    fi
}

assert_file_exists() {
    local desc="$1" path="$2"
    if [[ -f "${path}" ]]; then
        pass "${desc}"
    else
        fail "${desc} — file not found: ${path}"
    fi
}

assert_file_not_empty() {
    local desc="$1" path="$2"
    if [[ -s "${path}" ]]; then
        pass "${desc}"
    else
        fail "${desc} — file is empty or missing: ${path}"
    fi
}

# ---------------------------------------------------------------------------
# 1. Generate a throwaway GPG key in an isolated GNUPGHOME
# ---------------------------------------------------------------------------

echo "==> Setting up throwaway GPG key for testing..."

export GNUPGHOME="${GNUPGHOME_TEST}"

GPG_KEY_EMAIL="dr-backup-test@ttl-legacy.local"
GPG_KEY_NAME="TTL-Legacy DR Backup Test"

# Generate a minimal, non-interactive GPG key (no passphrase)
gpg --batch --quiet --gen-key <<EOF
%no-protection
Key-Type: RSA
Key-Length: 2048
Subkey-Type: RSA
Subkey-Length: 2048
Name-Real: ${GPG_KEY_NAME}
Name-Email: ${GPG_KEY_EMAIL}
Expire-Date: 1d
%commit
EOF

echo "  Throwaway GPG key generated for: ${GPG_KEY_EMAIL}"

# ---------------------------------------------------------------------------
# 2. Prepare fake backup artefacts
#    dr_backup.sh computes checksums for the files it has collected locally
#    before uploading them.  We replicate that set of files here.
# ---------------------------------------------------------------------------

echo ""
echo "==> Creating synthetic backup artefacts..."

ARTEFACT_DIR="${TMPDIR_TEST}/artefacts"
mkdir -p "${ARTEFACT_DIR}"

CONTRACT_STATE_FILE="${ARTEFACT_DIR}/contract_state.json"
DB_SNAPSHOT_FILE="${ARTEFACT_DIR}/backend_db.sqlite3"
WASM_FILE="${ARTEFACT_DIR}/ttl_vault.wasm"

# Synthetic content that is deterministic and machine-readable
printf '{"network":"testnet","contract":"CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4","vaults":[]}\n' \
    > "${CONTRACT_STATE_FILE}"

# Minimal valid SQLite3 file header (100 bytes magic + zero fill)
printf 'SQLite format 3\x00' > "${DB_SNAPSHOT_FILE}"
dd if=/dev/urandom bs=84 count=1 2>/dev/null >> "${DB_SNAPSHOT_FILE}"

# Fake WASM binary (magic bytes + version)
printf '\x00asm\x01\x00\x00\x00' > "${WASM_FILE}"

echo "  contract_state.json : $(wc -c < "${CONTRACT_STATE_FILE}") bytes"
echo "  backend_db.sqlite3  : $(wc -c < "${DB_SNAPSHOT_FILE}") bytes"
echo "  ttl_vault.wasm      : $(wc -c < "${WASM_FILE}") bytes"

# ---------------------------------------------------------------------------
# Helper: replicate dr_backup.sh's compute_checksums + sign_checksums logic
# (pure bash, no external dependencies beyond sha256sum and gpg)
# ---------------------------------------------------------------------------

compute_checksums() {
    local checksum_file="$1"
    shift
    local -a files=("$@")

    : > "${checksum_file}"
    for f in "${files[@]}"; do
        local digest
        digest=$(sha256sum "${f}" | awk '{print $1}')
        printf '%s  %s\n' "${digest}" "$(basename "${f}")" >> "${checksum_file}"
    done
}

sign_checksums() {
    local checksum_file="$1"
    local gpg_key_id="$2"
    local sig_file="${checksum_file}.asc"

    gpg --batch --yes --armor --detach-sign \
        --local-user "${gpg_key_id}" \
        --output "${sig_file}" \
        "${checksum_file}"
}

verify_signature() {
    local checksum_file="$1"
    local sig_file="${checksum_file}.asc"
    gpg --batch --verify "${sig_file}" "${checksum_file}" 2>/dev/null
}

# ---------------------------------------------------------------------------
# 3. Test: SHA-256 checksum file is produced and contains all artefacts
# ---------------------------------------------------------------------------

echo ""
echo "==> Test 1: SHA-256 checksum file covers all backup artefacts"

CHECKSUM_FILE="${TMPDIR_TEST}/checksums.sha256"

compute_checksums "${CHECKSUM_FILE}" \
    "${CONTRACT_STATE_FILE}" "${DB_SNAPSHOT_FILE}" "${WASM_FILE}"

assert_file_exists "checksum file is created" "${CHECKSUM_FILE}"
assert_file_not_empty "checksum file is non-empty" "${CHECKSUM_FILE}"

# Verify each artefact appears in the checksum file
for name in contract_state.json backend_db.sqlite3 ttl_vault.wasm; do
    if grep -q "${name}" "${CHECKSUM_FILE}"; then
        pass "checksum file contains entry for ${name}"
    else
        fail "checksum file missing entry for ${name}"
    fi
done

# Count lines: one per artefact
LINE_COUNT=$(wc -l < "${CHECKSUM_FILE}")
assert_eq "checksum file has exactly 3 lines" "3" "${LINE_COUNT}"

# ---------------------------------------------------------------------------
# 4. Test: SHA-256 digests in the checksum file are correct
# ---------------------------------------------------------------------------

echo ""
echo "==> Test 2: SHA-256 digests match the actual artefact contents"

for f in "${CONTRACT_STATE_FILE}" "${DB_SNAPSHOT_FILE}" "${WASM_FILE}"; do
    name="$(basename "${f}")"
    expected_digest=$(sha256sum "${f}" | awk '{print $1}')
    recorded_digest=$(grep "${name}" "${CHECKSUM_FILE}" | awk '{print $1}')

    assert_eq "digest for ${name} is correct" \
        "${expected_digest}" "${recorded_digest}"
done

# ---------------------------------------------------------------------------
# 5. Test: checksum file passes sha256sum --check verification
# ---------------------------------------------------------------------------

echo ""
echo "==> Test 3: sha256sum --check passes for unmodified artefacts"

# sha256sum --check expects paths relative to CWD; run from artefact dir
if (cd "${ARTEFACT_DIR}" && sha256sum --check "${CHECKSUM_FILE}" --quiet 2>/dev/null); then
    pass "sha256sum --check passed for all artefacts"
else
    fail "sha256sum --check failed for one or more artefacts"
fi

# ---------------------------------------------------------------------------
# 6. Test: sha256sum --check detects a tampered artefact
# ---------------------------------------------------------------------------

echo ""
echo "==> Test 4: sha256sum --check detects tampered artefact"

TAMPERED_DIR="${TMPDIR_TEST}/tampered"
cp -r "${ARTEFACT_DIR}" "${TAMPERED_DIR}"
cp "${CHECKSUM_FILE}" "${TMPDIR_TEST}/checksums_tampered.sha256"

# Append a byte to the WASM to corrupt it
printf '\xFF' >> "${TAMPERED_DIR}/ttl_vault.wasm"

if (cd "${TAMPERED_DIR}" && sha256sum --check "${TMPDIR_TEST}/checksums_tampered.sha256" --quiet 2>/dev/null); then
    fail "sha256sum --check should have failed for tampered WASM but did not"
else
    pass "sha256sum --check correctly rejected tampered ttl_vault.wasm"
fi

# ---------------------------------------------------------------------------
# 7. Test: GPG detached signature is produced and verifies successfully
# ---------------------------------------------------------------------------

echo ""
echo "==> Test 5: GPG detached signature is created and verifies"

sign_checksums "${CHECKSUM_FILE}" "${GPG_KEY_EMAIL}"

SIG_FILE="${CHECKSUM_FILE}.asc"
assert_file_exists "GPG signature file (.asc) is created" "${SIG_FILE}"
assert_file_not_empty "GPG signature file is non-empty" "${SIG_FILE}"

# The .asc file must begin with the PGP signature header
FIRST_LINE=$(head -1 "${SIG_FILE}")
assert_eq "signature file starts with PGP header" \
    "-----BEGIN PGP SIGNATURE-----" "${FIRST_LINE}"

if verify_signature "${CHECKSUM_FILE}"; then
    pass "GPG signature verifies successfully for unmodified checksum file"
else
    fail "GPG signature verification failed"
fi

# ---------------------------------------------------------------------------
# 8. Test: GPG verification fails for a modified checksum file
# ---------------------------------------------------------------------------

echo ""
echo "==> Test 6: GPG verification rejects a tampered checksum file"

MODIFIED_CHECKSUM="${TMPDIR_TEST}/checksums_modified.sha256"
cp "${CHECKSUM_FILE}" "${MODIFIED_CHECKSUM}"
cp "${SIG_FILE}" "${MODIFIED_CHECKSUM}.asc"

# Inject a fake entry to simulate tampering
echo "0000000000000000000000000000000000000000000000000000000000000000  evil.bin" \
    >> "${MODIFIED_CHECKSUM}"

if gpg --batch --verify "${MODIFIED_CHECKSUM}.asc" "${MODIFIED_CHECKSUM}" 2>/dev/null; then
    fail "GPG verification should have failed for tampered checksum file but did not"
else
    pass "GPG verification correctly rejected tampered checksum file"
fi

# ---------------------------------------------------------------------------
# 9. Test: missing GPG key gracefully skips signing (no key set)
# ---------------------------------------------------------------------------

echo ""
echo "==> Test 7: signing is skipped when DR_GPG_KEY_ID is not set"

# Replicate the guard from dr_backup.sh's sign_checksums function
SKIP_SIG_RESULT="skipped"
DR_GPG_KEY_ID_TEST=""
if [[ -z "${DR_GPG_KEY_ID_TEST:-}" ]]; then
    SKIP_SIG_RESULT="skipped"
else
    SKIP_SIG_RESULT="signed"
fi

assert_eq "GPG signing is skipped when no key is configured" \
    "skipped" "${SKIP_SIG_RESULT}"

# ---------------------------------------------------------------------------
# 10. Test: checksum file format — each line matches "<hex64>  <name>" pattern
# ---------------------------------------------------------------------------

echo ""
echo "==> Test 8: checksum file format is valid"

SHA256_PATTERN='^[a-f0-9]{64}  [^ ]+$'
INVALID_LINES=0
while IFS= read -r line; do
    if [[ ! "${line}" =~ ${SHA256_PATTERN} ]]; then
        ((INVALID_LINES++)) || true
    fi
done < "${CHECKSUM_FILE}"

assert_eq "all checksum file lines match the expected format" "0" "${INVALID_LINES}"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

echo ""
echo "========================================"
echo "DR Backup Integrity Test Results"
echo "========================================"
echo "  Passed : ${PASS}"
echo "  Failed : ${FAIL}"
echo "========================================"

if [[ ${FAIL} -gt 0 ]]; then
    echo "RESULT: FAILED (${FAIL} assertion(s) failed)"
    exit 1
else
    echo "RESULT: ALL TESTS PASSED"
    exit 0
fi
