# Threat Model & Security

## Document Control

| Field | Value |
|---|---|
| Scope | TTL-Legacy Soroban vault contract and its owner/beneficiary interactions |
| Version | 1.0 |
| Last reviewed | 2026-08-28 |
| Status | Pre-audit; external audit pending |
| Related control document | [Security Audit Checklist](security-audit-checklist.md) |

This document records the security assumptions, protected assets, threat scenarios, controls, and residual risks for the TTL-Legacy vault. It is a design-level threat model, not an assertion that the implementation has passed an independent audit.

## 1. Scope and Security Objectives

TTL-Legacy holds XLM or supported Stellar assets in time-locked vaults. An owner periodically checks in; if the check-in deadline expires, an eligible beneficiary may trigger release according to the contract rules.

The security objectives are to:

1. Prevent unauthorized creation, modification, withdrawal, or release of vault funds.
2. Preserve the integrity of owner, beneficiary, balance, TTL, pause, and release state.
3. Ensure release cannot occur before the configured expiry and cannot be replayed.
4. Make privileged actions observable and limit administrator authority.
5. Fail safely when authentication, validation, storage, or token operations fail.

Out of scope are the security of Stellar consensus and RPC infrastructure, the user's device and operating system, third-party notification providers, and the correctness of external identity or legal processes.

## 2. Assumptions and Trust Boundaries

- Stellar consensus and Soroban host authorization semantics are trusted to execute deployed bytecode correctly.
- The deployed contract address and WASM artifact are verified by operators and users.
- Owners and beneficiaries control their Stellar accounts and protect their signing keys or passkeys.
- Off-chain passkey, 2FA, reminder, and monitoring services may be unavailable or compromised; they must not be treated as custodians of vault funds.
- The token contract is trusted to implement the expected transfer semantics and is constrained by token allowlisting and initialization validation.
- The contract boundary is untrusted input: all addresses, amounts, identifiers, metadata, timing values, and authorization claims require validation.
- External token transfers and other host calls are trust boundaries; contract state must be made consistent before making calls where reentrancy or callback behavior is relevant.

## 3. Protected Assets

| Asset | Security property | Impact if compromised |
|---|---|---|
| Vault balances and token allowances | Confidentiality is limited by the public ledger; integrity and availability are critical | Unauthorized fund loss, locked funds, or incorrect payout |
| Owner and beneficiary configuration | Integrity and authorization | Funds released to the wrong account or owner locked out |
| Check-in deadline and TTL state | Integrity and availability | Premature release, delayed release, or archival/liveness failure |
| Release, withdrawal, vesting, and dispute state | Integrity and replay resistance | Double spend, duplicate claim, or bypass of release conditions |
| Contract admin and pause state | Integrity and availability | Emergency controls abused or unavailable |
| Authentication and 2FA verification state | Authenticity and freshness | Unauthorized owner action or an incorrectly blocked action |
| On-chain events and audit records | Integrity and traceability | Missed detection or inability to reconstruct security events |
| Contract code, storage layout, and configuration | Integrity and compatibility | System-wide compromise or migration-induced data loss |

## 4. Threat Register

Ratings describe the residual exposure after the listed controls: **High**, **Medium**, or **Low**. Likelihood and impact should be re-evaluated after every material contract change.

