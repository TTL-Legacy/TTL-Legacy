# OpenAPI Contract Tests

## Purpose

Contract tests ensure every path defined in `docs/openapi.yaml` exists in the running
backend and returns a non-404 response. This prevents the spec from drifting silently
from the actual implementation.

## Test File

See `backend/tests/openapi_contract_test.rs` for the implementation.

## Running Locally

```bash
# Start the backend first
docker-compose up -d

# Run only the contract tests
cargo test --package ttl-backend openapi_contract -- --nocapture
```

## What Is Checked

For every path+method combination in `docs/openapi.yaml`, the test:
1. Sends a request to `http://localhost:3000<path>` with minimal valid parameters.
2. Asserts the response status is **not** 404 (the route exists).
3. Does **not** assert on response body — only route existence.

Authenticated endpoints use a pre-generated test JWT. Endpoints that require a real
vault ID use a known test fixture ID.

## Keeping the Spec In Sync

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the required workflow when adding or
changing backend routes.
