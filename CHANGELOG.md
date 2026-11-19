# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- This file is automatically updated by git-cliff on each release.
     See cliff.toml for configuration. -->

## [Unreleased]

### Bug Fixes
- **check-in**: return structured `ContractError::VaultNotFound` instead of panicking with a raw string on non-existent vault (#1263)

### Features
- **ci**: add git-cliff changelog automation with `cliff.toml` and `release.yml` workflow (#1197)
- **ci**: enforce conventional commit format via commitlint on pull request titles (#1197)
- **frontend**: add React `ErrorBoundary` component with fallback UI and retry button to dashboard (#1198)
- **backend**: implement request input sanitization middleware — 64 KB body limit, 512-character field limit (#1199)

### Documentation
- **contributing**: add conventional commit format requirement and examples (#1197)
- **backend-api**: document request size and field-length limits (#1199)

## [1.0.0] - 2026-06-30

### Added
- Initial release of TTL-Legacy contract.
- XLM vaults with TTL-based automatic release to beneficiary.
- Passkey/WebAuthn authentication for all owner actions — no seed phrases required.
- Beneficiary conditional acceptance with minimum threshold.
- Beneficiary conflict resolution with automated adjudication.
- Withdrawal audit trail, batching, notifications, and 24-hour dispute window.
- Vesting schedules with cliff, milestone, and catch-up support.
- Multi-signature approval for high-value operations.
- Vault hibernation mode to pause the TTL countdown.
- Reminder service (email/SMS) for upcoming check-in deadlines.
- Backend Axum REST API for reminder preferences and notifications.
- OpenTelemetry distributed tracing integration.
- Comprehensive fuzz test suite for core entry points.
- Added versioning policy and CHANGELOG.md.
