# TTL-Legacy Threat Model

**Version:** 1.0  
**Date:** 2026-08-28  
**Status:** Active  
**Classification:** Internal / Engineering

---

## 1. Executive Summary

This document presents a formal threat model for the TTL-Legacy system — a decentralised digital-inheritance vault built on Stellar/Soroban smart contracts. The model was produced using the **STRIDE** methodology (Spoofing, Tampering, Repudiation, Information Disclosure, Denial of Service, Elevation of Privilege) and is intended to guide security engineering decisions, audit scope, and incident-response planning.

**Scope:** All components of the TTL-Legacy system as described in §2, and the data flows between them.

**Methodology:** STRIDE threat elicitation per component and per data-flow boundary, supported by two attack trees for the highest-risk scenarios. Residual risk is rated High / Medium / Low based on likelihood × impact after existing mitigations.

**Key conclusions:**
- The Soroban smart contract is the highest-value target; its immutability is both a strength (no backdoor) and a liability (bugs are permanent).
- Passkey/WebAuthn authentication eliminates the seed-phrase attack surface but shifts risk to device-compromise and account-recovery scenarios.
- The Rust backend holds the most plaintext-sensitive data (JWT tokens, push tokens, PostgreSQL state) and must be treated as a high-risk boundary.
- TTL expiry mechanics create a novel denial-of-service surface: disrupting check-in delivery could trigger unintended fund release.

---

## 2. System Overview

```
┌──────────────────────────────────────────────────────────────────────┐
│                        Stellar Network                               │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │              Soroban Smart Contract (ttl_vault)             │    │
│  │  - Vault state (owner, beneficiary, balance, TTL)           │    │
│  │  - check_in(), trigger_release(), deposit(), withdraw()     │    │
│  │  - State archival drives TTL expiry                         │    │
│  └─────────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────────┘
         ▲  (XDR transactions, signed by owner keypair)
         │
┌────────┴──────────────────────────────────────┐
│            Rust Backend (Axum / Actix)         │
│  - REST API for mobile clients                 │
│  - WebAuthn / Passkey challenge issuance       │
│  - JWT token generation and validation         │
│  - Push notification dispatch (FCM / APNs)    │
│  - Reminder scheduler                         │
│  - PostgreSQL (vault metadata, push tokens,   │
│    2FA state, audit log)                       │
│  - SQLite (offline / test mode)                │
└────────┬──────────────────────────────────────┘
         │  HTTPS / REST + JWT bearer token
    ┌────┴─────┐         ┌────────────┐
    │  iOS App  │         │ Android App│
    │ (SwiftUI) │         │ (Compose)  │
    │ Passkey   │         │ Passkey /  │
    │ Keychain  │         │ Biometric  │
    └──────────┘         └────────────┘
```

### Components

| Component | Technology | Role |
|---|---|---|
| Soroban Smart Contract | Rust / Soroban SDK | Holds vault funds, enforces TTL rules, executes release |
| Stellar Network | Stellar Core / Horizon | Transaction ordering, ledger consensus, state archival |
| Rust Backend | Rust (Axum), PostgreSQL, Redis | API gateway, authentication, push scheduling |
| iOS App | Swift / SwiftUI, AuthenticationServices | Owner-facing UI, Passkey registration/auth |
| Android App | Kotlin / Jetpack Compose, Credential Manager | Owner-facing UI, Passkey / biometric auth |
| FCM / APNs | Google / Apple push infrastructure | Check-in reminders, expiry warnings |

---

## 3. Assets & Trust Boundaries

### 3.1 Critical Assets

