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
