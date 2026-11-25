#!/usr/bin/env bash
# README_test.sh — Test suite for mobile/README.md documentation
#
# Tests:
#   1. Verify mobile/README.md exists
#   2. Verify README contains toolchain versions documentation
#   3. Verify README contains local backend setup documentation
#   4. Verify README contains test running documentation

set -euo pipefail

log() { echo "[mobile_readme_test] $*"; }
fail() { echo "[mobile_readme_test] FAIL: $*" >&2; exit 1; }

# Test 1: Verify mobile/README.md exists
test_readme_exists() {
    log "Test 1: mobile/README.md exists"

    if [[ ! -f "/workspaces/TTL-Legacy/mobile/README.md" ]]; then
        fail "mobile/README.md does not exist"
    fi

    log "  ✓ mobile/README.md exists"
}

# Test 2: Verify README contains toolchain versions section
test_toolchain_documentation() {
    log "Test 2: README contains toolchain versions documentation"

    local readme="/workspaces/TTL-Legacy/mobile/README.md"

    # Check for toolchain/version related content
    if ! grep -qi "toolchain\|node\|npm\|ruby\|swift\|kotlin\|android\|ios\|version" "${readme}"; then
        fail "README does not document toolchain versions"
    fi

    log "  ✓ Toolchain documentation present"
}

# Test 3: Verify README contains local backend setup documentation
test_local_backend_documentation() {
    log "Test 3: README contains local backend setup documentation"

    local readme="/workspaces/TTL-Legacy/mobile/README.md"

    # Check for local backend related content
    if ! grep -qi "backend\|local\|running\|setup\|start" "${readme}"; then
        fail "README does not document local backend setup"
    fi

    log "  ✓ Local backend setup documentation present"
}

# Test 4: Verify README contains test running documentation
test_test_documentation() {
    log "Test 4: README contains test running documentation"

    local readme="/workspaces/TTL-Legacy/mobile/README.md"

    # Check for test related content
    if ! grep -qi "test\|testing\|run.*test" "${readme}"; then
        fail "README does not document how to run tests"
    fi

    log "  ✓ Test documentation present"
}

# Test 5: Verify README has build documentation
test_build_documentation() {
    log "Test 5: README contains build documentation"

    local readme="/workspaces/TTL-Legacy/mobile/README.md"

    # Check for build related content
    if ! grep -qi "build\|compile\|develop" "${readme}"; then
        fail "README does not document how to build the app"
    fi

    log "  ✓ Build documentation present"
}

# Test 6: Verify README is not empty
test_readme_content() {
    log "Test 6: README has substantial content"

    local readme="/workspaces/TTL-Legacy/mobile/README.md"
    local line_count=$(wc -l < "${readme}")

    if [[ "${line_count}" -lt 10 ]]; then
        fail "README is too short (${line_count} lines), expected at least 10 lines"
    fi

    log "  ✓ README has sufficient content (${line_count} lines)"
}

# Run all tests
log "Starting mobile/README.md test suite"
test_readme_exists
test_toolchain_documentation
test_local_backend_documentation
test_test_documentation
test_build_documentation
test_readme_content
log "All tests passed!"
