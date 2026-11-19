# Contributing to TTL-Legacy

We welcome contributions! Please follow these guidelines to help us maintain project quality.

## Getting Started

### Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | 1.70+ | Install via [rustup](https://rustup.rs) |
| Node.js | 18+ | Required for frontend dev tooling |
| Docker | 24+ | Required for local dev stack |
| Stellar CLI | latest | `cargo install stellar-cli --locked` |
| Soroban CLI | latest | Bundled with Stellar CLI |
| `just` | any | Optional but recommended — `cargo install just` |

### Local Dev Setup (Step by Step)

1. **Copy the environment file:**
   ```bash
   cp .env.example .env
   ```

2. **Start the local dev stack:**
   ```bash
   docker-compose up -d
   ```
   This starts PostgreSQL (port 5432), the backend (port 3000), and Stellar Quickstart (port 8000).
   Wait for health checks to pass before proceeding.

3. **Verify the backend is healthy:**
   ```bash
   curl http://localhost:3000/health
   ```
   You should receive a `200 OK` response.

4. **Build the Soroban contracts:**
   ```bash
   just build
   # or without just:
   ./scripts/build.sh
   ```

5. **Deploy to the local Stellar Quickstart node:**
   ```bash
   just deploy-testnet
   ```
   This deploys to the local node at `localhost:8000` as configured in `docker-compose.override.yml`.

6. **Start the frontend dev server:**
   ```bash
   just frontend-dev
   ```
   The frontend dev server runs at `http://localhost:5173`.

### Running the Backend Against the Local Node

In your `.env`, set:
```env
STELLAR_RPC_URL=http://localhost:8000
```

This points the backend at your local Stellar Quickstart instance instead of testnet.

## Troubleshooting

Common errors encountered when building the Soroban contracts, and how to fix them.

### `wasm32-unknown-unknown` target not found

**Symptom:** Building the contracts fails with an error such as:

```
error[E0463]: can't find crate for `core`
  = note: the `wasm32-unknown-unknown` target may not be installed
```

or

```
error: target `wasm32-unknown-unknown` not found in target list
```

**Cause:** The Soroban contracts are compiled to WebAssembly, which requires the
`wasm32-unknown-unknown` compilation target. A default `rustup` installation only
includes the host target, so the wasm target must be added explicitly.

**Fix:** Install the target with `rustup`:

```bash
rustup target add wasm32-unknown-unknown
```

If you use a pinned toolchain (e.g. via `rust-toolchain.toml`), add the target to
that toolchain specifically:

```bash
rustup target add wasm32-unknown-unknown --toolchain <toolchain>
```

Verify the target is installed:

```bash
rustup target list --installed
```

You should see `wasm32-unknown-unknown` in the output. Then re-run `just build`
(or `./scripts/build.sh`).

### `soroban-sdk` version mismatch

**Symptom:** The build fails with errors such as:

```
error: failed to select a version for the requirement `soroban-sdk = "^21.0.0"`
candidate versions found which didn't match: 20.0.0
```

or a type/trait mismatch between crates that depend on different `soroban-sdk`
versions (e.g. `expected soroban_sdk::Address, found soroban_sdk::Address`).

**Cause:** Cargo resolves a single version of `soroban-sdk` for the whole
dependency graph. If the workspace `Cargo.toml` and the individual contract
crates (e.g. `contracts/ttl_vault/Cargo.toml`) pin different versions, Cargo
either fails to resolve or silently links two incompatible copies of the SDK.

**Fix:** Align the `soroban-sdk` version everywhere it is declared:

1. **Workspace `Cargo.toml`** — define the version once under
   `[workspace.dependencies]`:
   ```toml
   [workspace.dependencies]
   soroban-sdk = "21.0.0"
   ```

2. **Each contract crate's `Cargo.toml`** — reference the workspace dependency
   instead of pinning a version locally:
   ```toml
   [dependencies]
   soroban-sdk = { workspace = true }
   ```

   If a crate must declare it directly, use the exact same version string as the
   workspace.