| Asset | Description | Confidentiality | Integrity | Availability |
|---|---|---|---|---|
| Vault funds (XLM) | On-chain balance held by the smart contract | N/A (public ledger) | **Critical** | **Critical** |
| Owner Stellar keypair | Ed25519 key signing transactions; held in OS keychain via Passkey | **Critical** | **Critical** | High |
| Passkey credential (FIDO2) | Platform authenticator credential bound to device; used to derive transaction signing key | **Critical** | **Critical** | High |
| JWT bearer token | Short-lived token (RS256, ~15 min) authorising API calls | High | High | Medium |
| Push device tokens (FCM/APNs) | Used to send check-in reminders | Medium | High | High |
| Beneficiary address | On-chain; also stored in backend DB | Low (public) | **Critical** | Medium |
| Backend PostgreSQL | Vault metadata, push tokens, 2FA secrets, audit log | High | High | High |
| WebAuthn challenge nonce | Single-use 32-byte random; prevents replay | Low | **Critical** | Medium |
| 2FA TOTP secret | Per-vault seed for OTP generation | **Critical** | **Critical** | Medium |
| Backend TLS private key | Protects API traffic in transit | **Critical** | **Critical** | High |
| APNs / FCM API keys | Allows sending push notifications | High | High | High |

### 3.2 Trust Boundaries

```
[Device / User space] ──── TLS ────> [Backend API] ──── signed XDR ──> [Stellar Network]
      ↑ (WebAuthn assertion)               ↑ (DB access, internal)
   [OS Secure Enclave]              [PostgreSQL / Redis]
   [iOS Keychain / Android Keystore]
```

| Boundary | Direction | Protocol | Threat Surface |
|---|---|---|---|
| App ↔ Backend | Bidirectional | HTTPS/TLS 1.3 | MITM, token theft, injection |
| Backend ↔ Stellar | Outbound | HTTPS (Horizon) | RPC forgery, replay, DoS |
| App ↔ Secure Enclave | Local | WebAuthn / FIDO2 | Device compromise, side-channel |
| Backend ↔ DB | Local | TCP (mTLS recommended) | SQL injection, data exfil |
| Backend ↔ FCM/APNs | Outbound | HTTPS | Key compromise, notification spoofing |
| Stellar ↔ Contract | Inbound | XDR transactions | Malicious invocation, reentrancy |

---

## 4. STRIDE Threat Analysis

### 4.1 Spoofing

| ID | Threat | Target Asset | Current Mitigations | Residual Risk |
|---|---|---|---|---|
| S-01 | Attacker forges `check_in()` transaction by spoofing owner identity | Vault TTL / funds | Soroban `require_auth(owner)` enforces Ed25519 signature; Passkey ties key to device biometric | **Low** |
| S-02 | Attacker replays a captured WebAuthn assertion to obtain a JWT | JWT bearer token | Server-side challenge nonce is single-use and expires (30 s); assertion origin and rpID are verified | **Low** |
| S-03 | Attacker registers a forged Passkey against an existing account | Owner keypair | Backend links Passkey credential ID to account at registration; second registration requires authenticated session | Medium |
| S-04 | Malicious app spoofs the TTL-Legacy iOS/Android app to intercept Passkeys | Owner keypair | App-bound WebAuthn rpID `ttl-legacy.app`; FIDO2 verifies facetID / origin; code-signing enforced by OS | **Low** |
| S-05 | Attacker spoofs push notification to suppress check-in reminder | Check-in reminder delivery | Notifications are informational only; the contract TTL is the authoritative timer; missing a notification does not prevent check-in | Medium |
| S-06 | Phishing site at a lookalike domain tricks user into approving WebAuthn | Passkey assertion | WebAuthn origin check rejects assertions from non-matching domains; browser/OS enforces this | **Low** |

### 4.2 Tampering

| ID | Threat | Target Asset | Current Mitigations | Residual Risk |
|---|---|---|---|---|
| T-01 | Attacker modifies `beneficiary` address in DB to redirect vault release | Beneficiary address | Beneficiary address is set on-chain in the contract; DB copy is for display only; release is executed by the contract | Medium (DB mismatch could mislead UI) |
| T-02 | Attacker tampers with JWT payload to escalate privileges | API authorisation | RS256 signature; backend validates signature on every request; tokens are short-lived | **Low** |
| T-03 | Attacker modifies in-transit XDR transaction to redirect funds | Vault funds | XDR transactions are Ed25519-signed; Stellar rejects transactions with invalid signatures | **Low** |
| T-04 | Attacker tampers with the deployed contract bytecode | Smart contract logic | Wasm hash is verified by Soroban at deployment; deployed contracts are immutable (upgrade requires admin key + new deployment) | **Low** |
| T-05 | Compromised backend submits fraudulent `trigger_release()` transaction | Vault funds | `trigger_release()` checks TTL expiry on-chain; backend cannot bypass the time check | **Low** |
| T-06 | Attacker tampers with push-token record to redirect reminders | Check-in reminders | Push token update requires authenticated API call (JWT bearer); 2FA on sensitive operations planned | Medium |

