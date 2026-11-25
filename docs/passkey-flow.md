# Passkey Flows

This document covers the three passkey lifecycle flows using Mermaid sequence diagrams: registration, passkey-authenticated check-in, and passkey rotation.

---

## 1. Passkey Registration

Passkey registration binds a WebAuthn credential to a vault. It is a two-phase process: the frontend generates the credential and stores the hash on-chain; optionally, the owner then registers the secp256r1 public key to enable full on-chain signature verification.

```mermaid
sequenceDiagram
    participant User
    participant Frontend
    participant Backend
    participant Contract as ttl_vault Contract

    User->>Frontend: Initiate passkey registration
    Frontend->>Frontend: Call WebAuthn navigator.credentials.create()
    Note over Frontend: Authenticator generates a P-256 key pair.<br/>The public key is returned as a CBOR attestation object.

    Frontend->>Frontend: Hash(public_key_bytes) → passkey_hash (32 bytes)
    Frontend->>Frontend: Extract SEC-1 uncompressed public key (65 bytes)

    Frontend->>Backend: POST /passkey/register { vault_id, passkey_hash, public_key, backup_eligible }
    Backend->>Backend: Authenticate owner session
    Backend->>Contract: add_passkey(vault_id, owner, passkey_hash, backup_eligible)
    Contract->>Contract: Append PasskeyHash entry to VaultPasskeys(vault_id)
    Contract-->>Backend: Ok(())
    Backend->>Contract: register_passkey_public_key(vault_id, owner, passkey_hash, public_key_65_bytes)
    Contract->>Contract: Verify passkey_hash exists in VaultPasskeys
    Contract->>Contract: Store PasskeyPublicKey(vault_id, passkey_hash) = public_key
    Contract-->>Backend: Ok(())
    Backend-->>Frontend: Registration confirmed
    Frontend-->>User: Passkey registered
```

**Key contract functions:**
- `add_passkey(vault_id, caller, passkey_hash, backup_eligible)` — appends the hash to the vault's passkey list and emits `add_pk`.
- `register_passkey_public_key(vault_id, caller, passkey_hash, public_key)` — binds the 65-byte SEC-1 uncompressed public key to the hash, enabling on-chain secp256r1 signature verification. Emits `pk_pub`.

---

## 2. Passkey-Authenticated Check-In

After registration, the owner proves liveness by signing a check-in with their passkey. The contract verifies the signature on-chain and extends the vault's TTL.

```mermaid
sequenceDiagram
    participant User
    participant Frontend
    participant Backend
    participant Contract as ttl_vault Contract

    User->>Frontend: Initiate check-in
    Frontend->>Backend: GET /checkin/challenge { vault_id }
    Backend->>Contract: get_passkey_challenge(vault_id, passkey_hash)
    Contract-->>Backend: challenge (BytesN<32>)
    Backend-->>Frontend: challenge bytes

    Frontend->>Frontend: Call WebAuthn navigator.credentials.get(challenge)
    Note over Frontend: Authenticator signs authenticatorData + SHA-256(clientDataJSON)<br/>using the vault's private key.
    Frontend->>Frontend: Encode signature as 64-byte raw r||s (big-endian)

    Frontend->>Backend: POST /checkin { vault_id, passkey_hash, signature, message }
    Backend->>Backend: Authenticate owner session
    Backend->>Contract: check_in(vault_id, owner, passkey_hash, nonce, signature, message)

    Contract->>Contract: Assert contract not paused
    Contract->>Contract: Assert vault not locked / frozen / expired
    Contract->>Contract: Enforce minimum check-in cooldown (DEFAULT_MIN_CHECKIN_COOLDOWN = 60s)
    Contract->>Contract: validate_passkey_for_checkin() — check expiry + hash registration
    Contract->>Contract: verify_passkey_signature() — secp256r1_verify(public_key, SHA-256(message), signature)
    Note over Contract: If no public key registered, signature step is skipped (legacy vaults).

    Contract->>Contract: Update last_check_in = ledger.timestamp()
    Contract->>Contract: Extend persistent storage TTL
    Contract->>Contract: Update check-in streak, score, history
    Contract-->>Backend: Ok(())
    Backend-->>Frontend: Check-in confirmed
    Frontend-->>User: Check-in success, TTL extended
```

