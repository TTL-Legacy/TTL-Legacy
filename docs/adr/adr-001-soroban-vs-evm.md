# ADR-001: Soroban vs. EVM for Smart Contracts

## Status

Accepted (retroactive)

## Date

2024-01-15

## Context

The project requires on-chain smart-contract logic to implement time-locked vaults, beneficiary management, and native asset handling on the Stellar network. The core requirements driving the contract platform choice were:

- **TTL-based vault logic**: Vaults must expire or unlock based on ledger time, requiring deterministic, ledger-aware time semantics.
- **Native Stellar asset handling**: Contracts must interact directly with Stellar's native assets (XLM and issued assets) without wrapping or bridging overhead.
- **Security and auditability**: The contract platform must offer a strong security model suitable for funds custody.
- **Ecosystem fit**: The solution should align with the Stellar network where the project operates.

This ADR is recorded retroactively to document the historical rationale behind selecting Soroban over alternative smart-contract platforms. For the broader technology-stack rationale, see [docs/architecture.md](../architecture.md).

## Decision

Use **Soroban** (Stellar's smart-contract platform) with contracts written in **Rust**, deployed on the Stellar network.

## Alternatives Considered

### Ethereum / Solidity (EVM)

The most mature smart-contract ecosystem, with extensive tooling, a large developer community, and deep liquidity. However, it operates on a separate chain from Stellar, requiring bridges or wrapped assets to interact with Stellar-native assets, and introduces cross-chain trust assumptions.

### CosmWasm

A WebAssembly-based smart-contract platform used across the Cosmos ecosystem. It offers a strong security model and Rust-based development, but it targets Cosmos chains rather than Stellar, and does not provide native integration with Stellar assets or ledger semantics.

## Trade-offs Evaluated

| Criterion | Soroban | Ethereum / Solidity | CosmWasm |
|-----------|---------|---------------------|----------|
| **Tooling maturity** | Younger, evolving tooling | Most mature | Mature within Cosmos |
| **Ecosystem and liquidity** | Native to Stellar | Largest ecosystem/liquidity | Cosmos ecosystem |
| **Transaction cost** | Low, Stellar fee model | Variable, often high (gas) | Low-to-moderate |
| **Security model** | Rust memory safety, audited runtime | Battle-tested but complex | Rust memory safety |
| **Native Stellar asset integration** | First-class, direct | Requires bridges/wrapping | Not native |
| **Storage / TTL model** | Native TTL and ledger-entry model | Account/storage model differs | Chain-dependent |
| **Developer familiarity** | Rust (growing) | Solidity (widely known) | Rust (growing) |

Key considerations:

- **Native Stellar asset integration** and **storage/TTL model** were decisive: Soroban provides direct access to Stellar assets and a ledger-entry TTL model that maps naturally onto time-locked vault semantics.
- **Transaction cost** on Stellar is low and predictable, which matters for vault operations.
- **Security model**: Rust's memory safety and Soroban's audited runtime reduce classes of vulnerabilities.
- **Tooling maturity** and **developer familiarity** were acknowledged weaknesses of Soroban relative to Ethereum, accepted as the cost of native integration.

## Consequences

### Positive

- Direct, first-class handling of native Stellar assets without bridges or wrapping.
- Native TTL and ledger-time semantics that align with vault expiry logic.
- Low, predictable transaction costs.
- Rust's memory safety and Soroban's security model reduce common vulnerability classes.
- Operates natively on the Stellar network, matching the project's ecosystem.

### Negative

- Soroban tooling is younger and less mature than Ethereum's, requiring more manual work and adaptation.
- Smaller developer pool familiar with Soroban compared to Solidity.
- Fewer third-party libraries and reference implementations available.
- Ecosystem and liquidity are smaller than Ethereum's, limiting certain integrations.

## Outcome

Soroban was adopted as the smart-contract platform. The decision prioritized native Stellar asset integration, TTL-based vault semantics, low transaction costs, and a strong security model over the tooling maturity and ecosystem size advantages of Ethereum/Solidity and CosmWasm. The trade-offs were accepted as appropriate for a Stellar-native funds-custody application.

For the full technology-stack rationale, see [docs/architecture.md](../architecture.md).