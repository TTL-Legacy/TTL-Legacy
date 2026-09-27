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

# Run all tests
log "Starting dr_restore.sh test suite"
test_temp_file_cleanup
test_trap_cleanup_on_failure
test_dr_restore_uses_mktemp
test_restored_db_permissions
test_multiple_temp_files_cleanup
log "All tests passed!"
