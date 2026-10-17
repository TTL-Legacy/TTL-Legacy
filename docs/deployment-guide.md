# TTL-Legacy Deployment Guide

## Overview

This guide covers deploying TTL-Legacy to Stellar mainnet with security best practices, configuration, and rollback procedures.

## Prerequisites

- Rust 1.70+
- Soroban CLI (latest)
- Stellar CLI
- A funded Stellar mainnet account for deployment
- Access to secure key management infrastructure

## Environment Setup

### 1. Key Management

**Never use seed phrases in deployment scripts.** Use Stellar CLI key management:

```bash
# Generate a deployment key (mainnet)
stellar keys generate deployer-mainnet --network mainnet

# Verify the key exists
stellar keys list
```

Store the public key securely. The private key is managed by Stellar CLI in `~/.stellar/keys/`.

### 2. Environment Configuration

Create `.env.mainnet`:

```env
# Network
STELLAR_NETWORK=mainnet
STELLAR_MAINNET_RPC_URL=https://mainnet.sorobanrpc.com

# Deployment
DEPLOYER_IDENTITY=deployer-mainnet
CONTRACT_ADMIN=<admin-public-key>

# Monitoring
LOG_LEVEL=info
SENTRY_DSN=<optional-error-tracking>
```

### 3. Pre-Deployment Checklist

- [ ] Verify contract builds without warnings: `cargo build --package ttl-vault --lib --release`
- [ ] All tests pass: `cargo test --package ttl-vault`
- [ ] Security audit passes: `cargo audit`
- [ ] Code review completed
- [ ] Deployment key funded with XLM for fees (~10 XLM recommended)
- [ ] Admin key is secure and backed up
- [ ] Beneficiary address format validated

## Contract Deployment

### 1. Build for Production

```bash
./scripts/build.sh
```

This produces optimized WASM binaries in `target/wasm32-unknown-unknown/release/`.

### 2. Deploy to Mainnet

```bash
export STELLAR_MAINNET_RPC_URL=https://mainnet.sorobanrpc.com
./scripts/deploy_mainnet.sh
```

The script will:
- Display target network and identity
- Require confirmation (type `mainnet`)
- Deploy the contract
- Output the contract ID

**Save the contract ID** — you'll need it for initialization and frontend configuration.

### 3. Initialize the Contract

After deployment, initialize with admin and token:

```bash
stellar contract invoke \
  --network mainnet \
  --id <CONTRACT_ID> \
  --source-account deployer-mainnet \
  -- initialize \
  --token <STELLAR_ASSET_ADDRESS> \
  --admin <ADMIN_PUBLIC_KEY>
```

For native XLM, use the standard Stellar asset contract address.

## Security Checklist

### Key Management

- [ ] Deployment key stored in Stellar CLI keystore (not in files)
- [ ] Admin key backed up in secure location (hardware wallet or vault)
- [ ] No seed phrases in environment variables or logs
- [ ] Key rotation plan documented
- [ ] Access to deployment credentials restricted to authorized personnel

### Rate Limiting & Monitoring

- [ ] RPC endpoint rate limits configured (if using private RPC)
- [ ] Monitoring alerts set up for contract errors
- [ ] Transaction fee monitoring enabled
- [ ] Unusual activity alerts configured

### Audit Logging

- [ ] All deployments logged with timestamp, deployer, and contract ID
- [ ] Contract initialization events logged
- [ ] Admin actions logged to external system (e.g., Sentry, CloudWatch)
- [ ] Logs retained for 90+ days

### Post-Deployment Validation

- [ ] Contract responds to `get_admin()` call
- [ ] Contract responds to `get_contract_token()` call
- [ ] Test vault creation with small amount
- [ ] Verify beneficiary payout mechanism works
- [ ] Monitor for errors in first 24 hours

## Rollback Procedures

### Scenario 1: Critical Bug Found Before Mainnet Use

If a critical bug is discovered before users interact with the contract:

1. **Pause Operations**: Notify all users to stop using the contract
2. **Deploy Patch**: Fix the bug and deploy a new contract
3. **Migrate State**: If needed, migrate vault data to new contract (requires custom migration logic)
4. **Update Frontend**: Point frontend to new contract ID
5. **Communicate**: Post-mortem and transparency report

### Scenario 2: Bug Found After Mainnet Use

If users have already created vaults:

