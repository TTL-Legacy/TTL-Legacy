# Local Soroban Testnet Setup (Developer Onboarding)

End-to-end guide for running a **local Stellar/Soroban network** with
[`stellar-quickstart`](https://github.com/stellar/quickstart), deploying the
TTL-Legacy contracts to it, and pointing the backend at the local node for
development and integration testing.

This is the fastest path to a fully working local stack without touching the
public testnet.

---

## 1. Prerequisites

| Tool | Version | Why | Install |
|------|---------|-----|---------|
| Docker + Docker Compose | recent | Runs `stellar-quickstart`, Postgres, Redis, backend | <https://docs.docker.com/get-docker/> |
| Rust toolchain | 1.85+ | Builds the Soroban contracts to WASM | <https://rustup.rs> |
| `wasm32-unknown-unknown` target | matches toolchain | WASM compilation target | `rustup target add wasm32-unknown-unknown` |
| Stellar CLI | latest | Deploys + invokes contracts, manages keys/networks | `cargo install --locked stellar-cli` |
| `just` (optional) | any | Convenience wrappers for build/test/docker | `cargo install just` |

Verify the WASM target is installed (a missing target is the most common build
failure — see [Troubleshooting](#troubleshooting)):

```bash
rustup target list --installed | grep wasm32
```

---

## 2. Start the local network

The repository ships a `docker-compose.yml` that brings up the whole stack:

```bash
just docker-up
# or, without `just`:
docker-compose up -d
```

This starts four services:

| Service | Container | Port | Purpose |
|---------|----------|------|---------|
| `stellar-quickstart` | `stellar/quickstart:latest` | `8000` (RPC/Horizon/Friendbot), `11626` (Core) | Local standalone Soroban network |
| `db` | `postgres:16-alpine` | `5432` | Backend persistence |
| `redis` | `redis:7-alpine` | `6379` | Shared rate-limit store |
| `backend` | built from `./backend` | `3000` | The TTL-Legacy backend |

`docker-compose.override.yml` (applied automatically) switches the quickstart
node into **standalone** mode (`STANDALONE: "true"`), mounts source for live
reload, and loosens auth — i.e. a throwaway local network with its own ledger
and a funded root account.

> Only want the network, not the backend? You can run quickstart directly:
> ```bash
> docker run --rm -p 8000:8000 -p 11626:11626 -e STANDALONE=true stellar/quickstart:latest
> ```

### Wait for the node to be ready

The compose file defines a healthcheck that polls the Soroban RPC
`getLatestLedger` method. Wait for it to report healthy before deploying:

```bash
docker-compose ps           # `stellar-quickstart` should show "(healthy)"
# or poll the RPC directly:
curl -s -X POST http://localhost:8000/rpc \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}' | jq .result.sequence
```

A non-zero ledger sequence means the network is producing ledgers and is ready
to accept transactions. (If `/rpc` returns `404`, the JSON-RPC path differs on
your quickstart image version — see [Troubleshooting](#rpc-404--wrong-json-rpc-path).)

---

## 3. Register the local network with the Stellar CLI

The deploy scripts use `--network <name>`. Register the local standalone
network once so the CLI knows its RPC URL and passphrase:

```bash
stellar network add local \
  --rpc-url http://localhost:8000/rpc \
  --network-passphrase "Standalone Network ; February 2017"
```

> **Passphrase:** `stellar-quickstart` in standalone mode uses
> `Standalone Network ; February 2017` by default. If you customised the
> passphrase (the compose file sets a `NETWORK_PASSPHRASE` env that some image
> versions honour), use **that** value here and in
> `STELLAR_NETWORK_PASSPHRASE` below. Confirm with `stellar network ls`.

---

## 4. Create and fund a deployer identity

The contracts are deployed from a Stellar CLI identity. Generate one for the
local network and fund it from the quickstart Friendbot:

```bash
# Generate a key managed by the Stellar CLI (no seed phrase to handle).
stellar keys generate deployer --network local

# Fund it from the local Friendbot (standalone accounts start at zero).
curl "http://localhost:8000/friendbot?addr=$(stellar keys address deployer)"

# Confirm the account exists and has a balance.
stellar keys address deployer
```

---

## 5. Build and deploy the contracts

Build the contracts to WASM:

```bash
just build
# or:
./scripts/build.sh
```

This compiles `contracts/ttl_vault` and `contracts/zk_verifier` to
`target/wasm32-unknown-unknown/release/*.wasm`.

Deploy the vault contract to the local network:

```bash
WASM=target/wasm32-unknown-unknown/release/ttl_vault.wasm

CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM" \
  --source deployer \
  --network local)

echo "Deployed vault contract: $CONTRACT_ID"
```

`scripts/deploy_testnet.sh` automates the same flow for the public testnet
(`--network testnet`). To reuse it against your local node, point the
`NETWORK`/`DEPLOYER` variables or register the `testnet` network alias at your
local RPC URL.

Record the deployed contract id — the backend and `environments.toml` both
need it:

```bash
# Persist it for the backend:
echo "CONTRACT_TTL_VAULT=$CONTRACT_ID" >> .env
# (Optionally) record it in environments.toml for deploy tooling.
```

> Initialise the contract if your flow requires it (the vault contract's
> `initialize` entrypoint sets the XLM token + admin). See
> `docs/deployment-guide.md` for the mainnet flow; the same `stellar contract
> invoke` works against `--network local`.

---

## 6. Point the backend at the local node

Copy the example env and switch the network to local:

```bash
cp .env.example .env
```

Edit `.env` (the relevant lines):

```env
# Local standalone network
STELLAR_NETWORK=standalone
STELLAR_RPC_URL=http://localhost:8000/rpc
STELLAR_NETWORK_PASSPHRASE="Standalone Network ; February 2017"

# Contract deployed in step 5
CONTRACT_TTL_VAULT=<paste $CONTRACT_ID here>

# (Optional) Skip the deployed-version check during local dev. The backend
# refuses to start if the on-chain contract version is below
# MIN_CONTRACT_VERSION. When CONTRACT_ID/STELLAR_RPC_URL are unset the check is
# skipped automatically; otherwise set a low floor:
MIN_CONTRACT_VERSION=1
```

The backend reads `SOROBAN_RPC_URL` and falls back to `STELLAR_RPC_URL`; either
works. It POSTs JSON-RPC directly to that URL, so it must be the **full** RPC
endpoint (host **and** path), not just the host.

---

## 7. Run the backend

With `just docker-up` from step 2 the backend already runs in Docker against the
local node. To run it from source instead (useful for debugging), make sure
Postgres + Redis + the quickstart node are up, then:

```bash
# From the repo root, with the .env from step 6 loaded:
cargo run --manifest-path backend/Cargo.toml
```

The backend listens on `http://localhost:3000`. Confirm it is healthy and that
it can reach the contract:

```bash
curl http://localhost:3000/health        # {"status":"ok",...}
curl http://localhost:3000/ready         # database connectivity
```

---

## 8. Verify the full loop

```bash
# 1. The local network is advancing ledgers:
curl -s -X POST http://localhost:8000/rpc \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}' | jq .result.sequence

# 2. The contract is deployed and invokable:
stellar contract invoke \
  --id "$CONTRACT_ID" --source deployer --network local \
  -- <read-only-entrypoint> <args>

# 3. The backend is up and connected:
curl http://localhost:3000/health
```

When you're done, tear the stack down (this discards the standalone ledger
state but keeps the Docker volumes):

```bash
just docker-down
# or: docker-compose down
```

To wipe everything including the persistent volumes:
```bash
docker-compose down -v
```

---

## Troubleshooting

### `wasm32-unknown-unknown` target not found

**Symptom:** `cargo build` / `just build` fails with
`error[E0463]: can't find crate for core` or
`error: target 'wasm32-unknown-unknown' not found`.

**Cause:** The Soroban contracts compile to WebAssembly, which needs the
`wasm32-unknown-unknown` target, but a default `rustup` install only includes
the host target.

**Fix:**
```bash
rustup target add wasm32-unknown-unknown
# If you pin a toolchain via rust-toolchain.toml:
rustup target add wasm32-unknown-unknown --toolchain <toolchain>
rustup target list --installed   # confirm wasm32-unknown-unknown appears
```

### `soroban-sdk` version mismatch

**Symptom:** Build fails with
`failed to select a version for the requirement 'soroban-sdk = "^21.0.0"'`,
or a type/trait mismatch (`expected soroban_sdk::Address, found
soroban_sdk::Address`) from two linked SDK copies.

**Cause:** Cargo resolves a single `soroban-sdk` version for the whole graph. If
the workspace `Cargo.toml` and a contract crate pin different versions, Cargo
either fails to resolve or silently links two incompatible copies.

**Fix:** Declare the version once in `[workspace.dependencies]` of the root
`Cargo.toml` and reference it from each contract crate (`soroban-sdk =
workspace.workspace`), then run `cargo update -p soroban-sdk` so the lockfile
resolves one consistent version. Keep `Cargo.lock` committed so CI and local
builds pick the same versions.

### quickstart never becomes healthy / `getLatestLedger` times out

**Symptom:** `docker-compose ps` shows `stellar-quickstart` as `(unhealthy)` or
`(health: starting)` indefinitely; the RPC returns nothing.

**Cause:** The node can take 20–40s to produce its first ledger, or the
healthcheck is hitting the wrong RPC path for the image version.

**Fix:**
- Give it more time on a cold start: `docker-compose logs -f stellar-quickstart`
  until you see ledger-close messages.
- If the healthcheck keeps failing but the node logs look fine, the RPC path
  differs on your image — see the next item.
- On a constrained machine, raise the healthcheck `start_period` in
  `docker-compose.yml`.

### RPC 404 / wrong JSON-RPC path

**Symptom:** `stellar contract deploy` or the backend reports `404 Not Found`
or `method not found` when calling `getLatestLedger`.

**Cause:** The `stellar/quickstart` image has changed its Soroban JSON-RPC path
across versions. The repo's compose healthcheck uses `http://localhost:8000/rpc`,
but other image versions serve JSON-RPC at `/soroban/rpc` or `/soroban-rpc`.

**Fix:** Discover the working path, then use it for both `stellar network add`
and `STELLAR_RPC_URL`:
```bash
for path in /rpc /soroban/rpc /soroban-rpc; do
  echo "== $path =="
  curl -s -o /dev/null -w "%{http_code}\n" -X POST "http://localhost:8000$path" \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}'
done
```
Use the path that returns `200` (and a JSON body with `.result.sequence`).
Update `STELLAR_RPC_URL`/`SOROBAN_RPC_URL` to the full URL including that path.

### Port already in use (3000 / 5432 / 6379 / 8000 / 11626)

**Symptom:** `docker-compose up` logs `bind: address already in use`.

**Fix:** Stop whatever holds the port (e.g. another Postgres, a previous
quickstart run, or the backend running outside Docker), or remap the host port
in `docker-compose.override.yml`. Common offenders:
- `8000` — another local server; remap to e.g. `"8001:8000"`.
- `5432` — a host Postgres; stop it or remap.

### Friendbot returns an error / account not funded

**Symptom:** `stellar keys generate` succeeds but the account has no balance,
or Friendbot returns `400`/`500`.

**Fix:** In standalone mode the root account is pre-funded but new accounts
need Friendbot. Confirm the URL and that the node is healthy first:
```bash
curl "http://localhost:8000/friendbot?addr=$(stellar keys address deployer)"
```
If Friendbot isn't on `8000`, check the quickstart image docs — some versions
expose it on a different port. A funded account is required to pay for the
deploy transaction.

### Backend refuses to start: "contract version below minimum"

**Symptom:** The backend exits reporting the on-chain contract version is below
`MIN_CONTRACT_VERSION`.

**Cause:** The backend compares the deployed contract's reported version
against `MIN_CONTRACT_VERSION` (default `1`).

**Fix:** For local dev either leave `CONTRACT_ID`/`STELLAR_RPC_URL` unset (the
check is then skipped), set `MIN_CONTRACT_VERSION=1`, or re-deploy a fresh
contract. See `docs/deployment-guide.md` for the versioning contract.

### `docker-compose` vs `docker compose`

Recent Docker installs ship the Compose v2 plugin invoked as
`docker compose` (space, no hyphen). The repo's `Justfile` and scripts use the
legacy `docker-compose` binary. Install whichever your Docker provides; on
systems with only v2, alias it (`alias docker-compose='docker compose'`) or use
`docker compose` directly in place of `docker-compose` throughout this guide.

### Stale state after a quickstart image bump

**Symptom:** After pulling a newer `stellar/quickstart:latest`, deploys fail or
the passphrase/paths changed.

**Fix:** The standalone network is recreated from scratch when its volume is
removed, so re-deploy the contract and re-fund identities:
```bash
docker-compose down -v
docker-compose pull stellar-quickstart
just docker-up
# then repeat steps 3–6 (network add, keys generate, fund, deploy)
```
