# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Features

- **ci**: Add git-cliff changelog automation with `cliff.toml` and `release.yml` workflow (#1197) ([6df237a](https://github.com/OxDev-max/TTL-Legacy/commit/6df237a1c3952a2eb98fecb22a15434b6fb6d2e0))
- **ci**: Enforce conventional commit format via commitlint on pull request titles (#1197) ([6df237a](https://github.com/OxDev-max/TTL-Legacy/commit/6df237a1c3952a2eb98fecb22a15434b6fb6d2e0))
- **frontend**: Add React `ErrorBoundary` component with fallback UI and retry button to dashboard (#1198) ([6df237a](https://github.com/OxDev-max/TTL-Legacy/commit/6df237a1c3952a2eb98fecb22a15434b6fb6d2e0))
- **backend**: Implement request input sanitization middleware — 64 KB body limit, 512-character field limit (#1199) ([6df237a](https://github.com/OxDev-max/TTL-Legacy/commit/6df237a1c3952a2eb98fecb22a15434b6fb6d2e0))

### Bug Fixes

- **check-in**: Return structured `ContractError::VaultNotFound` instead of panicking with a raw string on non-existent vault (#1263) ([6df237a](https://github.com/OxDev-max/TTL-Legacy/commit/6df237a1c3952a2eb98fecb22a15434b6fb6d2e0))

### Documentation

- **contributing**: Add conventional commit format requirement and examples (#1197) ([6df237a](https://github.com/OxDev-max/TTL-Legacy/commit/6df237a1c3952a2eb98fecb22a15434b6fb6d2e0))
- **backend-api**: Document request size and field-length limits (#1199) ([6df237a](https://github.com/OxDev-max/TTL-Legacy/commit/6df237a1c3952a2eb98fecb22a15434b6fb6d2e0))

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
