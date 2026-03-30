# Architecture Overview

## System Components

### Smart Contracts (Soroban)

**ttl_vault** - Core vault contract managing vault lifecycle, check-ins, TTL-based expiry, and beneficiary releases.

**zk_verifier** - Passkey authentication verifier (future).

### Frontend (Planned)

Passkey-based authentication, vault dashboard, check-in interface.

### Backend (Planned)

Encrypted reminders, TTL monitoring, event indexing.

## Data Flow

```
Owner → Create Vault → Store on Stellar
Owner → Check In → Extend TTL
Time Passes → TTL Expires → Beneficiary triggers release
```

## Storage

- **Instance**: Admin, token, config
- **Persistent**: Vault data, count
- **Temporary**: Indexes

## Security

Owner authentication, admin controls, pause mechanism, structured errors.
