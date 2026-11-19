# ADR: Beneficiary privacy model and commitment/reveal tradeoff

- Status: Accepted
- Date: 2026-08-29

## Context

TTL-Legacy is designed for public blockchain execution on Stellar. Every contract state entry is readable by anyone with access to the network, so a plaintext beneficiary address stored in a vault is not private by default. This exposes the beneficiary identity before release, which creates a phishing and social-engineering risk even though the system remains transparent and auditable.

The repository included a privacy-oriented API surface (`commit_beneficiary` / `reveal_beneficiary`) but the state model and storage keys were incomplete, leaving the contract without a clear privacy story. The system therefore needed an explicit design decision that acknowledged the public-state reality and documented the recommended mitigations.

## Decision

We will keep the existing plaintext beneficiary field for backwards compatibility and query ergonomics, but we will formalize an explicit privacy layer:

1. Plaintext beneficiary storage remains the default public representation for legacy compatibility.
2. A vault may optionally store a `BeneficiaryCommitment` as a SHA-256 hash of the raw beneficiary address bytes.
3. The commitment is revealed only at release time, when `reveal_beneficiary` verifies the proof against the stored hash before transferring funds.
4. The contract records a `RevealedBeneficiary` entry once the reveal succeeds, preventing replay and repeated reveals.
5. The design is opt-in and best-effort; it does not promise full anonymity, because any release or transfer ultimately exposes the recipient address.

## Consequences

### Positive

- Reduces pre-release targeting exposure for social engineering attacks.
- Preserves compatibility with existing clients and dashboards that read the public beneficiary field.
- Gives owners a privacy-forward path without forcing a breaking migration.
- Provides an explicit verify-and-reveal flow with a hash proof to avoid accidental misdelivery.

### Negative

- The default state remains public and discoverable on-chain.
- The system is not fully private: the final beneficiary becomes visible when the vault releases.
- Users must understand that privacy is provided only by the commitment/reveal path, not by hiding data in a public ledger.

## Alternatives considered

### 1. Keep only plaintext beneficiary storage

This was the simplest implementation but left the issue unresolved and failed to document the privacy tradeoff.

### 2. Remove plaintext storage entirely

This would provide stronger privacy but would break compatibility with all existing queries, dashboards, and integrations that rely on the public beneficiary field.

### 3. Commit-hash-only model with no plaintext compatibility

This is more private but would require a breaking migration and significant client-side changes. It is not compatible with the current contract's public indexing model.

## Notes

The commitment model is a mitigation, not a complete anonymity layer. It is suitable when users want to protect against casual targeting before release but accept that the final payout address is publicly visible at the point of distribution.