### 4.3 Repudiation

| ID | Threat | Target Asset | Current Mitigations | Residual Risk |
|---|---|---|---|---|
| R-01 | Owner denies performing a check-in or withdrawal | Audit trail | All contract actions emit Soroban events on the public ledger; backend audit log records API call timestamps + credential IDs | **Low** |
| R-02 | Beneficiary denies accepting the beneficiary role | Beneficiary acceptance | On-chain `accept_beneficiary()` call is signed by beneficiary's keypair; ledger provides non-repudiable record | **Low** |
| R-03 | Backend operator denies modifying vault metadata | Backend audit log | Audit log in PostgreSQL records all privileged operations; log integrity depends on DB access controls | Medium (log tampering by DB admin) |
| R-04 | Owner claims check-in reminder was never sent | Reminder delivery | Backend logs push dispatch with timestamp; APNs/FCM delivery receipts not retained; gap in delivery proof | Medium |

### 4.4 Information Disclosure

| ID | Threat | Target Asset | Current Mitigations | Residual Risk |
|---|---|---|---|---|
| I-01 | JWT stolen from app memory or Keychain | API access | Keychain (iOS) / Keystore (Android) with hardware-backed storage; tokens are short-lived; HTTPS only | Medium |
| I-02 | TOTP secret extracted from backend DB | 2FA bypass | Secrets should be encrypted at rest (AES-256-GCM) with a KMS-managed key; key rotation policy required | **High** (if encryption not enforced) |
| I-03 | Push device tokens leaked via backend API | Notification spoofing / user tracking | Tokens are only returned to authenticated account owner; stored hashed where possible | Medium |
| I-04 | Vault balance and beneficiary address exposed to unauthenticated callers | Privacy | Stellar ledger is public; balances are visible to anyone; privacy by obfuscation only | Low (by design) |
| I-05 | Backend TLS key compromised; HTTPS traffic decrypted | All API traffic | Certificate pinning in mobile apps; HSTS; rotate keys on compromise | Medium |
| I-06 | Logs contain sensitive data (vault IDs, addresses) shipped to log aggregator | Vault metadata | Structured logging should redact or hash vault IDs in error messages; not yet enforced | Medium |
| I-07 | SQLite database on-device stores vault state unencrypted | Vault metadata | iOS uses Data Protection (`NSFileProtectionComplete`); Android Keystore; SQLCipher planned | Medium |

### 4.5 Denial of Service

| ID | Threat | Target Asset | Current Mitigations | Residual Risk |
|---|---|---|---|---|
| D-01 | Attacker floods `check_in()` calls to exhaust fee budget | Contract availability | Soroban charges fees per invocation; flooding is costly for the attacker; rate-limit on backend relay | Medium |
| D-02 | Attacker deliberately prevents check-in delivery (push suppression, network block) to trigger unintended vault release | Vault funds | Owner can check in directly via any Stellar-compatible client; reminder is advisory only | **High** (for unsophisticated owners who rely solely on push) |
| D-03 | Stellar network degradation delays TTL extension, causing false expiry | Vault TTL | TTL is ledger-based; partial network outages delay confirmation but do not falsely expire TTL | Medium |
| D-04 | Attacker submits spam vaults to exhaust backend DB / storage | Backend availability | Vault creation requires authenticated account; rate-limiting and account quotas needed | Medium |
| D-05 | FCM / APNs token flood triggers outgoing push rate-limit | Check-in reminders | Reminder scheduler uses per-vault debounce; flood requires authenticated token injection | Low |
| D-06 | Contract state archive expiry evicts vault state before release is claimed | Vault release | Owner or beneficiary must restore archived state before claim; hibernation mode extends archive TTL | Medium |