3. **Verify** that only one version is in the dependency graph:
   ```bash
   cargo tree -i soroban-sdk
   ```

   The output should show a single `soroban-sdk vX.Y.Z` entry. If multiple
   versions appear, update the offending `Cargo.toml` files and run
   `cargo update -p soroban-sdk` to refresh `Cargo.lock`.

Finally, rebuild the contracts with `just build` to confirm the fix.

## Development Workflow
=======
1. **Fork the repo** and create your branch: git checkout -b feature/your-feature-name.
2. **Formatting:** We use rustfmt. Please run the following command before committing:
   ```bash
   just fmt
   # or without just:
   cargo fmt
   ```
3. **Testing:** Run the test suite before submitting:
   ```bash
   just test
   # or without just:
   ./scripts/test.sh
   ```
4. **Pre-PR check:** Run the full CI suite locally before opening a PR:
   ```bash
   just ci
   ```
5. **Pull Requests:** Open a PR against main. Ensure your PR description clearly outlines the changes and links to the relevant issue.

## Stellar Wave Contributors

Contributors from the Stellar Wave program can pick up and complete issues following this process:

### Picking Up an Issue

1. **Check the issue labels:** Look for issues labeled with:
   - `good-first-issue` - Great for new contributors
   - `help-wanted` - Community contributions welcome
   - `stellar-wave` - Specifically for Stellar Wave participants
   
2. **Read the issue description:** Each issue has:
   - A clear description of what needs to be done
   - Estimated time to complete
   - Priority level (Low, Medium, High)
   - Category (e.g., Testing, Documentation, Feature, etc.)

3. **Comment on the issue:** Before starting work, comment on the issue to claim it:
   ```
   @ttl-legacy-bot claim
   ```
   This prevents duplicate work.

### Completing an Issue

1. **Create a feature branch:** Use the issue number and a descriptive name:
   ```bash
   git checkout -b feat/issue-XXXX-brief-description
   ```

2. **Follow the development workflow** (see above section)

3. **Link your PR to the issue:** In your PR description, include:
   ```
   Closes #XXXX
   ```
   Replace `XXXX` with the issue number. This automatically closes the issue when merged.

4. **PR Description Template:** Use this template:
   ```markdown
   ## Summary
   Brief description of what this PR does.

   ## Changes
   - Change 1
   - Change 2
   - Change 3

   ## Testing
   How to test these changes locally.

   Closes #XXXX
   ```

5. **Wait for review:** A maintainer will review your PR within 48 hours.

### Issue Categories & Guide

| Category | What to expect | PR checklist |
|----------|---|---|
| **Testing** | Add or improve test coverage | - Tests compile and pass locally<br>- New tests have clear assertions<br>- Run `just test` before submitting |
| **Documentation** | Add or update docs/guides | - Docs are clear and accurate<br>- Examples are tested<br>- Follow existing formatting |
| **Feature** | Implement new functionality | - All above + feature works end-to-end<br>- Add tests for new features<br>- Update relevant docs |
| **Bug Fix** | Resolve reported bugs | - Root cause identified<br>- Fix is minimal and targeted<br>- Add regression test |

## Automated Security Scanning in CI

The CI pipeline includes three layers of automated security scanning that run on every push and pull request:

### 1. **Gitleaks (Secret Detection)**
Gitleaks scans for accidentally committed secrets (API keys, tokens, credentials, etc.) in the git history and working directory.

- **Configuration:** `.gitleaks.toml` contains the detection rules and allowlist for test fixtures
- **False positives:** If legitimate test credentials or documentation examples trigger a false positive, add them to the `allowlist.regexes` or `allowlist.paths` in `.gitleaks.toml`
- **Local testing:** Run gitleaks locally before pushing:
  ```bash
  gitleaks detect --source . --config .gitleaks.toml --redact --fail
  ```

### 2. **Cargo Audit (Vulnerability Advisories)**
Cargo audit checks Rust dependencies against a database of known security vulnerabilities.

- **Runs:** `cargo audit --deny warnings`
- **What it catches:** Known CVEs in transitive dependencies
- **If audit fails:** Update the vulnerable dependency to a patched version, or if no fix is available, document the exception and work with the security team
- **Local testing:**
  ```bash
  cargo audit --deny warnings
  ```

