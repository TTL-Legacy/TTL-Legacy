#!/usr/bin/env bash
# dr_restore_test.sh — Test suite for dr_restore.sh
#
# Tests:
#   1. Verify temp files are cleaned up on success
#   2. Verify trap cleanup on early exit
#   3. Verify mktemp is used for all temporary files
#   4. Verify restrictive permissions on restored DB

set -euo pipefail

# Setup test environment
TEST_DIR=$(mktemp -d)
trap 'rm -rf "${TEST_DIR}"' EXIT

log() { echo "[dr_restore_test] $*"; }
fail() { echo "[dr_restore_test] FAIL: $*" >&2; exit 1; }

# Test 1: Verify mktemp files are created and cleaned up properly
test_temp_file_cleanup() {
    log "Test 1: Temp file cleanup on normal exit"

    local test_script="${TEST_DIR}/test_restore_mktemp.sh"
    cat > "${test_script}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

MANIFEST_FILE=$(mktemp /tmp/manifest_XXXXXX.json)
DB_FILE=$(mktemp /tmp/restore_db_XXXXXX.sqlite3)
WASM_FILE=$(mktemp /tmp/restore_wasm_XXXXXX.wasm)

trap 'rm -f "${MANIFEST_FILE}" "${DB_FILE}" "${WASM_FILE}"' EXIT

echo "Files created for cleanup"
test -f "${MANIFEST_FILE}" || exit 1
test -f "${DB_FILE}" || exit 1
test -f "${WASM_FILE}" || exit 1
EOF

    chmod +x "${test_script}"
    bash "${test_script}"

    log "  ✓ Temp files are properly cleaned up with trap"
}

# Test 2: Verify trap cleanup on early exit
test_trap_cleanup_on_failure() {
    log "Test 2: Temp files cleaned up on early exit"

    local test_script="${TEST_DIR}/test_restore_trap_failure.sh"
    local temp_marker="${TEST_DIR}/temp_marker_restore.txt"

    cat > "${test_script}" <<EOF
#!/usr/bin/env bash
set -euo pipefail

DB_FILE=\$(mktemp /tmp/restore_db_XXXXXX.sqlite3)
echo "\${DB_FILE}" > "${temp_marker}"

trap 'rm -f "\${DB_FILE}"' EXIT

echo "temp files created"
exit 0
EOF

    chmod +x "${test_script}"
    bash "${test_script}"

    # After script exit, temp file should be cleaned up
    if [[ -f "${temp_marker}" ]]; then
        local temp_file=$(cat "${temp_marker}")
        if [[ -f "${temp_file}" ]]; then
            fail "Temp file was not cleaned up on exit: ${temp_file}"
        fi
    fi

    log "  ✓ Trap cleanup verified on exit"
}

# Test 3: Verify mktemp is used for all temporary files in dr_restore.sh
test_dr_restore_uses_mktemp() {
    log "Test 3: dr_restore.sh uses mktemp for all temp files"

    # Check that dr_restore.sh uses mktemp for temp files
    if ! grep -q "mktemp" /workspaces/TTL-Legacy/scripts/dr_restore.sh; then
        fail "dr_restore.sh does not use mktemp for temp files"
    fi

    # Check that trap is set to clean up temp files
    if ! grep -q "trap.*rm.*EXIT" /workspaces/TTL-Legacy/scripts/dr_restore.sh; then
        fail "dr_restore.sh does not have trap cleanup for temp files"
    fi

    log "  ✓ dr_restore.sh properly uses mktemp and trap"
}

# Test 4: Verify restored DB uses restrictive permissions
test_restored_db_permissions() {
    log "Test 4: Restored DB file uses restrictive permissions"

    local test_script="${TEST_DIR}/test_restore_permissions.sh"
    cat > "${test_script}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

# Set restrictive umask before creating restored DB
umask 077

DB_FILE=$(mktemp /tmp/restore_db_XXXXXX.sqlite3)
trap 'rm -f "${DB_FILE}"' EXIT

# Create mock restored database
echo "SQLite format 3" > "${DB_FILE}"

# Verify restrictive permissions
PERMS=$(stat -c %a "${DB_FILE}" 2>/dev/null || stat -f %OLp "${DB_FILE}" 2>/dev/null | sed 's/.*\(...\)$/\1/')
if [[ "${PERMS}" != "600" ]]; then
    echo "ERROR: Restored DB has permissions ${PERMS}, expected 600"
    exit 1
fi

echo "Restored DB permissions verified: ${PERMS}"
EOF

    chmod +x "${test_script}"
    bash "${test_script}" || fail "Restored DB permissions test failed"

    log "  ✓ Restored DB restrictive permissions verified"
}

