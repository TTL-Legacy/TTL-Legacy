# Vault Export / Import — Disaster Recovery Procedure

_Issue #1338_

## Overview

Soroban persistent storage entries are subject to TTL (Time-to-Live).  
If a vault owner does not check in, the vault's TTL can lapse and the  
entry may be **archived** (still accessible via `restore_vault` if the  
ledger hasn't been pruned) or **permanently lost** if the state is pruned  
before recovery is attempted.

This document describes how to:

1. **Back up** vault configuration off-chain using `export_vault_config`.
2. **Restore** a vault from an export using `import_vault`.

---

## When to Use This Procedure

| Scenario | Recommended Action |
|---|---|
| Vault state is still live on-chain | Just call `check_in` to reset TTL. No export needed. |
| Vault state is archived but not pruned | Call `restore_vault(vault_id)` — cheaper and preserves the original vault ID. |
| Vault state has been permanently pruned | Follow this disaster-recovery procedure using a previously saved export. |
| Proactive backup before long absence | Export periodically and store the config off-chain for peace of mind. |

---

## Step 1 — Export the Vault Configuration (Owner Action)

Call `export_vault_config(vault_id, caller)` on the contract.  
`caller` must be the vault owner.

```bash
stellar contract invoke \
  --id  <CONTRACT_ID>  \
  --source <OWNER_KEYPAIR_OR_IDENTITY>  \
  --network testnet  \
  -- export_vault_config \
     --vault_id <VAULT_ID>  \
     --caller  <OWNER_ADDRESS>
```

The call returns a `VaultExportConfig` JSON object.  
**Save this output to a secure, off-chain location** (e.g., an encrypted  
file in your password manager, a private IPFS pin, or an encrypted cloud  
backup).

### Export payload fields

| Field | Description |
|---|---|
| `original_vault_id` | ID of the vault this config was exported from |
| `owner` | Vault owner address |
| `beneficiary` | Primary beneficiary address |
| `check_in_interval` | Check-in interval in seconds |
| `token_address` | Token contract used by the vault |
| `beneficiaries` | Multi-beneficiary BPS split (empty = single beneficiary) |
| `metadata` | Short label / IPFS hash |
| `custom_metadata` | Arbitrary custom bytes (max 2 KB) |
| `spending_limit` | Optional per-release spending cap (stroops) |
| `max_deposit_amount` | Optional maximum deposit cap (stroops) |
| `release_condition` | TTLExpiry, OwnerInitiated, or Oracle |
| `exported_at` | Ledger timestamp of export |

### What is NOT included

- **Balance** — funds must be re-deposited after import.
- `last_check_in` / `created_at` / `creation_ledger` — reset on import.
- Passkeys / backup codes — must be re-registered on the new vault.
- Vesting schedules, multi-sig config, withdrawal whitelist — must be  
  re-configured manually on the recreated vault.

---

## Step 2 — (Optional) Attempt `restore_vault` First

If the vault's state entry still exists on-chain (just archived / low TTL),  
try the cheaper option first:

```bash
stellar contract invoke \
  --id  <CONTRACT_ID>  \
  --source <ANY_KEYPAIR>  \
  --network testnet  \
  -- restore_vault \
     --vault_id <VAULT_ID>
```

If this succeeds (no error), the vault is live again with its original ID  
and balance.  **No further steps needed** — just check in to reset the TTL.

If it fails with `VaultNotFound`, the state has been pruned.  Proceed to  
Step 3.

---

## Step 3 — Import from the Export

Pass the saved `VaultExportConfig` to `import_vault`.  
`caller` must match `config.owner`.

```bash
stellar contract invoke \
  --id  <CONTRACT_ID>  \
  --source <OWNER_KEYPAIR_OR_IDENTITY>  \
  --network testnet  \
  -- import_vault \
     --config  '<PASTE_EXPORTED_JSON_HERE>' \
     --caller  <OWNER_ADDRESS>
```

The call returns a **new vault ID**.  Record it — this is your restored vault.

The imported vault:

- Inherits all configuration from the export (beneficiary, interval, token,  
  BPS split, metadata, spending limit, release condition).
- Has a **zero balance** (re-deposit required, see Step 4).
- Has a fresh `last_check_in` timestamp (TTL resets from now).
- Has no passkeys registered (re-register via `add_passkey`).

---

## Step 4 — Re-Deposit Funds

```bash
stellar contract invoke \
  --id  <CONTRACT_ID>  \
  --source <OWNER_KEYPAIR_OR_IDENTITY>  \
  --network testnet  \
  -- deposit \
     --vault_id <NEW_VAULT_ID> \
     --caller  <OWNER_ADDRESS> \
     --amount  <AMOUNT_IN_STROOPS>
```

---

## Step 5 — Re-Register Passkeys (if applicable)

If the original vault used Passkeys for authentication, re-register each  
device passkey on the new vault:

```bash
stellar contract invoke \
  --id  <CONTRACT_ID>  \
  --source <OWNER_KEYPAIR_OR_IDENTITY>  \
  --network testnet  \
  -- add_passkey \
     --vault_id <NEW_VAULT_ID> \
     --caller   <OWNER_ADDRESS> \
     --passkey_hash <HEX_HASH>
```

---

## Step 6 — Verify the Restored Vault

```bash
stellar contract invoke \
  --id  <CONTRACT_ID>  \
  --network testnet  \
  -- get_vault \
     --vault_id <NEW_VAULT_ID>
```

Confirm:

- `owner` and `beneficiary` match the original.
- `check_in_interval` matches.
- `balance` reflects the re-deposit.
- `status` is `Locked`.

---

## Recommended Backup Schedule

| Owner absence expected | Export frequency |
|---|---|
| < 1 × check-in interval | On-demand before planned absence |
| > 1 × check-in interval | Before each absence; automate via backend cron job |
| Long-term holders | Monthly + after any configuration change |

The backend's `dr_backup.sh` script can automate this export and upload to  
a secure off-chain store.  See `scripts/dr_backup.sh` for details.

---

## Security Considerations

- The `VaultExportConfig` does **not** contain private keys or passkey  
  credentials, so it is safe to store encrypted in cloud backup.
- Anyone who obtains the export JSON **cannot** re-create the vault on-chain  
  without holding the owner's signing key.
- Store the export JSON in an encrypted file (e.g., GPG or AES-256).
- Rotate your export after any configuration change (new beneficiary,  
  interval update, etc.).

---

## Disaster Recovery Checklist

```
[ ] Export saved securely (encrypted file / password manager)
[ ] Export timestamp noted and not stale
[ ] Contract ID saved for target network
[ ] Owner keypair / identity available on target network
[ ] Token address confirmed on target network
[ ] Funds available for re-deposit
[ ] Passkey device(s) available for re-registration
```

---

## Related Contract Methods

| Method | Description |
|---|---|
| `export_vault_config(vault_id, caller)` | Export vault config (owner-only) |
| `import_vault(config, caller)` | Re-create vault from export (owner-only) |
| `restore_vault(vault_id)` | Restore archived vault by extending TTL |
| `get_archived_vault_info(vault_id)` | Check if archived snapshot exists |
| `add_passkey(vault_id, caller, hash)` | Register a passkey on the vault |

---

## See Also

- [`docs/ttl-logic.md`](ttl-logic.md) — How Soroban TTL and state archival work
- [`docs/hibernation.md`](hibernation.md) — Alternative: put vault in hibernation before long absence
- [`scripts/dr_backup.sh`](../scripts/dr_backup.sh) — Automated backup script
- [`scripts/dr_restore.sh`](../scripts/dr_restore.sh) — Automated restore script
