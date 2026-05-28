# Architecture Overview

## System Components

### Smart Contracts (Soroban)

**ttl_vault** - Core vault contract managing vault lifecycle, check-ins, TTL-based expiry, and beneficiary releases.

**zk_verifier** - Passkey authentication verifier (future).

### Frontend (Planned)

Passkey-based authentication, vault dashboard, check-in interface.

### Backend (Planned)

Encrypted reminders, TTL monitoring, event indexing.

#### Reminder Retry Policy

The reminder service uses exponential backoff for failed deliveries. Each notification tracks a `ReminderDeliveryLog` with per-attempt records and a `DeliveryStatus` of `Pending | Sent | Failed | Retrying`.

Retry schedule after a failed attempt:

| Attempt | Delay   |
|---------|---------|
| 1       | 1 min   |
| 2       | 5 min   |
| 3       | 15 min  |
| 4       | 1 hour  |
| 5       | 6 hours |

After all 5 retries are exhausted the status is set to `Failed` and an `[ALERT]` log line is emitted for external monitoring. The background scheduler calls `flush_retries()` on every tick alongside `flush_pending()`.

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
