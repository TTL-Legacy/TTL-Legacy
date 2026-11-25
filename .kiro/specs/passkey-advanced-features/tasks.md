# Tasks — Passkey Advanced Features (Issues #557–#560)

Derived from `requirements.md`. Implementation target: Soroban `ttl_vault`
contract (`contracts/ttl_vault/src`) + Rust backend notification service
(`backend/src`).

## Contract (`contracts/ttl_vault/src/types.rs` + `lib.rs`)

- [x] 1. Add `StorageKey` variants: `PasskeyDelegation(u64, BytesN<32>)`,
      `PasskeyAuditLog(u64)`, `PasskeyEscrow(u64, BytesN<32>)`.
- [x] 2. Add contract types: `PasskeyDelegation`, `PasskeyAuditEntry`,
      `PasskeyEscrowRecord`.
- [x] 3. Add topic symbols: `pk_del`, `pk_audit`, `pk_esc` (+ inline `Symbol::new`
      for the >9-char `pk_del_rev`, `pk_esc_rel`, `pk_esc_can`, `pk_exp_warn`).
- [x] 4. Add `ContractError::AlreadyInEscrow` (= 163).
- [x] 5. Issue #557 — Delegation: `delegate_passkey`, `revoke_passkey_delegation`,
      `get_passkey_delegation`; integrate into `check_in` via
      `is_valid_passkey_delegation`; events `pk_del` / `pk_del_rev`.
- [x] 6. Issue #558 — Audit trail: `append_passkey_audit` helper +
      `get_passkey_audit_log`; append `add`/`remove`/`rotate`(remove+add)/`use`
      entries; event `pk_audit`; `VaultNotFound` for unknown vault.
- [x] 7. Issue #559 — Escrow: `escrow_passkey`, `release_escrow_passkey`,
      `cancel_passkey_escrow`, `get_passkey_escrow`; block check-in while
      escrowed; events `pk_esc` / `pk_esc_rel` / `pk_esc_can`.
- [x] 8. Issue #560 — Contract side: `ping_expiry` emits `pk_exp_warn`
      `(vault_id, passkey_hash, seconds_remaining)` per expiring passkey.

## Backend (`backend/src`)

- [x] 9. Issue #560 — `NotificationType::PasskeyExpiringSoon` +
      `PasskeyExpired`.
- [x] 10. Add `passkey_hash: Option<String>` to `ScheduledNotification` and
       `PasskeyExpiryInput` model.
- [x] 11. `notification_content` arms for the new types (vault id, passkey hash
       truncated to 8 chars, hours remaining).
- [x] 12. `schedule_passkey_expiry_check(vault_id, owner, passkey_hash, expiry)`
       + `check_passkey_expiry(vault_id, owner, &[PasskeyExpiryInput])`;
       respect `expiry_warning_enabled`; dedupe pending
       `PasskeyExpiringSoon` per `(vault_id, passkey_hash)`.
- [x] 13. Keep existing `match`es exhaustive (`templates.rs`, `schedule_immediate`).

Note: per instructions, code is written but not compiled or tested.
