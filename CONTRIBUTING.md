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

## Security

Report vulnerabilities via [Security Policy](SECURITY.md).

## License

Contributions are licensed under MIT License.