### 4.6 Elevation of Privilege

| ID | Threat | Target Asset | Current Mitigations | Residual Risk |
|---|---|---|---|---|
| E-01 | Attacker obtains valid JWT and calls owner-only endpoints | API access | `require_auth` middleware validates JWT and asserts ownership claim; vault ID is tied to token subject | **Low** |
| E-02 | Malicious beneficiary claims release before TTL expiry | Vault funds | `trigger_release()` checks TTL on-chain; beneficiary cannot trigger early release | **Low** |
| E-03 | SQL injection in backend grants attacker DB read/write | Backend DB | Parameterised queries (SQLx); input validation; no raw SQL construction from user input | **Low** |
| E-04 | Compromised backend service account pivots to Stellar signing key | Owner keypair | Backend never holds owner signing keys; keys live in device Secure Enclave via Passkey | **Low** |
| E-05 | Attacker exploits Soroban reentrancy in vault contract | Vault funds | Soroban's execution model does not allow mid-function callbacks; cross-contract reentrancy is architecturally constrained | **Low** |
| E-06 | Biometric fallback session token (`biometric-fallback-session`) accepted for privileged operations without on-chain verification | API privileged endpoints | Fallback token must be scoped to read-only or offline operations only; privileged mutations must re-require passkey | **High** (if not scoped) |
| E-07 | Attacker registers as beneficiary of a vault by guessing vault IDs | Beneficiary assignment | Beneficiary acceptance requires a signed invitation token issued by the owner; vault IDs alone are insufficient | **Low** |

---

## 5. Attack Trees

### 5.1 Attack Tree: Unauthorized Vault Fund Release

**Goal:** Trigger `trigger_release()` and redirect vault funds to an attacker-controlled address.

```
[GOAL] Unauthorised fund release
├── [A] Control the beneficiary address at release time
│   ├── [A1] Modify beneficiary on-chain BEFORE release
│   │   ├── [A1a] Steal owner Passkey + call update_beneficiary()  ← HIGH effort
│   │   └── [A1b] Exploit contract bug allowing beneficiary mutation without auth  ← requires 0-day
│   └── [A2] Redirect funds after release (off-chain)
│       └── [A2a] Social-engineer or compromise beneficiary's wallet  ← out of scope
│
├── [B] Trigger release before TTL genuinely expires
│   ├── [B1] Exploit off-by-one in TTL ledger calculation  ← requires contract bug
│   ├── [B2] Tamper with ledger close time (requires 51% of validators)  ← extremely high effort
│   └── [B3] Prevent owner from checking in (DoS attack tree §5.2 feeds here)
│
└── [C] Impersonate owner to call check_in() and then update_beneficiary()
    ├── [C1] Compromise device with registered Passkey
    │   ├── [C1a] Physical device access + biometric bypass
    │   └── [C1b] Malware with root privileges extracting Secure Enclave key  ← infeasible on modern OS
    └── [C2] Social-engineer owner into approving malicious transaction
        └── [C2a] Phishing site (blocked by WebAuthn rpID check)
```

**Highest-risk path:** B3 (DoS on check-in) → TTL genuine expiry → C (beneficiary already compromised). Mitigation: educate owners to use multiple check-in paths; allow check-in from CLI/Horizon directly.

### 5.2 Attack Tree: Owner Account Takeover

**Goal:** Gain the ability to sign transactions as the vault owner.

```
[GOAL] Owner account takeover
├── [A] Steal or clone Passkey credential
│   ├── [A1] Physical access to unlocked device
│   │   └── [A1a] Bypass biometric lock (fake fingerprint, face spoofing)
│   ├── [A2] Exploit OS vulnerability to extract Secure Enclave key material
│   │   └── — Requires unpatched hardware-level 0-day; extremely low probability
│   └── [A3] Intercept WebAuthn assertion in transit
│       └── — Blocked by TLS + FIDO2 origin binding
│
├── [B] Hijack the account-recovery flow
│   ├── [B1] No recovery flow exists (by design)  ← protects against hijack but creates loss risk
│   └── [B2] If recovery is added: compromise recovery channel (email/SMS)
│       ├── [B2a] SIM swap for SMS recovery
│       └── [B2b] Email account compromise
│
├── [C] Compromise the backend and forge JWT with owner claim
│   ├── [C1] Steal RS256 private key from backend
│   └── [C2] Exploit JWT validation bug (algorithm confusion, none-alg)
│       └── — Mitigated by explicit RS256 enforcement; no none-alg allowed
│
└── [D] Man-in-the-middle API traffic and replay valid JWT
    ├── [D1] Undermine TLS (cert pinning bypass in compromised app)
    └── [D2] Steal JWT from app memory (requires root / malware)
```

