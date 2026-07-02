# TTL-Legacy Development Task Runner
# Documentation: https://github.com/casey/just

# Set the default shell to bash for cross-platform compatibility
set shell := ["bash", "-c"]

# Default target - shows available commands
default:
    @just --list

# Build all contracts (WASM)
build:
    @echo "Building TTL-Legacy contracts..."
    cargo build --target wasm32-unknown-unknown --release --manifest-path contracts/ttl_vault/Cargo.toml
    cargo build --target wasm32-unknown-unknown --release --manifest-path contracts/zk_verifier/Cargo.toml
    @echo "✓ Build complete."

# Run tests
test:
    @echo "Running TTL-Legacy tests..."
    cargo test --manifest-path contracts/ttl_vault/Cargo.toml
    @echo "✓ All tests passed."

# Run clippy linter
clippy:
    @echo "Running clippy linter..."
    cargo clippy --all-targets --all-features -- -D warnings
    @echo "✓ Clippy check passed."

# Run cargo audit for security vulnerabilities
audit:
    @echo "Running security audit..."
    cargo audit
    @echo "✓ Security audit complete."

# Format code with rustfmt
fmt:
    @echo "Formatting code..."
    cargo fmt --all
    @echo "✓ Code formatted."

# Check code formatting without modifying files
fmt-check:
    @echo "Checking code formatting..."
    cargo fmt --all -- --check

# Deploy to testnet
deploy-testnet:
    @echo "Deploying TTL-Legacy to testnet..."
    ./scripts/deploy_testnet.sh

# Deploy to mainnet (requires confirmation)
deploy-mainnet:
    @echo "Deploying TTL-Legacy to mainnet..."
    ./scripts/deploy_mainnet.sh

# Start Docker Compose services
docker-up:
	@echo "Starting Docker Compose services..."
	docker-compose up -d
	@echo "✓ Docker services started."
	@echo "  - PostgreSQL: localhost:5432"
	@echo "  - Backend: localhost:3000"
	@echo "  - Stellar Quickstart: localhost:8000"

# Stop Docker Compose services
docker-down:
	@echo "Stopping Docker Compose services..."
	docker-compose down
	@echo "✓ Docker services stopped."

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	rm -rf target/
	@echo "✓ Clean complete."

# Run all checks (format, clippy, test)
check: fmt-check clippy test
	@echo "✓ All checks passed."

# Quick development workflow: build and test
dev: build test
	@echo "✓ Build and test complete."