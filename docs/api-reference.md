# TTL-Legacy Backend API Reference

Base URL: `http://localhost:3000`

---

## Reminder Preferences

### POST `/api/vaults/{vault_id}/reminder-preferences`

Create or update reminder preferences for a vault.

**Path Parameters**

| Name       | Type   | Description          |
|------------|--------|----------------------|
| `vault_id` | uint64 | On-chain vault ID    |

**Request Body** (`application/json`)

| Field                  | Type            | Required | Description                                              |
|------------------------|-----------------|----------|----------------------------------------------------------|
| `channels`             | array of string | Yes      | One or more of: `"email"`, `"sms"`, `"push"`            |
| `hours_before_expiry`  | integer (> 0)   | Yes      | Hours before TTL expiry to send the first reminder       |
| `frequency`            | string          | Yes      | How often to repeat: `"once"`, `"daily"`, `"hourly"`    |
| `timezone`             | string          | No       | IANA zone for the 09:00 local reminder time (default `"UTC"`) |

**Example Request**

```json
{
  "channels": ["email", "sms"],
  "hours_before_expiry": 48,
  "frequency": "daily",
  "timezone": "America/New_York"
}
```

**Responses**

| Status | Description                              |
|--------|------------------------------------------|
| 200    | Preferences saved; returns saved object  |
| 422    | Validation error (empty channels, zero hours) |
| 500    | Internal server error                    |

**Example Response (200)**

```json
{
  "vault_id": 42,
  "channels": ["email", "sms"],
  "hours_before_expiry": 48,
  "frequency": "daily"
}
```

---

### GET `/api/vaults/{vault_id}/reminder-preferences`

Retrieve current reminder preferences for a vault.

**Path Parameters**

| Name       | Type   | Description       |
|------------|--------|-------------------|
| `vault_id` | uint64 | On-chain vault ID |

**Responses**

| Status | Description                                  |
|--------|----------------------------------------------|
| 200    | Returns the stored preferences               |
| 404    | No preferences found for this vault          |
| 500    | Internal server error                        |

**Example Response (200)**

```json
{
  "vault_id": 42,
  "channels": ["email", "sms"],
  "hours_before_expiry": 48,
  "frequency": "daily"
}
```

---

## Two-Factor Authentication

2FA endpoints live under `/api/vaults/{vault_id}/2fa`. All endpoints require the vault to exist and return `404` with `{"error": "vault not found"}` otherwise.

TOTP secrets are encrypted at rest using the server's `TOTP_ENCRYPTION_KEY` environment variable. If that variable is not set, any endpoint that needs to read or write a secret returns `500` with `{"error": "TOTP_ENCRYPTION_KEY not configured"}`.

### POST `/api/vaults/{vault_id}/2fa/setup`

Enable 2FA for a vault. For `totp`, generates a new secret and returns it together with a provisioning URI for authenticator apps. For `sms` / `email`, stores the destination contact and sends a verification code.

**Path Parameters**

| Name       | Type   | Description       |
|------------|--------|-------------------|
| `vault_id` | uint64 | On-chain vault ID |

**Request Body** (`application/json`)

| Field    | Type   | Required | Description                                          |
|----------|--------|----------|------------------------------------------------------|
| `method` | string | Yes      | One of: `"totp"`, `"sms"`, `"email"`                 |
| `phone`  | string | For `sms`| Destination phone number                             |
| `email`  | string | For `email` | Destination email address                         |

**Example Request**

```json
{
  "method": "totp"
}
```

**Responses**

| Status | Description                                                        |
|--------|--------------------------------------------------------------------|
| 200    | 2FA enabled; returns `Enable2FAResponse`                           |
| 400    | Invalid method, or missing `phone` / `email` for the chosen method |
| 404    | Vault not found                                                    |
| 500    | Internal server error, or `TOTP_ENCRYPTION_KEY` not configured     |

**Example Response (200)**

```json
{
  "method": "totp",
  "secret": "JBSWY3DPEHPK3PXP",
  "provisioning_uri": "otpauth://totp/TTL-Legacy:42?secret=JBSWY3DPEHPK3PXP&issuer=TTL-Legacy"
}
```

For `sms` / `email` the response omits `secret` and `provisioning_uri`.

---

### POST `/api/vaults/{vault_id}/2fa/verify`

Verify a 2FA code. On success the vault is marked as verified for the current session/window.

**Path Parameters**

| Name       | Type   | Description       |
|------------|--------|-------------------|
| `vault_id` | uint64 | On-chain vault ID |

**Request Body** (`application/json`)

| Field  | Type   | Required | Description                                  |
|--------|--------|----------|----------------------------------------------|
| `code` | string | Yes      | TOTP code or SMS/email verification code     |

**Example Request**

```json
{
  "code": "123456"
}
```

**Responses**

| Status | Description                                                  |
|--------|--------------------------------------------------------------|
| 200    | Code accepted; returns `{"verified": true}`                  |
| 401    | Invalid or expired code                                      |
| 404    | Vault not found, or 2FA not configured for this vault        |
| 500    | Internal server error, or `TOTP_ENCRYPTION_KEY` not configured |

**Example Response (200)**

```json
{
  "verified": true
}
```

---

### POST `/api/vaults/{vault_id}/2fa/rotate-key`