**Highest-risk path:** B2 (recovery channel compromise) if account recovery is ever added without MFA. Current design has no recovery, which eliminates B2 but creates fund-loss risk if owner loses device.

---

## 6. Mitigations Summary Table

| Threat ID | Threat Summary | Mitigation | Status |
|---|---|---|---|
| S-01 | Forge check_in() as owner | `require_auth(owner)` in Soroban; Passkey biometric binding | ✅ Implemented |
| S-02 | WebAuthn assertion replay | Single-use server nonce, 30 s expiry; origin/rpID binding | ✅ Implemented |
| S-03 | Register forged Passkey on existing account | Second registration requires authenticated session | ⚠️ Partial |
| S-04 | App spoofing to steal Passkey | FIDO2 facetID / app-bound rpID; OS code-signing | ✅ Implemented |
| S-05 | Suppressed push notification | TTL is contract-authoritative; push is advisory only | ✅ Implemented |
| T-01 | Tamper beneficiary in DB | Beneficiary canonical source is on-chain; UI reconciliation needed | ⚠️ Partial |
| T-02 | Forge JWT payload | RS256 signature; short-lived tokens | ✅ Implemented |
| T-03 | Tamper XDR in transit | Ed25519 transaction signatures; TLS | ✅ Implemented |
| T-04 | Modify deployed contract | Soroban Wasm hash verification; immutable deployment | ✅ Implemented |
| T-05 | Backend submits fraudulent release | On-chain TTL check in contract | ✅ Implemented |
| T-06 | Redirect push reminders via token tampering | Push token update requires JWT auth | ⚠️ Partial (2FA not yet required) |
| R-01 | Owner denies check-in / withdrawal | Stellar ledger events + backend audit log | ✅ Implemented |
| R-02 | Beneficiary denies acceptance | On-chain signed acceptance | ✅ Implemented |
| R-03 | Operator log tampering | PostgreSQL audit log; access controls | ⚠️ Partial (immutable log needed) |
| R-04 | Reminder never sent | Backend dispatch log; delivery receipts not stored | ⚠️ Partial |
| I-01 | JWT stolen from device | Hardware-backed Keychain/Keystore; short TTL | ✅ Implemented |
| I-02 | TOTP secret extracted from DB | Encryption at rest with KMS key | 🔲 Planned |
| I-03 | Push tokens leaked | Tokens returned only to authenticated owner | ✅ Implemented |
| I-05 | TLS key compromise | Certificate pinning; HSTS | ⚠️ Partial (pinning not yet shipped) |
| I-06 | Sensitive data in logs | Structured log redaction | 🔲 Planned |
| I-07 | Unencrypted on-device SQLite | Data Protection / Keystore; SQLCipher | ⚠️ Partial |
| D-01 | check_in() fee exhaustion | Soroban fee market; backend rate-limiting | ✅ Implemented |
| D-02 | Push suppression → unintended release | Multi-channel check-in; Horizon CLI fallback | ⚠️ Partial (owner education needed) |
| D-03 | Stellar network delay | Ledger-based TTL; no false expiry | ✅ Implemented |
| D-04 | Spam vault creation | Authenticated account required; rate-limiting | ⚠️ Partial (quotas not enforced) |
| D-06 | State archive expiry | Hibernation mode extends archive TTL | ✅ Implemented |
| E-01 | JWT privilege escalation | Ownership claim tied to token subject | ✅ Implemented |
| E-02 | Beneficiary early release | On-chain TTL check | ✅ Implemented |
| E-03 | SQL injection | SQLx parameterised queries; input validation | ✅ Implemented |
| E-04 | Backend pivots to owner key | Backend never holds owner signing keys | ✅ Implemented |
| E-06 | Biometric fallback token over-privilege | Scope fallback token to read-only ops | 🔲 Planned |
| E-07 | Guess vault ID to register as beneficiary | Signed invitation token required | ✅ Implemented |

