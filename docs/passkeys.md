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
