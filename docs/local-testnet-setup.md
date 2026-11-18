# Local Stellar network setup

How to run a local Stellar node with `stellar/quickstart`, build and deploy the contracts, and run the backend against it. Every command here comes from this repository (`docker-compose.yml`, `docker-compose.override.yml`, the `Justfile`, `scripts/`, `CONTRIBUTING.md`).

> **Status of this guide.** It was written by reading those files. The steps were **not run**: the machine it was written on has no Docker, Rust or Stellar CLI. Each step says *Not tried*, and *Read from the code* where a claim comes from a file. If a step fails for you, please fix the guide.

For the build errors that are not specific to a local node, see [Troubleshooting in CONTRIBUTING.md](../CONTRIBUTING.md#troubleshooting).

## What runs

`docker-compose up -d` starts four containers (`docker-compose.yml`):

| Service | Container | Port | Health check |
|---|---|---|---|
| `db` (PostgreSQL 16) | `ttl-legacy-db` | 5432 | `pg_isready` |
| `redis` (Redis 7) | `ttl-legacy-redis` | 6379 | `redis-cli ping` |
| `stellar-quickstart` | `stellar-quickstart` | 8000, 11626 | a `getLatestLedger` call to `http://localhost:8000/rpc` |
| `backend` | `ttl-legacy-backend` | 3000 | `curl -f http://localhost:3000/ready` |

`docker-compose.override.yml` is applied automatically. It sets `STANDALONE: "true"` on `stellar-quickstart`, builds the backend's `development` target, and mounts `backend/migrations` into the database's init folder.

The backend waits for `db`, `redis` and `stellar-quickstart` to be healthy before it starts.

## Prerequisites

From the table in `CONTRIBUTING.md`:

| Tool | Version |
|---|---|
| Rust | 1.70+ (`rustup`) |
| Node.js | 18+ |
| Docker | 24+ |
| Stellar CLI | latest (`cargo install stellar-cli --locked`) |
| `just` | optional (`cargo install just`) |

The contracts build for `wasm32-unknown-unknown`. If that target is missing, see the first entry of the troubleshooting section in `CONTRIBUTING.md`.

## 1. Start the stack

```bash
cp .env.example .env      # or: just env-setup
docker-compose up -d      # or: just docker-up
docker-compose ps         # wait until the four services are healthy
```

*Not tried.*

Check the local node with the same call the health check uses:

```bash
curl -s -X POST http://localhost:8000/rpc \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}'
```

A healthy node returns a `result.sequence` greater than 0. *Not tried.*

Check the backend:

```bash
curl http://localhost:3000/ready
```

`CONTRIBUTING.md` uses `/health`. The backend source has both paths (`/health` and `/ready`), and the compose health check uses `/ready`. *Not tried.*

## 2. Build the contracts

```bash
just build
# or, without just:
./scripts/build.sh
```

Both run `cargo build --target wasm32-unknown-unknown --release` for `contracts/ttl_vault` and `contracts/zk_verifier`. `scripts/build.sh` also reads `.env` and prints a warning for each of `STELLAR_NETWORK`, `STELLAR_RPC_URL`, `REMINDER_EMAIL_API_KEY` and `REMINDER_SMS_API_KEY` that is not set; the build itself does not need them. The vault is written to `target/wasm32-unknown-unknown/release/ttl_vault.wasm`. *Not tried.*

## 3. Deploy to the local node

`just deploy-testnet` runs `./scripts/deploy_testnet.sh`, which builds and then runs:

```bash
stellar contract deploy --wasm target/wasm32-unknown-unknown/release/ttl_vault.wasm --source deployer --network testnet
```

*Read from the code:* the network name `testnet` and the source account `deployer` are fixed in the script. Nothing in the repository defines where the Stellar CLI's `testnet` network points. `CONTRIBUTING.md` says this deploys to the local node at `localhost:8000`, but that depends on how `testnet` is configured in your Stellar CLI; by default it is the public testnet. Before you deploy:

1. Check where the network named `testnet` points (`stellar network --help` lists the commands of your CLI version). For the local node the RPC endpoint is `http://localhost:8000/rpc`, the one the health check calls. The network passphrase must be the one your node uses, which is not set in this repository for the standalone mode (`docker-compose.yml` sets `NETWORK_PASSPHRASE` to the public testnet's).
2. Check that an identity named `deployer` exists and is funded on that network (`stellar keys --help`).

*Not tried.* Because of these two points, treat a successful deploy to the public testnet as "it did not reach the local node".

Afterwards the script prints `CONTRACT_ID`. It then tries to write the address into `environments.toml` with `set_contract_address` (`scripts/deploy_utils.sh`). *Read from the code:* that function looks for a `[testnet]` section with a `contract_ttl_vault` key, and the current `environments.toml` has neither, so nothing is written. Copy the printed id by hand: `CONTRACT_TTL_VAULT=<id>` in `.env`, as the script says. To redeploy without the confirmation prompt, use `just deploy-testnet-force`.

## 4. Run the backend against the local node

The backend reads the RPC endpoint from `SOROBAN_RPC_URL`, falling back to `STELLAR_RPC_URL`, and the contract from `CONTRACT_ID` (`backend/src/main.rs`, `fetch_contract_version`). When either is missing it logs a warning and skips the contract version check, so it still starts.

`CONTRIBUTING.md` says to set `STELLAR_RPC_URL=http://localhost:8000` in `.env`. *Read from the code:* the `backend` service in `docker-compose.yml` has an `environment:` list and no `env_file:`, and that list has no `STELLAR_RPC_URL` or `CONTRACT_ID`. So a value in `.env` does not reach the backend container. Either:

- add `SOROBAN_RPC_URL` and `CONTRACT_ID` under `backend.environment` in `docker-compose.override.yml`. Inside the compose network the node is `stellar-quickstart`, so the URL is `http://stellar-quickstart:8000`, not `localhost`; or
- run the backend outside Docker, where `localhost:8000` is correct.

Then `docker-compose up -d backend`. *Not tried.*

## 5. Run the frontend

```bash
cd frontend
npm install
npm run dev
```

The dev server runs at `http://localhost:5173`. See [frontend-development.md](frontend-development.md) for its variables (`VITE_STELLAR_RPC_URL=http://localhost:8000` for the local node).

*Read from the code:* `just frontend-dev` checks for a `web/` folder and exits with `Frontend directory not found`. This repository has `frontend/`, not `web/`, so use the commands above. *Not tried.*

## Troubleshooting

| Problem | What the files say |
|---|---|
| `backend` stays in `starting` or never starts | It depends on `db`, `redis` and `stellar-quickstart` being healthy. The node's health check has `start_period: 30s` and up to 15 retries every 10 s, so the first start can take minutes. `docker-compose ps` and `docker-compose logs stellar-quickstart` show which service is not ready. |
| The node's health check keeps failing | The check runs `curl` and `jq` inside the `stellar/quickstart` image, and calls `getLatestLedger` on `/rpc`. Run the `curl` call from step 1 yourself to see the answer. |
| An image or SDK version problem after an update | `stellar/quickstart` is pinned to `latest`, so a new pull can change the node. The contracts use `soroban-sdk` 21.0.0 (`contracts/ttl_vault/Cargo.toml`) and the backend image builds on `rust:1.78-slim`. For a `soroban-sdk` version mismatch, follow its entry in `CONTRIBUTING.md`. |
| Port already in use | The stack publishes 5432, 6379, 8000, 11626 and 3000. Stop what uses them, or change the left side of the `ports:` entries in `docker-compose.yml`. |
| Start again from a clean node or database | `docker-compose down` removes the containers (`just docker-down`). The data lives in the `postgres_data` and `quickstart_data` volumes, which `docker-compose down` keeps. `docker-compose down -v` removes them too. |
| The deploy reaches the wrong network | See step 3. |
| `just frontend-dev` fails | See step 5. |
