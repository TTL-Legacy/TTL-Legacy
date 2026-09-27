#!/usr/bin/env bash
# shellcheck_test.sh — Test suite for shell script linting
#
# Tests:
#   1. Verify shellcheck is available
#   2. Verify dr_backup.sh passes shellcheck
#   3. Verify dr_restore.sh passes shellcheck
#   4. Verify build.sh passes shellcheck
#   5. Verify deploy_utils.sh passes shellcheck

set -euo pipefail

log() { echo "[shellcheck_test] $*"; }
fail() { echo "[shellcheck_test] FAIL: $*" >&2; exit 1; }

# Test 1: Verify shellcheck is available
test_shellcheck_available() {
    log "Test 1: shellcheck is available"

    if ! command -v shellcheck &>/dev/null; then
        fail "shellcheck is not installed. Install it with: apt-get install shellcheck"
    fi

    local version=$(shellcheck --version | head -1)
    log "  ✓ shellcheck available: ${version}"
}

# Test 2: Verify dr_backup.sh passes shellcheck
test_dr_backup_shellcheck() {
    log "Test 2: dr_backup.sh passes shellcheck"

    if ! shellcheck /workspaces/TTL-Legacy/scripts/dr_backup.sh; then
        fail "dr_backup.sh has shellcheck warnings"
    fi

    log "  ✓ dr_backup.sh passes shellcheck"
}

# Test 3: Verify dr_restore.sh passes shellcheck
test_dr_restore_shellcheck() {
    log "Test 3: dr_restore.sh passes shellcheck"

    if ! shellcheck /workspaces/TTL-Legacy/scripts/dr_restore.sh; then
        fail "dr_restore.sh has shellcheck warnings"
    fi

    log "  ✓ dr_restore.sh passes shellcheck"
}

# Test 4: Verify build.sh passes shellcheck
test_build_shellcheck() {
    log "Test 4: build.sh passes shellcheck"

    if ! shellcheck /workspaces/TTL-Legacy/scripts/build.sh; then
        fail "build.sh has shellcheck warnings"
    fi

    log "  ✓ build.sh passes shellcheck"
}

# Test 5: Verify deploy_utils.sh passes shellcheck
test_deploy_utils_shellcheck() {
    log "Test 5: deploy_utils.sh passes shellcheck"

    if ! shellcheck /workspaces/TTL-Legacy/scripts/deploy_utils.sh; then
        fail "deploy_utils.sh has shellcheck warnings"
    fi

    log "  ✓ deploy_utils.sh passes shellcheck"
}

# Test 6: Verify deploy_mainnet.sh passes shellcheck
test_deploy_mainnet_shellcheck() {
    log "Test 6: deploy_mainnet.sh passes shellcheck"

    if ! shellcheck /workspaces/TTL-Legacy/scripts/deploy_mainnet.sh; then
        fail "deploy_mainnet.sh has shellcheck warnings"
    fi

    log "  ✓ deploy_mainnet.sh passes shellcheck"
}

# Test 7: Verify all scripts in scripts directory pass shellcheck
test_all_scripts_shellcheck() {
    log "Test 7: All shell scripts pass shellcheck"

    local failed_scripts=()
    while IFS= read -r script; do
        if ! shellcheck "${script}" 2>/dev/null; then
            failed_scripts+=("${script}")
        fi
    done < <(find /workspaces/TTL-Legacy/scripts -type f -name "*.sh")

    if [[ ${#failed_scripts[@]} -gt 0 ]]; then
        fail "The following scripts have shellcheck warnings: ${failed_scripts[*]}"
    fi

    log "  ✓ All shell scripts pass shellcheck"
}

# Run all tests
log "Starting shellcheck test suite"

# Check if shellcheck is available
if ! command -v shellcheck &>/dev/null; then
    log "WARNING: shellcheck not installed, skipping tests"
    log "To run these tests, install shellcheck with: apt-get install shellcheck"
    exit 0
fi

test_shellcheck_available
test_dr_backup_shellcheck
test_dr_restore_shellcheck
test_build_shellcheck
test_deploy_utils_shellcheck
test_deploy_mainnet_shellcheck
test_all_scripts_shellcheck
log "All tests passed!"