| ID | Threat / attack scenario | Affected assets | Mitigations and verification evidence | Residual risk |
|---|---|---|---|---|
| T-01 | Owner signing key, passkey, backup code, or connected device is compromised and an attacker performs owner actions. | Balances; configuration; authentication state | Owner actions require `owner.require_auth()`; passkey hashes and single-use backup codes are validated; optional 2FA causes `withdraw` to require a current verification. Test authentication, replay, expiry, and recovery paths. | **High** — a fully compromised owner authority may still authorize valid actions; users must protect credentials and use 2FA. |
| T-02 | A beneficiary or other caller attempts release before the owner is inactive. | Balances; deadline; release state | `is_expired()` gates release and returns `ContractError::NotExpired`; check-in resets the countdown; release state is one-way. Test boundary ledgers and repeated release attempts. | **Low** — depends on correct ledger-time semantics and deployed code. |
| T-03 | An attacker changes beneficiaries, adds the owner as a beneficiary, or exploits invalid BPS totals. | Beneficiary configuration; balances | Owner authentication; `create_vault` and `set_beneficiaries` reject owner entries; beneficiary and BPS validation occurs before persistence. Test zero, duplicate, owner, and rounding cases. | **Low** — malformed or ambiguous beneficiary requirements can still cause user error. |
| T-04 | Admin pauses the contract, rotates authority maliciously, or attempts to redirect or access funds. | Pause state; availability; admin state | Admin cannot access vault funds or change owners/beneficiaries; state-changing functions enforce pause checks; `propose_admin`/`accept_admin` is two-step; admin actions emit observable events. Test unauthorized, paused, and rotation paths. | **Medium** — admin can deny service while paused; governance and key protection remain operational risks. |
| T-05 | Contract is initialized twice or initialization parameters are malicious. | Admin; token configuration; all vaults | `initialize()` rejects an existing admin/token and returns `ContractError::AlreadyInitialized`; token and XLM addresses are validated. Test double initialization and invalid configuration. | **Low** — deployment and upgrade operators must verify the initial transaction. |
| T-06 | Reentrancy or an external token call observes inconsistent state and causes a duplicate withdrawal/release. | Balances; release and withdrawal state | Mutations occur before token transfers; released status is written before `token.transfer`; external calls are not placed between state reads and writes. Confirm with code review and adversarial tests. | **Low** — relies on Soroban execution semantics and complete coverage of every external-call path. |
| T-07 | Integer overflow, negative/zero amount, BPS rounding, or vesting arithmetic creates or destroys value. | Balances; distributions; vesting state | Checked/saturating arithmetic; amount and installment validation; BPS must total 10,000; final beneficiary absorbs remainder. Run property tests and boundary-value tests. | **Low** — future arithmetic changes require regression and property testing. |
| T-08 | Persistent storage expires, is archived, collides across vault IDs, or cannot be restored. | Vault state; deadline; availability | TTL is extended on storage operations; TTL bounds are validated; keys are unique per vault; archived state can be restored before release. Test low TTL, maximum TTL, archival, restoration, and key-collision cases. | **Medium** — users may lose practical access if they do not renew or restore state in time. |
| T-09 | A beneficiary claims twice, claims before release, or bypasses vesting, dispute, decline, or acceptance conditions. | Release, claim, and vesting state; balances | Claims require released state and valid installment/index; balances are decremented before transfers; declined/disputed states block release; beneficiary identity and acceptance checks apply. Test replay and state-transition matrices. | **Low** — beneficiary account compromise and unresolved human disputes are not prevented by the contract. |
| T-10 | Metadata, identifiers, or unbounded loops cause storage exhaustion or exceed Soroban CPU/memory budgets. | Availability; storage; transaction execution | Enforce metadata and custom metadata limits; bound beneficiary and schedule collections; test worst-case host budgets and ledger entry sizes. | **Medium** — resource limits can change with feature growth and require monitoring. |
| T-11 | A malicious or incorrect token contract reports unexpected transfer behavior or an unapproved asset is deposited. | Vault balances; token integrity | Accept only whitelisted token addresses; validate the XLM token at initialization; propagate transfer errors; reconcile aggregate vault balances. | **Medium** — supported asset behavior and external token upgrades remain dependencies. |
| T-12 | Upgrade, migration, or version mismatch corrupts storage or changes security invariants. | Code; storage layout; all vaults | Store/read contract version; document breaking key changes; preserve layout; require migration tests and audit sign-off before mainnet upgrades. | **High** — upgrade authority and deployment integrity can affect every vault. |
| T-13 | Events, reminders, RPC responses, or monitoring are unavailable or misleading, delaying user action. | Availability; audit records; check-in liveness | Core authorization and release rules execute on-chain; actions emit transparent events; users can query view functions; operational monitoring should alert on admin rotations and low TTL. | **Medium** — off-chain outages may cause an owner to miss a check-in and cannot be fully eliminated. |

## 5. Required Security Controls

- **Authorization:** Every owner/admin state mutation must authenticate the correct actor; no caller-supplied identity is trusted without authorization.
- **State transitions:** Validate all preconditions before mutation, write security-critical state before external transfers, and reject invalid or terminal-state transitions.
- **Value conservation:** Enforce non-negative amounts, checked arithmetic, exact BPS allocation, and balance reconciliation around every transfer.
- **Availability:** Apply bounded inputs and loops, extend TTL deliberately, and document restoration and expiry procedures.
- **Observability:** Emit events for admin rotation, release, withdrawal, pause, and other security-relevant transitions; retain enough context for monitoring without relying on raw ledger topic inspection.
- **Change management:** Require code review, regression/property tests, storage migration tests, dependency review, and independent audit before mainnet deployment or security-sensitive upgrades.

The detailed release checklist is maintained in [security-audit-checklist.md](security-audit-checklist.md). A checklist item marked as complete is evidence of a review or test result, not a substitute for an external audit.

## 6. Residual Risk and Operational Acceptance

The following risks remain even when the contract controls operate as designed:

- Compromise of an owner's complete authentication authority can authorize otherwise valid transactions.
- Loss of keys, passkeys, backup codes, or access to the Stellar account can make funds unrecoverable.
- Missed check-ins, expired TTL, unavailable RPC infrastructure, or an unresponsive reminder provider can cause unintended release or inability to act.
- Admin compromise can cause denial of service through pausing and can affect upgrade/configuration authority, although it cannot directly withdraw vault funds under the stated controls.
- Errors or malicious behavior in supported external token contracts, wallets, frontends, SDKs, and monitoring systems remain outside the vault's trust boundary.
- Public-chain metadata and transaction history are observable; users must not store secrets or sensitive personal data in vault metadata.
- Smart-contract defects not covered by tests or audit may still cause fund loss or permanent lockup.

These risks require explicit acceptance by the deployment owner before mainnet. High residual risks must have an owner, a documented response plan, and either a compensating control or a decision not to deploy.

## 7. Audit and Review Status

- **Current status:** Not independently audited.
- **Pre-mainnet gate:** Complete the [security audit checklist](security-audit-checklist.md), obtain an external audit, resolve all critical/high findings, and complete community review.
- **Review triggers:** Reassess this document for changes to authentication, token support, release/vesting logic, admin or upgrade paths, storage layout, external integrations, or Soroban runtime behavior.
- **Evidence expected:** Linked commits, test results, audit reports, migration plans, deployment artifact hashes, and sign-off for accepted residual risks.

Community review and responsible disclosure are welcome; see the repository [security policy](../SECURITY.md) for reporting instructions.