---

## 7. Security Assumptions

The threat model relies on the following explicit assumptions. If any assumption is violated, associated threats must be re-evaluated.

1. **Secure Enclave integrity:** The iOS Secure Enclave and Android StrongBox / Trusted Execution Environment correctly protect Ed25519 key material and Passkey credentials. Hardware-level side-channel attacks are out of scope.

2. **WebAuthn specification correctness:** The `AuthenticationServices` (iOS) and `CredentialManager` (Android) frameworks correctly implement WebAuthn Level 2, including origin binding and assertion replay prevention.

3. **Stellar consensus correctness:** The Stellar network provides Byzantine fault-tolerant consensus and does not produce ledgers with falsified timestamps or out-of-order TTL calculations.

4. **Soroban contract immutability:** Once deployed, the vault contract Wasm cannot be replaced without an explicit upgrade transaction signed by the contract admin key. The admin key is not held by the backend.

5. **TLS 1.3 integrity:** All client–backend communication is protected by TLS 1.3 with valid certificates issued by a public CA. Certificate pinning is assumed to be enforced by v1.1.

6. **Backend OS hardening:** The server running the Rust backend is a hardened Linux instance with SELinux/AppArmor, minimal attack surface, and automated patching.

7. **PostgreSQL access controls:** Database access is restricted to the backend service account; direct external access is not possible from the internet.

8. **FCM / APNs authenticity:** Google and Apple correctly authenticate TTL-Legacy's server credentials before delivering push notifications to registered tokens.

9. **JWT RS256 key security:** The RS256 private key used for JWT signing is stored in a hardware security module (HSM) or equivalent key management service, and is never written to disk in plaintext.

10. **Biometric fallback scope:** The `biometric-fallback-session` token is only accepted for read-only and non-financial operations. On-chain transactions always require a fresh Passkey assertion.

---

## 8. Out of Scope

The following items are explicitly **not** covered by this threat model:

- **Stellar validator compromise:** Attacks requiring control of ≥ 34% of quorum slices.
- **Hardware-level cryptographic attacks:** Side-channel attacks against Secure Enclave (power analysis, cache timing).
- **OS zero-days:** Exploitation of unpatched iOS / Android kernel vulnerabilities granting ring-0 access.
- **Physical coercion of the owner:** Rubber-hose cryptanalysis or legal compulsion.
- **Third-party dependency supply-chain:** Vulnerabilities in Rust crates, Swift packages, or Android libraries used by the project (addressed separately by `cargo audit` and Dependabot).
- **Regulatory and legal risk:** Estate law, jurisdiction, and compliance requirements.
- **Front-end (web dashboard) security:** The web frontend is covered by a separate threat model.
- **Beneficiary wallet security:** Security of the beneficiary's own Stellar account after funds are released.
- **APNs / FCM infrastructure attacks:** Attacks against Google's or Apple's push infrastructure.

---

## 9. Review Cadence

| Trigger | Action |
|---|---|
| **Scheduled:** Every 6 months | Full STRIDE review with updated component inventory |
| **On any smart-contract upgrade** | Re-evaluate all T-0x and E-0x threats |
| **On any new authentication method added** | Re-evaluate all S-0x and I-0x threats |
| **On any new backend API endpoint** | Evaluate applicable STRIDE categories for the new surface |
| **On a disclosed CVE in a direct dependency** | Ad-hoc review of affected component threats |
| **After any security incident** | Post-incident threat-model update and residual-risk reassessment |
| **Before any public mainnet launch** | Full independent security audit + threat-model sign-off |

---

*This document was produced by the TTL-Legacy engineering team. Questions or proposed updates should be submitted as a pull request to `docs/threat-model.md`. The document becomes stale if not reviewed within 12 months of the date above.*
