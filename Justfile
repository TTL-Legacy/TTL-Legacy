# TTL-Legacy Justfile
# Run `just` or `just --list` to see all available targets.
# Install just: https://just.systems/man/en/packages.html

# Default: list all targets
default:
    @just --list

# ── Build ─────────────────────────────────────────────────────────────────────

# Build both Soroban contracts for wasm32 release
build:
    @echo "Building TTL-Legacy contracts..."
    cargo build --target wasm32-unknown-unknown --release --manifest-path contracts/ttl_vault/Cargo.toml
    cargo build --target wasm32-unknown-unknown --release --manifest-path contracts/zk_verifier/Cargo.toml
    @echo "Build complete."

# ── Test ──────────────────────────────────────────────────────────────────────

# Run the full ttl_vault test suite
test:
    @echo "Running TTL-Legacy tests..."
    cargo test --manifest-path contracts/ttl_vault/Cargo.toml
    @echo "All tests passed."

# ── Lint / Quality ────────────────────────────────────────────────────────────

# Run clippy (warnings treated as errors, matching CI)
clippy:
    cargo clippy --package ttl-vault -- -D warnings

# Check code formatting
fmt-check:
    cargo fmt --all -- --check

# Auto-format all code
fmt:
    cargo fmt --all

# ── Security ──────────────────────────────────────────────────────────────────

# Run cargo-audit (install with: cargo install cargo-audit)
audit:
    cargo audit --deny warnings

# Audit all `pub fn` definitions for require_auth / admin-check coverage (issue #1507)
auth-audit:
    @echo "Auditing admin-only functions for require_auth coverage..."
    bash scripts/audit_auth.sh

# ── Deploy ────────────────────────────────────────────────────────────────────

# Deploy to Stellar testnet (prompts if a contract already exists)
deploy-testnet:
    ./scripts/deploy_testnet.sh

# Force-redeploy to testnet without confirmation prompt
deploy-testnet-force:
    ./scripts/deploy_testnet.sh --force

# Deploy to Stellar mainnet (requires STELLAR_MAINNET_RPC_URL; prompts for confirmation)
deploy-mainnet:
    ./scripts/deploy_mainnet.sh

# Force-redeploy to mainnet without the existing-contract prompt
deploy-mainnet-force:
    ./scripts/deploy_mainnet.sh --force

# ── Docker ────────────────────────────────────────────────────────────────────

# Start local dev stack (PostgreSQL, backend, Stellar Quickstart)
docker-up:
    docker-compose up -d

# Stop and remove local dev stack containers
docker-down:
    docker-compose down

# ── Frontend ──────────────────────────────────────────────────────────────────

# Start frontend development server
frontend-dev:
    @if [ ! -d "web" ]; then \
        echo "Frontend directory not found"; \
        exit 1; \
    fi
    cd web && npm install && npm run dev

# Run frontend tests
frontend-test:
    @if [ ! -d "web" ]; then \
        echo "Frontend directory not found"; \
        exit 1; \
    fi
    cd web && npm install && npm run test

# ── Disaster Recovery ─────────────────────────────────────────────────────────

# Run disaster-recovery backup
dr-backup:
    @echo "Running disaster-recovery backup..."
    bash scripts/dr_backup.sh

# ── Helpers ───────────────────────────────────────────────────────────────────

# Run build + test + clippy in one shot (useful before opening a PR)
ci: build test clippy

# Copy .env.example to .env (skips if .env already exists)
env-setup:
    @if [ -f .env ]; then \
        echo ".env already exists — skipping."; \
    else \
        cp .env.example .env; \
        echo "Created .env from .env.example. Fill in your values."; \
    fi