### 3. **Clippy Linting (Static Analysis)**
Clippy enforces lint warnings as hard errors to catch common mistakes and anti-patterns.

- **Runs:** `cargo clippy --package ttl-vault -- -D warnings`
- **Warnings treated as errors:** All clippy warnings must be fixed before merging
- **Suppress spurious warnings:** Use `#[allow(clippy::rule_name)]` on specific code with a comment explaining why
- **Local testing:**
  ```bash
  cargo clippy --package ttl-vault -- -D warnings
  ```

### Running Security Checks Locally

Before opening a PR, run all security checks locally:

```bash
# Run the full CI suite including all security scans
just ci

# Or manually:
cargo fmt --all -- --check
cargo clippy --package ttl-vault -- -D warnings
cargo audit --deny warnings
gitleaks detect --source . --config .gitleaks.toml --redact --fail
```

If any check fails, fix it before pushing. The CI pipeline will enforce the same checks.

## Available `just` Targets

Install [just](https://just.systems/man/en/packages.html), then run `just --list` from the repo root:

```
Available recipes:
    audit               # Run cargo-audit (install with: cargo install cargo-audit)
    build               # Build both Soroban contracts for wasm32 release
    ci                  # Run build + test + clippy in one shot (useful before opening a PR)
    clippy              # Run clippy (warnings treated as errors, matching CI)
    deploy-mainnet      # Deploy to Stellar mainnet (requires STELLAR_MAINNET_RPC_URL; prompts for confirmation)
    deploy-mainnet-force# Force-redeploy to mainnet without the existing-contract prompt
    deploy-testnet      # Deploy to Stellar testnet (prompts if a contract already exists)
    deploy-testnet-force# Force-redeploy to testnet without confirmation prompt
    docker-down         # Stop and remove local dev stack containers
    docker-up           # Start local dev stack (PostgreSQL, backend, Stellar Quickstart)
    dr-backup           # Run disaster-recovery backup
    env-setup           # Copy .env.example to .env (skips if .env already exists)
    fmt                 # Auto-format all code
    fmt-check           # Check code formatting
    frontend-dev        # Start frontend development server
    frontend-test       # Run frontend tests
    test                # Run the full ttl_vault test suite
```

### Frontend Development

The `frontend-dev` and `frontend-test` commands simplify frontend work:

```bash
# Start the frontend dev server at http://localhost:5173
just frontend-dev

# Run the frontend test suite
just frontend-test
```

These commands automatically install dependencies and run the appropriate npm scripts.

### Disaster Recovery

The `dr-backup` command runs the disaster-recovery backup script:

```bash
# Trigger a backup to the configured S3 bucket
just dr-backup
```

See [RECOVERY_IMPLEMENTATION.md](RECOVERY_IMPLEMENTATION.md) for more details on disaster recovery procedures.

## Fuzz Testing

The TTL Vault contract includes comprehensive fuzz testing to catch panics, unexpected errors, and boundary condition violations. Fuzz tests are located in `contracts/ttl_vault/fuzz/`.

### Running Fuzz Tests Locally

Fuzz testing requires the nightly Rust toolchain:

```bash
# Install nightly if you haven't already
rustup install nightly
rustup component add rust-src --toolchain nightly
```

Run a single fuzz target:

```bash
cd contracts/ttl_vault/fuzz
# Fuzz for 10 minutes (600 seconds)
cargo +nightly fuzz run fuzz_create_vault -- -max_total_time=600

# Fuzz with corpus (if available)
cargo +nightly fuzz run fuzz_deposit corpus/fuzz_deposit -- -max_total_time=600
```

Run all fuzz targets:

```bash
# Run each target for 10 minutes
for target in fuzz_vesting fuzz_create_vault fuzz_deposit fuzz_withdraw fuzz_check_in; do
  cargo +nightly fuzz run $target -- -max_total_time=600
done
```

### Fuzz Targets

Four primary contract entry points have fuzz targets:

- **`fuzz_create_vault`** - Tests vault creation with arbitrary addresses and intervals
- **`fuzz_deposit`** - Tests deposits with arbitrary amounts and limits
- **`fuzz_withdraw`** - Tests withdrawals with approval thresholds and guards
- **`fuzz_check_in`** - Tests check-ins with TTL caps and inactivity penalties
- **`fuzz_vesting`** - Tests vesting schedule calculations (existing)

### Handling Fuzz Findings

If the fuzzer finds a crash or panic:

1. **Reproduce locally**: The fuzzer creates a corpus file with the failing input
2. **Minimize**: Use `cargo +nightly fuzz cmin <target>` to reduce input size
3. **Fix**: Address the underlying issue in the contract code
4. **Verify**: Re-run the fuzzer to confirm the fix
5. **Commit**: Add the test case to prevent regressions

Example:
```bash
# If fuzz_create_vault crashes
cd contracts/ttl_vault/fuzz
# Reproduce the crash
cargo +nightly fuzz run fuzz_create_vault corpus/fuzz_create_vault/crash-xxx
# Minimize the input
cargo +nightly fuzz cmin fuzz_create_vault corpus/fuzz_create_vault/crash-xxx
```

### CI Fuzz Testing

Fuzz tests run nightly via `.github/workflows/nightly-fuzz.yml`:

- **Schedule**: Daily at 2 AM UTC
- **Duration**: 10 minutes per target (configurable)
- **Failure Mode**: Any crash/panic causes CI failure
- **Artifacts**: Corpus and crash artifacts uploaded for 30 days

Manual trigger:
```bash
# Trigger via GitHub CLI with custom parameters
gh workflow run nightly-fuzz.yml -f fuzz_time=1800 -f max_len=2048
```

### Corpus Management

Seed corpus files are stored in `contracts/ttl_vault/fuzz/corpus/`:

- `fuzz_create_vault/` - Seeds for vault creation testing
- `fuzz_deposit/` - Seeds for deposit testing
- `fuzz_withdraw/` - Seeds for withdrawal testing
- `fuzz_check_in/` - Seeds for check-in testing

Generate or regenerate corpus:
```bash
cd contracts/ttl_vault/fuzz
python3 generate_corpus.py
```

After fuzzing discovers new interesting inputs, commit them:
```bash
git add contracts/ttl_vault/fuzz/corpus/
git commit -m "Update fuzz corpus after extended fuzzing run"
```

### Best Practices

1. **Run fuzz tests before major releases** - Extended runs (hours/days) catch subtle bugs
2. **Commit crash inputs** - Prevents regressions
3. **Review fuzzer findings** - Understand why an input triggered coverage
4. **Maintain corpus** - Keep seed inputs diverse and minimal

For more details, see [contracts/ttl_vault/fuzz/README.md](contracts/ttl_vault/fuzz/README.md).

## Conventional Commits

TTL-Legacy enforces the [Conventional Commits](https://www.conventionalcommits.org/) specification.
All **PR titles** and **squash-merge commit messages** must follow this format:

```
<type>(<optional scope>): <short description>

[optional body]

[optional footer(s)]
```

### Allowed Types

| Type       | When to use                                          |
|------------|------------------------------------------------------|
| `feat`     | A new feature                                        |
| `fix`      | A bug fix                                            |
| `docs`     | Documentation-only changes                           |
| `refactor` | Code change that is neither a fix nor a feature      |
| `perf`     | Performance improvement                              |
| `test`     | Adding or updating tests                             |
| `build`    | Changes to the build system or external dependencies |
| `ci`       | CI/CD configuration changes                          |
| `chore`    | Other changes that do not modify src or test files   |
| `revert`   | Reverts a previous commit                            |
| `security` | Security fix or hardening                            |

### Examples

```
feat(vault): add batch check-in endpoint
fix(check-in): return VaultNotFound instead of raw panic string
docs(contributing): add conventional commit guide
security(auth): enforce passkey nonce replay protection
ci(release): add git-cliff changelog automation
refactor(backend): extract sanitization middleware
```

### Why?

Commit messages are parsed by [git-cliff](https://git-cliff.org/) to auto-generate
`CHANGELOG.md` on every release.  A CI job (`commitlint.yml`) validates PR titles
against these rules on every pull request.

## Style Guide
- Follow standard Rust idiomatic practices.
- Use /// for all public function documentation.
- Maintain consistency with the existing project structure.