1. **Assess Impact**: Determine which vaults are affected
2. **Notify Users**: Communicate the issue and mitigation plan
3. **Deploy Patch**: Deploy a new contract with the fix
4. **Provide Migration Path**: Offer users ability to migrate vaults to new contract
5. **Maintain Old Contract**: Keep old contract running for users who choose not to migrate

### Scenario 3: Security Vulnerability

1. **Immediate Action**: Disable affected functionality if possible
2. **Notify Users**: Send urgent security notice
3. **Deploy Patch**: Deploy fixed contract immediately
4. **Audit**: Conduct security audit of the fix
5. **Post-Incident Review**: Document lessons learned

## Troubleshooting

### Deployment Fails with "Insufficient Balance"

The deployment key doesn't have enough XLM. Fund it:

```bash
# Get the public key
stellar keys show deployer-mainnet

# Fund via Stellar testnet faucet (for testnet) or transfer from another account
```

### Contract Initialization Fails

Verify the token address is correct:

```bash
stellar contract read \
  --network mainnet \
  --id <CONTRACT_ID> \
  --key <ADMIN_KEY>
```

### RPC Connection Timeout

Check RPC endpoint availability:

```bash
curl https://mainnet.sorobanrpc.com/health
```

If the endpoint is down, switch to an alternative RPC provider.

### Transaction Rejected with "InvalidContractData"

The contract may already be initialized. Verify:

```bash
stellar contract invoke \
  --network mainnet \
  --id <CONTRACT_ID> \
  -- get_admin
```

If it returns an admin, the contract is already initialized.

## Monitoring & Maintenance

### Daily Checks

- [ ] Contract responds to read-only calls
- [ ] No error spikes in logs
- [ ] RPC endpoint latency normal

### Weekly Checks

- [ ] Review vault creation metrics
- [ ] Check for failed transactions
- [ ] Verify beneficiary payouts are processing

### Monthly Checks

- [ ] Security audit of recent changes
- [ ] Performance review
- [ ] Backup verification

## Kubernetes Deployment (Helm)

The backend service (`backend/`) can be deployed to Kubernetes with a Helm chart. The chart
layout below is the reference for that deployment; it is not yet vendored in this repository.

### Probes

The backend exposes two probe endpoints on port `3000`:

| Endpoint | Probe | Behaviour |
|---|---|---|
| `/health` | `livenessProbe` | Returns `200 OK` if the process is serving. No dependency checks, so it does not flap when the DB or RPC is down. |
| `/ready` | `readinessProbe` | Checks DB and Soroban RPC reachability. Failing pods are removed from the Service until the dependencies recover. |

```yaml
livenessProbe:
  httpGet:
    path: /health
    port: http
  initialDelaySeconds: 5
  periodSeconds: 10
  failureThreshold: 3

readinessProbe:
  httpGet:
    path: /ready
    port: http
  initialDelaySeconds: 5
  periodSeconds: 10
  failureThreshold: 3
```

### Chart Layout

```
charts/ttl-legacy-backend/
├── Chart.yaml
├── values.yaml
└── templates/
    ├── deployment.yaml
    ├── service.yaml
    ├── secret.yaml
    └── hpa.yaml
```

Minimal `values.yaml`:

```yaml
image:
  repository: ghcr.io/<org>/ttl-legacy-backend
  tag: "<release-tag>"
  pullPolicy: IfNotPresent

replicaCount: 2

service:
  type: ClusterIP
  port: 3000

config:
  APP_ENV: production
  LOG_FORMAT: json
  SOROBAN_RPC_URL: https://mainnet.sorobanrpc.com
  CORS_ALLOWED_ORIGINS: https://app.example.com

# Secret values are never stored in values.yaml. Reference an existing Secret instead.
existingSecret: ttl-legacy-backend-secrets

resources:
  requests:
    cpu: 250m
    memory: 256Mi
  limits:
    memory: 512Mi
```

Container spec in `templates/deployment.yaml`:

```yaml
containers:
  - name: backend
    image: "{{ .Values.image.repository }}:{{ .Values.image.tag }}"
    ports:
      - name: http
        containerPort: 3000
    envFrom:
      - configMapRef:
          name: {{ include "ttl-legacy-backend.fullname" . }}
      - secretRef:
          name: {{ .Values.existingSecret }}
    # livenessProbe and readinessProbe: use the /health and /ready blocks from "Probes" above
    resources:
      {{- toYaml .Values.resources | nindent 6 }}
```

