# Passkey Integration

## Overview

TTL-Legacy uses Passkeys (WebAuthn) for authentication, eliminating seed phrase management.

## Why Passkeys?

- No seed phrases to lose or expose
- Biometric authentication (fingerprint, Face ID)
- Hardware-backed security
- Phishing-resistant

## Architecture (Planned)

1. **Frontend**: WebAuthn API for passkey creation and signing
2. **Smart Contract**: Verifies signatures via zk_verifier contract
3. **User Flow**:
   - Register passkey during vault creation
   - Sign check-ins with passkey
   - No private key exposure

## Current Status

Passkey integration is planned for v2.0. Current implementation uses standard Stellar address authentication.

## Future Implementation

- Store passkey public key in vault metadata
- Verify WebAuthn signatures on-chain
- Support multiple passkeys per vault

## Biometric Verification

TTL-Legacy supports biometric verification (fingerprint, face) as an enhanced check-in mechanism. Biometric credentials are stored as SHA-256 hash commitments — the raw biometric data never leaves the device.

### How It Works

1. Owner registers a biometric credential hash via `register_biometric`
2. On check-in, the owner presents the credential hash via `biometric_check_in`
3. The contract verifies the hash matches a registered credential
4. On success, `last_check_in` is reset and a `bio_ci` event is emitted

### Biometric API

```rust
register_biometric(vault_id: u64, caller: Address, credential_hash: BytesN<32>) -> Result<(), ContractError>
remove_biometric(vault_id: u64, caller: Address, credential_hash: BytesN<32>) -> Result<(), ContractError>
biometric_check_in(vault_id: u64, caller: Address, credential_hash: BytesN<32>) -> Result<(), ContractError>
get_vault_biometrics(vault_id: u64) -> Vec<BiometricEntry>
is_valid_biometric(vault_id: u64, credential_hash: BytesN<32>) -> bool
```

### Events

| Topic | Data | Description |
|---|---|---|
| `bio_reg` | `credential_hash` | Biometric credential registered |
| `bio_rm` | `credential_hash` | Biometric credential removed |
| `bio_ci` | `(caller, timestamp)` | Biometric check-in performed |

### Security Properties

- Multiple credentials per vault (e.g., fingerprint + face ID)
- Duplicate registration is rejected with `InvalidPasskey`
- Only the vault owner can register or remove credentials
- Biometric check-in respects contract and vault pause state
- Raw biometric data is never stored on-chain — only the hash commitment
- Check-in on a Released vault is rejected with `AlreadyReleased`

## Backup Code Encryption (Issue #553)

Backup codes can be stored on-chain in encrypted form so only the vault owner can recover them.

### How It Works

1. The client generates 10 backup codes off-chain.
2. The client encrypts the codes using the owner's X25519 public key (NaCl box: XSalsa20-Poly1305).
3. The encrypted payload (`nonce || ciphertext`) is submitted to the contract via `store_encrypted_backup_codes`.
4. The contract stores the payload alongside the owner's public key and a timestamp.
5. Only the owner's private key can decrypt the payload — the contract never sees plaintext codes.

### API

```rust
store_encrypted_backup_codes(
    vault_id: u64,
    caller: Address,
    owner_pubkey: BytesN<32>,
    encrypted_payload: Bytes,
) -> Result<(), ContractError>

get_encrypted_backup_codes(vault_id: u64) -> Option<EncryptedBackupCodes>
```

### Events

| Topic   | Data                          | Description                          |
|---------|-------------------------------|--------------------------------------|
| `bk_enc`| `(owner_pubkey, generated_at)`| Encrypted backup codes stored        |

### Security Properties

- Encryption is performed entirely client-side; the contract stores only opaque ciphertext.
- Overwriting is allowed — calling `store_encrypted_backup_codes` again replaces the previous entry.
- Only the vault owner (authenticated via `caller.require_auth()`) can store encrypted codes.
- Storing is rejected on Released vaults with `AlreadyReleased`.

## Passkey Usage Analytics (Issue #554)

Detailed per-passkey usage statistics are available for security auditing.

### How It Works

Every `check_in` call appends a `PasskeyUsageEntry` (passkey hash + timestamp) to persistent storage. The `get_passkey_analytics` function aggregates this log into a `PasskeyAnalytics` report.

### API

```rust
get_passkey_analytics(vault_id: u64) -> PasskeyAnalytics
```

### PasskeyAnalytics Fields

| Field             | Type                      | Description                                  |
|-------------------|---------------------------|----------------------------------------------|
| `vault_id`        | `u64`                     | The vault being queried                      |
| `total_uses`      | `u32`                     | Total number of passkey check-ins            |
| `unique_passkeys` | `u32`                     | Number of distinct passkeys used             |
| `last_used`       | `u64`                     | Timestamp of the most recent check-in        |
| `per_passkey`     | `Vec<PasskeyUsageStat>`   | Per-passkey breakdown                        |

Each `PasskeyUsageStat` contains:

| Field          | Type          | Description                              |
|----------------|---------------|------------------------------------------|
| `passkey_hash` | `BytesN<32>`  | The passkey identifier                   |
| `use_count`    | `u32`         | How many times this passkey was used     |
| `first_used`   | `u64`         | Timestamp of first use                   |
| `last_used`    | `u64`         | Timestamp of most recent use             |

### Events

| Topic     | Data                          | Description                          |
|-----------|-------------------------------|--------------------------------------|
| `pk_anly` | `(total_uses, unique_passkeys)`| Analytics query performed            |

### Use Cases

- Detect dormant passkeys (not used recently) for rotation reminders.
- Identify which device/passkey is most active.
- Audit unusual check-in patterns for security review.
