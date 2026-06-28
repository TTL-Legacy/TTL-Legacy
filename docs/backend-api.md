# TTL-Legacy Backend API Reference

This document provides a comprehensive reference for the TTL-Legacy backend server endpoints. The backend is a Rust-based Axum web service that manages reminder preferences, notifications, and system health checks for vault owners.

---

## Base URL

```
http://localhost:3000/api
```

**Environment Variables**:

- `ALLOWED_ORIGINS` — Comma-separated CORS origins (e.g., `http://localhost:3001,https://example.com`)
- `DATABASE_URL` — Database connection string (defaults to in-memory `:memory:`)
- `DATABASE_POOL_MIN` — Minimum connections (default: 5)
- `DATABASE_POOL_MAX` — Maximum connections (default: 20)
- `DATABASE_POOL_TIMEOUT_SECS` — Connection timeout (default: 30)

---

## Authentication

### Per-Endpoint Auth

Most endpoints **require idempotency key support** for safe retries:

```bash
curl -X POST http://localhost:3000/api/vaults/123/reminder-preferences \
  -H "idempotency-key: req-12345-67890" \
  -H "Content-Type: application/json" \
  -d '{...}'
```

**Idempotency Key Header**: `idempotency-key` (optional, RFC 7231)

- Format: Any string (UUID recommended)
- Used to deduplicate retries
- Responses are cached for 24 hours

---

## System Endpoints

### Health Check

```
GET /health
```

**Purpose**: Check if the backend service is running.

**Authentication**: None required

**Response** (200 OK):

```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

**Error Responses**: None (always 200)

---

### Readiness Check

```
GET /ready
```

**Purpose**: Check if the backend service and database are ready to serve traffic.

**Authentication**: None required

**Response** (200 OK):

```json
{
  "status": "ok",
  "version": "0.1.0",
  "database": "connected"
}
```

**Response** (503 Service Unavailable):

If database is unavailable:

```json
{
  "status": "unavailable",
  "reason": "database_connection_failed"
}
```

**Used by**: Kubernetes liveness/readiness probes

---

## Reminder Preferences API

### Set or Update Reminder Preferences

```
POST /vaults/{vault_id}/reminder-preferences
```

**Purpose**: Configure reminder notifications for a vault owner.

**Path Parameters**:

| Name | Type | Description |
|------|------|-------------|
| `vault_id` | integer (u64) | Stellar contract vault ID |

**Request Headers**:

| Header | Required | Value | Example |
|--------|----------|-------|---------|
| `Content-Type` | ✓ | `application/json` | `application/json` |
| `idempotency-key` | ✗ | UUID string | `550e8400-e29b-41d4-a716-446655440000` |

**Request Body**:

```json
{
  "channels": ["email", "push"],
  "hours_before_expiry": 24,
  "frequency": "daily"
}
```

**Request Fields**:

| Field | Type | Required | Description | Constraints |
|-------|------|----------|-------------|-------------|
| `channels` | array | ✓ | Notification channels | Non-empty; values: `"email"`, `"sms"`, `"push"` |
| `hours_before_expiry` | integer | ✓ | Hours before vault expiry to notify | > 0 |
| `frequency` | string | ✓ | Notification frequency | One of: `"once"`, `"daily"`, `"weekly"`, `"hourly"`, `"monthly"` |

**Response** (200 OK):

```json
{
  "vault_id": 123,
  "channels": ["email", "push"],
  "hours_before_expiry": 24,
  "frequency": "daily"
}
```

**Response Fields**:

| Field | Type | Description |
|-------|------|-------------|
| `vault_id` | integer | Vault ID from the request |
| `channels` | array | Stored notification channels |
| `hours_before_expiry` | integer | Hours before expiry |
| `frequency` | string | Notification frequency |

**Error Responses**:

| Status | Code | Message | Cause |
|--------|------|---------|-------|
| 400 | `InvalidInput` | `"channels must not be empty"` | Empty channels array |
| 400 | `InvalidInput` | `"hours_before_expiry must be > 0"` | hours_before_expiry is 0 or negative |
| 500 | `DatabaseError` | `"Failed to store preferences"` | Database write failed |

**Idempotency Behavior**:

- If `idempotency-key` is provided, the same request will return the same response (cached)
- Duplicate requests within 24 hours return the cached response (200 OK, same body)
- Cache expires after 24 hours

---

### Get Reminder Preferences

```
GET /vaults/{vault_id}/reminder-preferences
```

**Purpose**: Retrieve the current reminder preferences for a vault.

**Path Parameters**:

| Name | Type | Description |
|------|------|-------------|
| `vault_id` | integer (u64) | Stellar contract vault ID |

**Authentication**: None required

**Response** (200 OK):

```json
{
  "vault_id": 123,
  "channels": ["email", "push"],
  "hours_before_expiry": 24,
  "frequency": "daily"
}
```

**Error Responses**:

| Status | Code | Message | Cause |
|--------|------|---------|-------|
| 404 | `NotFound` | `"Preferences not found for vault"` | No preferences set for this vault |
| 500 | `DatabaseError` | `"Failed to retrieve preferences"` | Database read failed |

---

## Unsubscribe Endpoint

### Unsubscribe from Reminders

```
GET /unsubscribe
```

**Purpose**: Unsubscribe a vault owner from all reminder emails using a token.

**Query Parameters**:

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `token` | string | ✓ | Unsubscribe token (from email) |

**Authentication**: None required

**Response** (200 OK):

```
You (GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX) have been unsubscribed from reminder emails.
```

**Error Responses**:

| Status | Code | Message | Cause |
|--------|------|---------|-------|
| 400 | `InvalidInput` | `"Invalid or expired unsubscribe token"` | Token not found or expired |
| 500 | `DatabaseError` | `"Failed to process unsubscribe"` | Database operation failed |

**Token Format**: Tokens are generated by the backend and included in reminder emails. They expire after 30 days.

---

## CORS Configuration

The backend supports Cross-Origin Resource Sharing (CORS) for browser-based frontend applications.

**Configuration**:

```bash
# Set allowed origins (comma-separated)
export ALLOWED_ORIGINS="http://localhost:3001,https://app.example.com"
```

**Allowed Methods**:

- `GET`
- `POST`
- `PUT`
- `DELETE`
- `OPTIONS`

**Allowed Headers**: All (`*`)

---

## Error Handling

### Standard Error Response Format

```json
{
  "error": "InvalidInput",
  "message": "channels must not be empty",
  "status_code": 400
}
```

### Error Codes Reference

| Code | HTTP Status | Description | Recovery |
|------|-------------|-------------|----------|
| `InvalidInput` | 400 | Malformed request body or invalid parameters | Fix the request and retry |
| `NotFound` | 404 | Resource does not exist | Verify vault_id or token |
| `Unauthorized` | 401 | Authentication required or failed | Include required auth headers |
| `Forbidden` | 403 | Not authorized for this resource | Use valid credentials |
| `DatabaseError` | 500 | Database operation failed | Retry later; contact support if persistent |
| `InternalError` | 500 | Unexpected server error | Retry later; contact support |

---

## Request/Response Examples

### Example 1: Create Preferences with Idempotency

**Request**:

```bash
curl -X POST http://localhost:3000/api/vaults/42/reminder-preferences \
  -H "idempotency-key: req-001-2024-06-27" \
  -H "Content-Type: application/json" \
  -d '{
    "channels": ["email"],
    "hours_before_expiry": 48,
    "frequency": "once"
  }'
