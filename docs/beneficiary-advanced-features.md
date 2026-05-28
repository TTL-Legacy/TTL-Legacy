# Beneficiary Advanced Features

This document covers four advanced beneficiary features added in issues #494–#497.

---

## #494 — Beneficiary Succession Planning

Allows the vault owner to designate a **successor beneficiary** as a fallback in case the primary beneficiary is unavailable or declines.

### How it works

1. Owner calls `set_succession_plan(vault_id, caller, successor, activation_delay)`.
2. If the primary beneficiary declines or becomes unavailable, the owner calls `activate_succession(vault_id, caller)`.
3. The vault's `beneficiary` field is updated to the successor immediately. An optional `activation_delay` (seconds) is recorded for off-chain enforcement.

### API

```rust
set_succession_plan(vault_id: u64, caller: Address, successor: Address, activation_delay: u64)
activate_succession(vault_id: u64, caller: Address)
get_succession_plan(vault_id: u64) -> Option<SuccessionPlan>
```

### Events

| Topic             | Data                                          |
|-------------------|-----------------------------------------------|
| `suc_set`         | `(caller, successor, activation_delay)`       |
| `suc_act`         | `(old_beneficiary, new_beneficiary, timestamp)` |

### Error codes

| Code | Name                        | Meaning                              |
|------|-----------------------------|--------------------------------------|
| 55   | `SuccessionNotSet`          | No succession plan exists            |
| 56   | `SuccessionAlreadyActivated`| Plan has already been activated      |

---

## #495 — Beneficiary Escrow

Holds released funds in escrow pending explicit beneficiary acceptance, preventing accidental or unwanted transfers.

### How it works

1. Owner calls `create_escrow(vault_id, caller, expiry_seconds)` after depositing funds.
2. Beneficiary calls `accept_escrow` to receive funds, or `reject_escrow` to return them to the owner.
3. If neither action is taken before `expiry_seconds`, anyone can call `expire_escrow` to return funds to the owner.

### API

```rust
create_escrow(vault_id: u64, caller: Address, expiry_seconds: u64)
accept_escrow(vault_id: u64, caller: Address)
reject_escrow(vault_id: u64, caller: Address)
expire_escrow(vault_id: u64)
get_escrow(vault_id: u64) -> Option<EscrowEntry>
```

### Events

| Topic     | Data                                      |
|-----------|-------------------------------------------|
| `esc_cre` | `(beneficiary, amount, expires_at)`       |
| `esc_acc` | `(beneficiary, amount)`                   |
| `esc_rej` | `(beneficiary, amount)`                   |
| `esc_exp` | `(owner, amount)`                         |

### Error codes

| Code | Name                  | Meaning                                  |
|------|-----------------------|------------------------------------------|
| 57   | `EscrowNotFound`      | No escrow entry exists for this vault    |
| 58   | `EscrowAlreadySettled`| Escrow was already accepted or removed   |
| 59   | `EscrowExpired`       | Escrow deadline has passed               |

---

## #496 — Beneficiary Dispute Arbitration

Enables multi-party arbitration for disputes between the vault owner and beneficiary via a designated neutral arbitrator.

### How it works

1. Owner calls `set_arbitrator(vault_id, caller, arbitrator)` to designate a trusted third party.
2. Beneficiary files a dispute via the existing `file_dispute` function.
3. The arbitrator calls `arbitrate_dispute(vault_id, caller, ruling)`:
   - `ruling = true` → funds transferred to beneficiary
   - `ruling = false` → funds returned to owner
4. The dispute is automatically marked `Resolved`.

### API

```rust
set_arbitrator(vault_id: u64, caller: Address, arbitrator: Address)
arbitrate_dispute(vault_id: u64, caller: Address, ruling: bool)
get_arbitration_config(vault_id: u64) -> Option<ArbitrationConfig>
```

### Events

| Topic     | Data                          |
|-----------|-------------------------------|
| `arb_set` | `(owner, arbitrator)`         |
| `arb_rul` | `(arbitrator, ruling, timestamp)` |

### Error codes

| Code | Name                    | Meaning                                  |
|------|-------------------------|------------------------------------------|
| 60   | `ArbitratorNotSet`      | No arbitrator configured for this vault  |
| 61   | `ArbitrationAlreadyRuled` | A ruling has already been issued        |
| 62   | `NotArbitrator`         | Caller is not the configured arbitrator  |

---

## #497 — Beneficiary Notification System

Automatically emits on-chain events and maintains a persistent notification log whenever vault status changes occur.

### How it works

- Notifications are emitted automatically by succession activation, escrow lifecycle events, and arbitration rulings.
- The vault owner or admin can also manually emit a notification via `notify_beneficiary`.
- All notifications are appended to a persistent `NotificationLog` keyed by vault ID.

### API

```rust
notify_beneficiary(vault_id: u64, caller: Address, kind: String)
get_notification_log(vault_id: u64) -> Vec<NotificationEntry>
```

### Event

| Topic     | Data              |
|-----------|-------------------|
| `v_notif` | `(kind, timestamp)` |

### Automatic notifications

| Trigger                  | `kind` value            |
|--------------------------|-------------------------|
| Succession activated     | `succession_activated`  |
| Escrow created           | `escrow_created`        |
| Escrow accepted          | `escrow_accepted`       |
| Escrow rejected          | `escrow_rejected`       |
| Escrow expired           | `escrow_expired`        |
| Arbitration ruled        | `arbitration_ruled`     |
