# Contributing to TTL-Legacy

We welcome contributions! Please follow these guidelines to help us maintain project quality.

## Development Workflow

1. **Fork the repo** and create your branch: `git checkout -b feature/your-feature-name`.
2. **Formatting:** We use rustfmt. Please run the following command before committing:
   ```bash
   cargo fmt
   ```
3. **Testing:** Run the test suite before submitting:
   ```bash
   ./scripts/test.sh
   ```
4. **Pull Requests:** Open a PR against main. Ensure your PR description clearly outlines the changes and links to the relevant issue.

## Using Just Command Runner

This project uses [just](https://github.com/casey/just) as a command runner to simplify common development tasks. All build, test, and deployment commands are centralized in the `Justfile`.

### Available Commands

Run `just --list` to see all available commands:

```bash
$ just --list

Available recipes:
    build          # Build all contracts (WASM)
    test           # Run tests
    clippy         # Run clippy linter
    audit          # Run cargo audit for security vulnerabilities
    fmt            # Format code with rustfmt
    fmt-check      # Check code formatting without modifying files
    deploy-testnet # Deploy to testnet
    deploy-mainnet # Deploy to mainnet (requires confirmation)
    docker-up      # Start Docker Compose services
    docker-down    # Stop Docker Compose services
    clean          # Clean build artifacts
    check          # Run all checks (format, clippy, test)
    dev            # Quick development workflow: build and test
    default        # Shows available commands
```

### Quick Start with Just

```bash
# Install just (if not already installed)
# macOS: brew install just
# Ubuntu/Debian: sudo apt install just
# Windows: cargo install just

# Build contracts
just build

# Run tests
just test

# Run clippy linter
just clippy

# Run security audit
just audit

# Start Docker services
just docker-up

# Stop Docker services
just docker-down

# Run all checks (format, clippy, test)
just check
```

## Style Guide

- Follow standard Rust idiomatic practices.
- Use /// for all public function documentation.
- Maintain consistency with the existing project structure.