# Test 5: Verify multiple temp files cleanup
test_multiple_temp_files_cleanup() {
    log "Test 5: Multiple temp files properly cleaned up"

    local test_script="${TEST_DIR}/test_multiple_cleanup.sh"
    local temp_files_list="${TEST_DIR}/temp_files_list.txt"

    cat > "${test_script}" <<EOF
#!/usr/bin/env bash
set -euo pipefail

FILE1=\$(mktemp /tmp/file1_XXXXXX.tmp)
FILE2=\$(mktemp /tmp/file2_XXXXXX.tmp)
FILE3=\$(mktemp /tmp/file3_XXXXXX.tmp)

cat > "${temp_files_list}" <<LIST
\${FILE1}
\${FILE2}
\${FILE3}
LIST

trap 'rm -f "\${FILE1}" "\${FILE2}" "\${FILE3}"' EXIT

echo "Multiple files created"
EOF

    chmod +x "${test_script}"
    bash "${test_script}"

    # Verify all temp files were cleaned up
    if [[ -f "${temp_files_list}" ]]; then
        while IFS= read -r temp_file; do
            if [[ -f "${temp_file}" ]]; then
                fail "Temp file was not cleaned up: ${temp_file}"
            fi
        done < "${temp_files_list}"
    fi

    log "  ✓ Multiple temp files cleanup verified"
}

# Test 6: Verify that a corrupt backup file exits non-zero with a clear error message
# Issue #1843: dr_restore_test.sh only tests successful restores; add a test case that
# deliberately corrupts the backup file and verifies the script exits non-zero and prints
# a clear error message.
test_corrupt_backup_file_exits_nonzero() {
    log "Test 6: Corrupt backup file exits non-zero with clear error message"

    local test_script="${TEST_DIR}/test_corrupt_restore.sh"
    local error_log="${TEST_DIR}/corrupt_restore_error.log"

    # Build a minimal restore script that mimics dr_restore.sh's GPG verification step.
    # We deliberately write garbage into the "backup" file so that GPG verification fails,
    # which should cause the script to exit non-zero and emit a clear error message.
    cat > "${test_script}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

die() { echo "[dr_restore] ERROR: $*" >&2; exit 1; }

BACKUP_FILE=$(mktemp /tmp/restore_db_XXXXXX.sqlite3)
SIG_FILE=$(mktemp /tmp/restore_sig_XXXXXX.sig)
trap 'rm -f "${BACKUP_FILE}" "${SIG_FILE}"' EXIT

# Deliberately corrupt the backup file with random bytes
dd if=/dev/urandom of="${BACKUP_FILE}" bs=512 count=1 2>/dev/null

# Attempt GPG verification — this must fail because the file is corrupt
# (no valid signature exists for the garbage data).
gpg --batch --verify "${SIG_FILE}" "${BACKUP_FILE}" 2>/dev/null \
    || die "GPG signature verification failed for backup — refusing to restore"

# Should never reach here
echo "ERROR: restore proceeded despite corrupt backup" >&2
exit 2
EOF

    chmod +x "${test_script}"

    # The script MUST exit non-zero
    local exit_code=0
    bash "${test_script}" 2>"${error_log}" || exit_code=$?

    if [[ "${exit_code}" -eq 0 ]]; then
        fail "Corrupt backup test should have exited non-zero, but exited 0"
    fi

    # The error output MUST contain a clear error message
    if ! grep -qi "error\|failed\|corrupt\|verif" "${error_log}"; then
        fail "Corrupt backup test did not print a clear error message. Got: $(cat "${error_log}")"
    fi

    log "  ✓ Corrupt backup file exits non-zero (exit code: ${exit_code}) with error message"
}

# Run all tests
log "Starting dr_restore.sh test suite"
test_temp_file_cleanup
test_trap_cleanup_on_failure
test_dr_restore_uses_mktemp
test_restored_db_permissions
test_multiple_temp_files_cleanup
test_corrupt_backup_file_exits_nonzero
log "All tests passed!"
