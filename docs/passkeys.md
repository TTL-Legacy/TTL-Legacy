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
