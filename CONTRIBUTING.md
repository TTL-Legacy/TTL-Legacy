# Contributing to TTL-Legacy

Thank you for contributing to TTL-Legacy!

## Getting Started

1. Fork the repository
2. Clone: `git clone https://github.com/YOUR_USERNAME/TTL-Legacy.git`
3. Create branch: `git checkout -b feature/your-feature-name`

## Branch Naming

- `feature/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation
- `test/` - Tests

## Commit Messages

Format: `<type>(#issue): Brief description`

Types: `feat`, `fix`, `test`, `docs`, `refactor`

## Pull Requests

**Before submitting:**
- Run: `cargo test --package ttl-vault`
- Check: `cargo fmt --all -- --check`
- Lint: `cargo clippy --package ttl-vault -- -D warnings`
- Audit: `cargo audit`

## Clippy Lint Policy

We use a shared Clippy configuration so lint rules apply consistently across all
workspace crates and match CI.

### Running Clippy Locally

```bash
# Primary contract (same as CI)
cargo clippy --package ttl-vault -- -D warnings

# Any other workspace crate
cargo clippy --package zk-verifier -- -D warnings
cargo clippy --package ttl-legacy-backend -- -D warnings
```

The `-D warnings` flag promotes all warnings (including `clippy::pedantic`) to
errors, matching CI behavior.

### Configuration Files

| File | Purpose |
|------|---------|
| `.clippy.toml` | Clippy thresholds (complexity, argument counts, etc.) |
| `Cargo.toml` `[workspace.lints.clippy]` | Lint levels, including `clippy::pedantic` and selective allows |

Every workspace member opts in via `[lints] workspace = true` in its `Cargo.toml`.

### Pedantic Lints

The workspace enables `clippy::pedantic` at the warn level. Pedantic lints catch
style and correctness issues beyond Clippy defaults. A set of **selective allows**
in the root `Cargo.toml` documents accepted patterns for Soroban contracts (for
example integer casts in ledger math, large contract APIs, and test helpers).

To propose a new allow, add it to `[workspace.lints.clippy]` with a brief comment
and explain the accepted pattern in your PR.

### CI Integration

CI runs `cargo clippy --package ttl-vault -- -D warnings` on every PR. Builds fail
if any Clippy warning remains after the shared configuration is applied.

## Security Audit Process

We use `cargo audit` to automatically detect and report security vulnerabilities in dependencies.

### Running Audit Locally

```bash
# Install cargo-audit
cargo install cargo-audit

# Run audit (fails on CRITICAL or HIGH severity)
cargo audit --deny warnings
```

### Audit Configuration

The audit configuration is managed in `.cargo/audit.toml`:

```toml
# Denies CI build on these severity levels
deny = ["unmaintained", "unsound"]

# Advisory allowlist with justifications
[advisories]
# "ADVISORY_ID" = { reason = "Justification" }
```

### Handling Vulnerabilities

1. **Immediate Patch**: If a CRITICAL or HIGH vulnerability is found, update the dependency immediately
2. **Minor Patch**: For MEDIUM vulnerabilities, plan an update in the next release cycle
3. **Accepted Advisories**: If a vulnerability cannot be fixed immediately, document the acceptance in `.cargo/audit.toml` with a clear justification

Example accepted advisory:

```toml
[advisories]
"RUSTSEC-2021-0001" = { reason = "Affects unused feature X; scheduled for removal in v2.0" }
```

### CI Integration

The CI pipeline runs `cargo audit` on every PR. Builds will fail if:
- Any CRITICAL or HIGH severity vulnerabilities are detected
- Accepted advisories lack proper justification

### Secret Scanning

We use Gitleaks in CI to prevent secrets from being committed in repository files or PR diffs.

- The workflow scans the repository on every push and pull request.
- Pull requests are scanned against the configured Gitleaks ruleset in `.gitleaks.toml`.
- Local developers can run the same check before pushing:

```bash
# Install gitleaks if needed
brew install gitleaks
# or: go install github.com/gitleaks/gitleaks/v8@latest

# Scan the repository
gitleaks detect --source . --config .gitleaks.toml --redact
```

If a false positive is encountered, add a narrow allowlist entry to `.gitleaks.toml` with a clear justification.



## Versioning Policy

We follow these versioning standards:

- **Contract ABI**: Semantic Versioning (SemVer)
- **Releases**: Calendar Versioning (CalVer)

All contract versions must be documented in `CHANGELOG.md`.

