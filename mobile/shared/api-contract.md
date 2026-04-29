# TTL-Legacy Mobile API Contract

Base URL: `https://api.ttl-legacy.app/v1` (configurable via env)

## Authentication
All authenticated endpoints require `Authorization: Bearer <jwt>` header.
JWT is obtained via Passkey (WebAuthn) challenge/response flow.

## Endpoints

### Auth
| Method | Path | Description |
|--------|------|-------------|
| POST | `/auth/challenge` | Get WebAuthn challenge |
| POST | `/auth/verify` | Verify passkey assertion, returns JWT |
| POST | `/auth/register` | Register new passkey credential |

### Vaults
| Method | Path | Description |
|--------|------|-------------|
| GET | `/vaults` | List owner's vaults |
| POST | `/vaults` | Create vault |
| GET | `/vaults/{id}` | Get vault detail |
| POST | `/vaults/{id}/checkin` | Check in (extend TTL) |
| POST | `/vaults/{id}/deposit` | Deposit funds |
| POST | `/vaults/{id}/withdraw` | Withdraw funds |
| GET | `/vaults/{id}/ttl` | Get TTL remaining |

### Notifications
| Method | Path | Description |
|--------|------|-------------|
| POST | `/notifications/register` | Register push token |
| DELETE | `/notifications/register` | Unregister push token |

## WebSocket
`wss://api.ttl-legacy.app/v1/ws?vault_id={id}` — real-time vault events

## Models

### Vault
```json
{
  "id": "string",
  "owner": "string",
  "beneficiary": "string",
  "balance": 0,
  "check_in_interval": 0,
  "last_check_in": "ISO8601",
  "ttl_remaining": 0,
  "status": "active|expired|released|paused"
}
```

### AuthChallenge
```json
{ "challenge": "base64url", "expires_at": "ISO8601" }
```

### AuthToken
```json
{ "token": "string", "expires_at": "ISO8601" }
```
