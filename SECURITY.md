# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability, please report it responsibly by opening a private security advisory or contacting the maintainers directly. Do not disclose vulnerabilities publicly until they have been addressed.

## Admin Function Auth Audit

All admin-only entrypoints in the vault contract must enforce authorization via `require_auth` (or an equivalent admin check) before mutating state. To keep this guarantee from regressing as the contract grows, the repository ships an audit script that scans the contract sources and reports the auth coverage of every public function.

### Running the audit

```sh
./scripts/audit_admin_auth.sh
```

The script:

1. Lists every `pub fn` defined under `contracts/`.
2. Inspects each function body for an auth check (`require_auth`, `require_admin`, or an admin guard).
3. Flags any function whose name indicates an admin-only path (e.g. `set_`, `update_`, `pause`, `unpause`, `upgrade`, `withdraw`, `mint`, `burn`, `transfer_admin`, `initialize`) but that is missing an auth check.
4. Exits non-zero when a missing check is found, so it can gate CI.

### CI enforcement

The audit runs automatically on every pull request and push to `main` via the `security-audit` job. A failing audit blocks the merge until the missing `require_auth` call is added.

### Adding a new admin function

When adding an admin-only entrypoint:

- Call `require_auth` on the stored admin address (or the caller) before any state mutation.
- Add a test asserting that a non-admin caller fails with the expected authorization error.
- Run `./scripts/audit_admin_auth.sh` locally to confirm the function is recognized as covered.
