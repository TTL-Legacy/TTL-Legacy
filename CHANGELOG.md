# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Bug Fixes

- **check-in**: Return structured `ContractError::VaultNotFound` instead of panicking with a raw string on non-existent vault ([#1263](https://github.com/OxDev-max/TTL-Legacy/issues/1263))

### Features

- **ci**: Add git-cliff changelog automation with `cliff.toml` and `release.yml` workflow ([#1197](https://github.com/OxDev-max/TTL-Legacy/issues/1197))
- **ci**: Enforce conventional commit format via commitlint on pull request titles ([#1197](https://github.com/OxDev-max/TTL-Legacy/issues/1197))
- **frontend**: Add React `ErrorBoundary` component with fallback UI and retry button to dashboard ([#1198](https://github.com/OxDev-max/TTL-Legacy/issues/1198))
- **backend**: Implement request input sanitization middleware — 64 KB body limit, 512-character field limit ([#1199](https://github.com/OxDev-max/TTL-Legacy/issues/1199))

### Documentation

- **contributing**: Add conventional commit format requirement and examples ([#1197](https://github.com/OxDev-max/TTL-Legacy/issues/1197))
- **backend-api**: Document request size and field-length limits ([#1199](https://github.com/OxDev-max/TTL-Legacy/issues/1199))

## [1.0.0] - 2026-06-30

### Features

- Initial release of TTL-Legacy contract
- XLM vaults with TTL-based automatic release to beneficiary
- Passkey/WebAuthn authentication for all owner actions — no seed phrases required
- Beneficiary conditional acceptance with minimum threshold
- Beneficiary conflict resolution with automated adjudication
- Withdrawal audit trail, batching, notifications, and 24-hour dispute window
- Vesting schedules with cliff, milestone, and catch-up support
- Multi-signature approval for high-value operations
- Vault hibernation mode to pause the TTL countdown
- Reminder service (email/SMS) for upcoming check-in deadlines
- Backend Axum REST API for reminder preferences and notifications
- OpenTelemetry distributed tracing integration
- Comprehensive fuzz test suite for core entry points
- Versioning policy and CHANGELOG.md
