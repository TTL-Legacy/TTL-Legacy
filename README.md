# TTL-Legacy — Micro-Endowment Check-In Vault on Stellar

[![CI](https://github.com/success-OG/TTL-Legacy/actions/workflows/ci.yml/badge.svg)](https://github.com/success-OG/TTL-Legacy/actions/workflows/ci.yml)

A decentralized "Dead Man's Switch" built on Stellar/Soroban smart contracts.

TTL-Legacy is a time-capsule vault where funds (XLM or tokenized assets) are released to a beneficiary only if the owner fails to "check in" via a Passkey-powered interface. It leverages Soroban's State Archival and TTL (Time to Live) features to automate asset inheritance — no seed phrase complexity required.

## 🎯 What is TTL-Legacy?

TTL-Legacy turns Stellar's native state archival mechanics into a programmable inheritance trigger. Vault owners:

- Deposit funds into a personal vault contract
- Periodically "check in" to extend the contract's TTL and prove liveness
- Designate a beneficiary address for automatic release
- Authenticate exclusively via Passkeys (WebAuthn) — no seed phrases

If the owner stops checking in, the contract's TTL expires and the vault automatically releases funds to the beneficiary.

This Soroban implementation makes TTL-Legacy:

✅ Trustless (no executor, lawyer, or coordinator needed)  
✅ Transparent (all vault state and transfers are on-chain)  
✅ Secure (Passkey/WebAuthn authentication, no exposed seed phrases)  
✅ Automated (TTL expiry triggers transfer without manual intervention)

## 🚀 Features

- **Create a Vault**: Set a beneficiary address and check-in interval
- **Check-In**: Extend the contract TTL to reset the countdown
- **Automatic Release**: Funds transfer to beneficiary when TTL lapses
- **Passkey Auth**: WebAuthn-based authentication for all owner actions
- **Reminder System**: Backend sends encrypted email/SMS check-in reminders
- **Legacy Dashboard**: Minimalist frontend to manage vault state and history
- **Native XLM Support**: Built-in support for Stellar Lumens
- **Token Ready**: Architecture supports custom Stellar tokens (roadmap item)

## 🛠️ Quick Start

### Prerequisites

- Rust (1.70+)
- Soroban CLI
- Stellar CLI

### Build

```bash
./scripts/build.sh
```

### Test

```bash
./scripts/test.sh
```

### Setup Environment

Copy the example environment file:

```bash
cp .env.example .env
```

Configure your environment variables in `.env`:

```env
# Network configuration
STELLAR_NETWORK=testnet
STELLAR_RPC_URL=https://soroban-testnet.stellar.org

# Contract addresses (after deployment)
CONTRACT_TTL_VAULT=<your-contract-id>

# Frontend configuration
VITE_STELLAR_NETWORK=testnet
VITE_STELLAR_RPC_URL=https://soroban-testnet.stellar.org

# Backend (reminder service)
REMINDER_EMAIL_API_KEY=<your-key>
REMINDER_SMS_API_KEY=<your-key>
```

Network configurations are defined in `environments.toml`:

- `testnet` — Stellar testnet
- `mainnet` — Stellar mainnet
- `futurenet` — Stellar futurenet
- `standalone` — Local development

### Deploy to Testnet

```bash
# Configure your testnet identity first
stellar keys generate deployer --network testnet

# Deploy
./scripts/deploy_testnet.sh
```

## 📖 Documentation

- [Architecture Overview](docs/architecture.md)
- [TTL & State Archival Logic](docs/ttl-logic.md)
- [Passkey Integration](docs/passkeys.md)
- [Threat Model & Security](docs/security.md)
- [Security Policy & Vulnerability Disclosure](SECURITY.md)
- [Roadmap](docs/roadmap.md)

## 🎓 Smart Contract API

### Vault Management

```rust
create_vault(beneficiary: Address, check_in_interval: u64) -> u64
get_vault(vault_id: u64) -> Vault
get_ttl_remaining(vault_id: u64) -> Option<u64>
```

### Owner Actions

```rust
check_in(vault_id: u64)
deposit(vault_id: u64, amount: i128)
withdraw(vault_id: u64, amount: i128)
update_beneficiary(vault_id: u64, new_beneficiary: Address)
```

### Release

```rust
trigger_release(vault_id: u64)
is_expired(vault_id: u64) -> bool
get_release_status(vault_id: u64) -> ReleaseStatus
```

## 🧪 Testing

Comprehensive test suite covering:

✅ Vault creation and configuration  
✅ Check-in flow and TTL extension  
✅ TTL expiry and automatic release  
✅ Passkey authentication validation  
✅ Beneficiary payout execution  
✅ Error handling and edge cases  

Run tests:

```bash
cargo test
```

## 🌍 Why This Matters

**The Problem**: Over $140 billion in crypto assets are estimated to be permanently lost due to inaccessible wallets. Traditional inheritance mechanisms don't map to self-custodied digital assets.

**Blockchain Benefits**:

- No trusted executor or legal intermediary required
- Transparent vault state and release history on-chain
- Programmable rules enforced by smart contracts
- Passkey auth removes the seed phrase single point of failure

**Target Users**:

- Long-term crypto holders planning for asset continuity
- Individuals without access to traditional estate planning
- Families and communities building generational wealth on-chain
- Anyone who wants a secure, automated digital legacy plan

## 🗺️ Roadmap

- **v1.0 (Current)**: XLM vaults, TTL-based release, Passkey auth
- **v1.1**: Custom token support (USDC, EURC, etc.)
- **v2.0**: Multi-beneficiary splits, conditional release logic
- **v3.0**: Mobile-friendly frontend, push notification reminders
- **v4.0**: Fiat on/off-ramps, legal document anchoring via Stellar

See [docs/roadmap.md](docs/roadmap.md) for details.

## 🤝 Contributing

We welcome contributions! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

See our [Code of Conduct](CODE_OF_CONDUCT.md) and [Contributing Guidelines](CONTRIBUTING.md).

## 📄 License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Stellar Development Foundation](https://stellar.org) for Soroban
- The WebAuthn/Passkey standards community
- Everyone building toward self-sovereign financial tools