### Secret Management

Sensitive values are stored in a Kubernetes Secret and injected as environment variables.
Do not put them in `values.yaml`, a ConfigMap, or the Helm release history.

| Secret key | Purpose |
|---|---|
| `JWT_SECRET` | Signs auth tokens |
| `TOTP_ENCRYPTION_KEY` | Encrypts TOTP seeds at rest |
| `TOTP_ENCRYPTION_KEY_PREVIOUS` | Previous key, kept during rotation (optional) |
| `ADMIN_API_KEY` | Authenticates admin endpoints |
| `REDIS_URL` | Rate-limit store, if `RATE_LIMIT_STORE=redis` (may embed credentials) |

Create the Secret out-of-band, from a secret manager or a sealed/encrypted source
(e.g. External Secrets Operator, Sealed Secrets, or SOPS):

```bash
kubectl create secret generic ttl-legacy-backend-secrets \
  --namespace ttl-legacy \
  --from-literal=JWT_SECRET="$(openssl rand -base64 48)" \
  --from-literal=TOTP_ENCRYPTION_KEY="$(openssl rand -base64 32)" \
  --from-literal=ADMIN_API_KEY="$(openssl rand -hex 32)"
```

Rules:

- [ ] Secrets are created by an operator or a secret manager, never committed to Git
- [ ] RBAC restricts `get`/`list` on Secrets in the namespace to deployers
- [ ] Encryption at rest is enabled for etcd (or the cluster's KMS provider is configured)
- [ ] Secret rotation is performed with a new Secret version and a rolling restart

### Install and Verify

```bash
helm upgrade --install ttl-legacy-backend ./charts/ttl-legacy-backend \
  --namespace ttl-legacy --create-namespace \
  --values values.production.yaml

kubectl -n ttl-legacy rollout status deploy/ttl-legacy-backend
kubectl -n ttl-legacy port-forward svc/ttl-legacy-backend 3000:3000 &
curl -f http://localhost:3000/health
curl -f http://localhost:3000/ready
```

A `/ready` failure after rollout usually means the DB or `SOROBAN_RPC_URL` is unreachable from
the cluster; check the pod logs and the Secret/ConfigMap values before rolling back.

## Support & Escalation

For deployment issues:

1. Check this guide's troubleshooting section
2. Review contract logs and RPC responses
3. Consult [SECURITY.md](../SECURITY.md) for security concerns
4. Open an issue on GitHub with deployment logs (sanitized)

## References

- [Stellar Documentation](https://developers.stellar.org)
- [Soroban Smart Contracts](https://soroban.stellar.org)
- [Security Policy](../SECURITY.md)

## Automated Testnet Deployment (CI/CD)

Every time a GitHub Release is published, the `.github/workflows/deploy-testnet.yml`
workflow automatically:

1. Builds the `ttl_vault` WASM artifact.
2. Deploys the contract to Stellar Testnet using the `deployer-testnet` identity
   (backed by the `STELLAR_TESTNET_ACCOUNT_SECRET` GitHub secret).
3. Records the new contract ID as a workflow output and uploads a deployment artifact
   (retained 90 days).
4. Posts the contract ID as a comment on the GitHub Release page.

### Required GitHub Secrets

| Secret | Description |
|---|---|
| `STELLAR_TESTNET_ACCOUNT_SECRET` | Secret key of the testnet deployer account |
| `TESTNET_ADMIN_SECRET` | Admin secret for smoke test initialization |
| `TESTNET_OWNER_SECRET` | Owner secret for smoke test vault operations |

### Creating a Release

```bash
# Tag a release (triggers the deployment workflow)
git tag -a v1.2.0 -m "Release v1.2.0"
git push origin v1.2.0

# Then on GitHub: create a Release from this tag and publish it.
# The deploy-testnet.yml workflow fires automatically on publish.
```

### Finding the Deployed Contract ID

After the workflow completes:
- Check the **Release page** — the workflow posts a comment with the contract ID.
- Download the `testnet-deployment-<tag>` artifact from the workflow run.
- Or read the `CONTRACT_ID` output from the `deploy-testnet` job in the Actions log.

### Re-deploying Without a New Release

```bash
# Trigger manually via GitHub CLI
gh workflow run deploy-testnet.yml
```

This requires the workflow to also have `workflow_dispatch` as a trigger, which can be
added to the `on:` block when needed.