**Key contract functions:**
- `check_in(vault_id, caller, passkey_hash, nonce, signature, message)` — primary owner check-in entry point. Emits `check_in` and `ci_rec`.

**Validation order inside `check_in`:**
1. Contract-level pause guard.
2. Vault-level pause and freeze guards.
3. Owner or delegate identity check.
4. Per-delegation nonce check (delegates only).
5. Vault status and expiry checks.
6. Minimum cooldown rate limit.
7. Passkey registration and expiry validation.
8. secp256r1 signature verification (if a public key is registered).

---

## 3. Passkey Rotation

Passkey rotation replaces the primary passkey with a new credential. A configurable rotation policy can enforce a minimum interval between rotations.

```mermaid
sequenceDiagram
    participant User
    participant Frontend
    participant Backend
    participant Contract as ttl_vault Contract

    User->>Frontend: Initiate passkey rotation
    Frontend->>Frontend: Generate new WebAuthn credential via navigator.credentials.create()
    Frontend->>Frontend: new_passkey_hash = Hash(new_public_key_bytes)
    Frontend->>Frontend: Extract new 65-byte SEC-1 uncompressed public key

    Frontend->>Backend: POST /passkey/rotate { vault_id, old_passkey_hash, new_passkey_hash, new_public_key }
    Backend->>Backend: Authenticate owner session

    Backend->>Contract: rotate_passkey(vault_id, owner, old_passkey_hash, new_passkey_hash)
    Contract->>Contract: Verify old_passkey_hash matches vault.passkey_hash
    Contract->>Contract: Load PasskeyRotationPolicy(vault_id)
    Contract->>Contract: If policy.enforce: check now - last_rotation >= rotation_period_days * 86400
    Note over Contract: Returns PasskeyRotationTooSoon (error 154) if interval not elapsed.
    Contract->>Contract: Update vault.passkey_hash = new_passkey_hash
    Contract->>Contract: Store LastVaultPasskeyRotation(vault_id) = now
    Contract-->>Backend: Ok(()) — emits rot_pk

    Backend->>Contract: register_passkey_public_key(vault_id, owner, new_passkey_hash, new_public_key_65_bytes)
    Contract->>Contract: Bind new public key to new_passkey_hash
    Contract-->>Backend: Ok(()) — emits pk_pub

    Backend-->>Frontend: Rotation confirmed
    Frontend-->>User: New passkey active
```

**Key contract functions:**
- `rotate_passkey(vault_id, caller, old_passkey_hash, new_passkey_hash)` — atomically replaces the primary passkey. Enforces rotation policy if configured. Emits `rot_pk`.
- `set_passkey_rotation_policy(vault_id, caller, policy)` — configures the minimum rotation interval (`rotation_period_days`) and whether it is enforced. Only the vault owner can call this.

**Rotation policy fields (`PasskeyRotationPolicy`):**

| Field | Type | Description |
| :--- | :--- | :--- |
| `rotation_period_days` | `u32` | Minimum days between rotations. `0` disables the interval check. |
| `enforce` | `bool` | When `false`, the policy is advisory only and does not block rotation. |

**Relevant events:**

| Topic | Emitted By | Payload |
| :--- | :--- | :--- |
| `add_pk` | `add_passkey` | `(passkey_hash, backup_eligible)` |
| `pk_pub` | `register_passkey_public_key` | `passkey_hash` |
| `rot_pk` | `rotate_passkey` | `(old_passkey_hash, new_passkey_hash)` |
| `rm_pk` | `remove_passkey` | `passkey_hash` |
| `pk_revoke` | internal revocation path | `passkey_hash` |
| `pk_expd` | check-in with expired passkey | `passkey_hash` |
| `pk_comp` | compromise detected/reported | `(vault_id, passkey_hash)` |
| `pk_lock` | lockout triggered | `vault_id` |