Rotate the stored TOTP secret. Generates a fresh secret, re-encrypts it with `TOTP_ENCRYPTION_KEY`, and returns a new provisioning URI. Previously issued codes stop working immediately.

**Path Parameters**

| Name       | Type   | Description       |
|------------|--------|-------------------|
| `vault_id` | uint64 | On-chain vault ID |

**Request Body:** none.

**Responses**

| Status | Description                                                  |
|--------|--------------------------------------------------------------|
| 200    | Secret rotated; returns `Enable2FAResponse`                  |
| 404    | Vault not found, or 2FA not configured for this vault        |
| 500    | Internal server error, or `TOTP_ENCRYPTION_KEY` not configured |

**Example Response (200)**

```json
{
  "method": "totp",
  "secret": "KRSXG5CTMVRXEZLU",
  "provisioning_uri": "otpauth://totp/TTL-Legacy:42?secret=KRSXG5CTMVRXEZLU&issuer=TTL-Legacy"
}
```

---

### DELETE `/api/vaults/{vault_id}/2fa`

Disable 2FA for a vault and delete the stored secret / contact.

**Path Parameters**

| Name       | Type   | Description       |
|------------|--------|-------------------|
| `vault_id` | uint64 | On-chain vault ID |

**Responses**

| Status | Description                                            |
|--------|--------------------------------------------------------|
| 200    | 2FA disabled; returns `{"disabled": true}`             |
| 404    | Vault not found, or 2FA not configured for this vault  |
| 500    | Internal server error                                  |

**Example Response (200)**

```json
{
  "disabled": true
}
```

---

## Scheduler Behaviour

The background scheduler polls every 60 seconds. For each vault with stored preferences it:

1. Fetches the vault's TTL remaining (hours) from the Stellar RPC.
2. Compares against `hours_before_expiry`.
3. Fires reminders on the configured channels according to `frequency`:
   - `once` — fires exactly once when TTL enters the window.
   - `daily` — fires every 24 hours while inside the window.
   - `hourly` — fires every hour while inside the window.

Preferences are stored off-chain in the backend SQLite database and are never written to the Soroban contract.


---

## Configurable Countdown Notifications

### `set_countdown_config`

```rust
set_countdown_config(vault_id: u64, caller: Address, thresholds: Vec<u64>)
```

Sets the countdown notification thresholds for a vault. Only the vault owner may call this. Each threshold is a number of seconds before expiry at which `check_countdown` will emit a `cd_notif` event. Pass an empty vec to disable notifications. Calling this also clears any previously fired threshold flags, so all thresholds will fire fresh on the next countdown cycle.

**Errors**

| Error     | Code | Condition                    |
|-----------|------|------------------------------|
| `NotOwner`| 6    | Caller is not the vault owner|

**Event emitted:** `set_cd` — `(thresholds)`

---

### `get_countdown_config`

```rust
get_countdown_config(vault_id: u64) -> CountdownConfig
```

Returns the countdown config for a vault. If not explicitly set, returns the default thresholds: 604800 (7 days), 259200 (3 days), 86400 (1 day).

---

### `check_countdown`

```rust
check_countdown(vault_id: u64) -> u64
```

Checks the vault's remaining TTL against its configured thresholds and emits a `cd_notif` event for each threshold that has been crossed since the last check-in or config reset. Each threshold fires **at most once** per countdown cycle. Fired flags are cleared automatically when the owner calls `check_in` or `set_countdown_config`.

Can be called by anyone — intended for off-chain keepers, cron jobs, or reminder services.

**Returns** the remaining TTL in seconds (0 if expired or vault is not Locked).

**Event emitted:** `cd_notif` — `(threshold_seconds, ttl_remaining)` — one per newly crossed threshold.

---

### `CountdownConfig` type

```rust
pub struct CountdownConfig {
    /// Sorted descending list of thresholds in seconds.
    /// Default: [604800, 259200, 86400] (7d, 3d, 1d)
    pub thresholds: Vec<u64>,
}
```

### Integration example

An off-chain keeper calls `check_countdown` on a schedule (e.g. every hour). When the vault TTL drops below a threshold, the contract emits a `cd_notif` event. The keeper reads the event and dispatches email/SMS/push reminders via the backend reminder service.

---

## Vault Lifecycle Metrics

### `get_state_transition_count`

```rust
get_state_transition_count(vault_id: u64) -> u64
```

Returns the number of recorded entries in the vault's state transition audit trail (see `get_state_transition_log`), without deserializing the full log. Useful for dashboards that only need to gauge activity level.

---

## Multi-Sig Queries

### `get_multisig_proposal_status`

```rust
get_multisig_proposal_status(vault_id: u64, proposal_id: u64) -> ProposalStatus
```

Returns just the `status` field of a multi-sig proposal, without deserializing the full `MultiSigProposal` struct. Intended for lightweight polling of proposal state.

**Errors**

| Error               | Code | Condition                          |
|----------------------|------|-------------------------------------|
| `ProposalNotFound`   | 44   | No proposal exists with that ID    |

---

### `get_deposit_total`

```rust
get_deposit_total(vault_id: u64) -> Result<i128, ContractError>
```

Lightweight query that returns the current vault balance without deserializing the full `Vault` struct. Reduces compute fees on read-heavy paths where only the deposit total is needed.

**Errors**

| Error            | Code | Condition                  |
|------------------|------|----------------------------|
| `VaultNotFound`  | 3    | The vault does not exist   |