```

**Response** (200 OK):

```json
{
  "vault_id": 42,
  "channels": ["email"],
  "hours_before_expiry": 48,
  "frequency": "once"
}
```

**Retry (same request)**:

```bash
# Same idempotency-key → returns cached response (200 OK)
curl -X POST http://localhost:3000/api/vaults/42/reminder-preferences \
  -H "idempotency-key: req-001-2024-06-27" \
  -H "Content-Type: application/json" \
  -d '{
    "channels": ["email"],
    "hours_before_expiry": 48,
    "frequency": "once"
  }'
```

Response is identical (cached).

---

### Example 2: Retrieve Preferences

**Request**:

```bash
curl http://localhost:3000/api/vaults/42/reminder-preferences
```

**Response** (200 OK):

```json
{
  "vault_id": 42,
  "channels": ["email"],
  "hours_before_expiry": 48,
  "frequency": "once"
}
```

---

### Example 3: Unsubscribe from Email

**Request** (from email link):

```
http://localhost:3000/api/unsubscribe?token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

**Response** (200 OK):

```
You (GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX) have been unsubscribed from reminder emails.
```

---

## Database Schema

The backend uses SQLite to persist reminder preferences and unsubscribe tokens.

### Tables

#### `reminder_preferences`

```sql
CREATE TABLE reminder_preferences (
    vault_id INTEGER PRIMARY KEY,
    channels TEXT NOT NULL,          -- JSON array: ["email", "push", ...]
    hours_before_expiry INTEGER NOT NULL,
    frequency TEXT NOT NULL,         -- "daily", "weekly", ...
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

#### `idempotency_cache`

```sql
CREATE TABLE idempotency_cache (
    key TEXT PRIMARY KEY,
    status_code INTEGER,
    response_body TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME
);
```

#### `unsubscribe_tokens`

```sql
CREATE TABLE unsubscribe_tokens (
    token TEXT PRIMARY KEY,
    owner TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME
);
```

---

## Rate Limiting

Currently, rate limiting is **not implemented**. In production, consider:

- **Per-IP rate limits**: 100 requests/minute
- **Per-vault rate limits**: 10 preference updates/hour
- **Token bucket algorithm** for adaptive rate limiting

---

## Pagination

Currently, no endpoints support pagination. For future expansion:

- Use `?limit=20&offset=0` query parameters
- Return `_links` with `next` and `prev` URLs
- Limit range: 1-100 items

---

## Versioning

API version is embedded in responses:

```json
{
  "version": "0.1.0"
}
```

**Breaking changes** will increment the major version (e.g., `/api/v2/...`).

---

## Deployment Notes

### Port Configuration

The backend binds to `0.0.0.0:3000` (hardcoded). To change, modify `src/main.rs`:

```rust
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
```

### Database Persistence

By default, the backend uses an in-memory SQLite database (`:memory:`), which is cleared on restart.

**For persistent storage**, set:

```bash
export DATABASE_URL="/path/to/ttl-legacy.db"
```

### Health Checks

**Kubernetes probes**:

```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 3000
  initialDelaySeconds: 5
  periodSeconds: 10

readinessProbe:
  httpGet:
    path: /ready
    port: 3000
  initialDelaySeconds: 5
  periodSeconds: 10
```

---

## Troubleshooting

### "Database connection failed"

- Check `DATABASE_URL` environment variable
- Verify database file exists and is readable
- Check pool configuration (`DATABASE_POOL_*` env vars)

### "Idempotency key not working"

- Ensure `idempotency-key` header is set on both request and retry
- Wait at most 24 hours for cache to expire
- Clear the cache by restarting the service

### "CORS errors in browser"

- Verify `ALLOWED_ORIGINS` includes your frontend URL
- Check that `Content-Type` is `application/json`
- Ensure no typos in domain (must match exactly)

---

## Support

For issues or questions:

- **GitHub Issues**: https://github.com/TTL-Legacy/TTL-Legacy/issues
- **Documentation**: https://github.com/TTL-Legacy/TTL-Legacy/blob/main/docs/
