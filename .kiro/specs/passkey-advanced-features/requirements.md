# Requirements Document

## Introduction

This feature covers four advanced passkey capabilities for the TTL-Legacy vault system (issues #557–#560). The vault contract already supports multi-passkey storage (`VaultPasskeys`), per-passkey expiry (`PasskeyExpiry`), usage tracking (`PasskeyUsage`), and a backend notification service. These four issues extend that foundation with:

- **Passkey Delegation** (#557): allow a vault owner to temporarily grant another address the right to perform check-ins using a specific passkey.
- **Passkey Audit Trail** (#558): record every passkey lifecycle event (add, remove, rotate, use) with a timestamp and actor, queryable on-chain.
- **Passkey Escrow** (#559): hold a passkey in escrow with a designated recovery contact who can release it for emergency vault access.
- **Passkey Expiry Notifications** (#560): notify the vault owner via the backend push-notification service when one or more passkeys are approaching their expiry timestamp.

All four features must emit on-chain events, include comprehensive tests, and be documented.

---

## Glossary

- **Vault**: A time-locked asset container identified by a `u64` vault ID, owned by an `Address`.
- **Owner**: The `Address` that created and controls a vault.
- **Passkey**: A 32-byte hash (`BytesN<32>`) stored in `VaultPasskeys(vault_id)` that authorises check-ins.
- **Passkey_Expiry**: The per-passkey expiry timestamp stored under `DataKey::PasskeyExpiry(vault_id, hash)`.
- **Delegate**: An `Address` temporarily authorised by the Owner to perform check-ins on behalf of the Owner using a specific passkey.
- **Delegation**: A time-bounded grant linking a Passkey to a Delegate, stored under `DataKey::PasskeyDelegation(vault_id, hash)`.
- **Audit_Trail**: An append-only log of passkey lifecycle events stored under `DataKey::PasskeyAuditLog(vault_id)`.
- **Audit_Entry**: A single record in the Audit_Trail containing the operation name, actor address, passkey hash, and timestamp.
- **Escrow**: A holding state for a passkey where the passkey hash is locked and only releasable by the Recovery_Contact.
- **Recovery_Contact**: An `Address` designated by the Owner to release an escrowed passkey in an emergency.
- **Escrow_Record**: The data stored under `DataKey::PasskeyEscrow(vault_id, hash)` describing the escrow state.
- **Notification_Service**: The Rust backend service (`NotificationService`) that schedules and delivers push notifications via FCM.
- **Expiry_Warning_Threshold**: The configurable number of seconds before passkey expiry at which a notification is triggered (default: 86 400 s / 24 h, matching `EXPIRY_WARNING_THRESHOLD`).
- **PasskeyExpiryNotification**: A push notification of type `PasskeyExpiringSoon` sent to the Owner's registered devices.

---

## Requirements

### Requirement 1: Passkey Delegation (Issue #557)

**User Story:** As a vault owner, I want to temporarily delegate check-in authority for a specific passkey to a trusted contact, so that the contact can perform check-ins on my behalf during a defined period without gaining full vault ownership.

#### Acceptance Criteria

1. WHEN the Owner calls `delegate_passkey` with a valid `vault_id`, `passkey_hash`, `delegate` address, and `expires_at` timestamp, THE Contract SHALL store a Delegation record associating the Passkey with the Delegate and the expiry time.
2. WHEN `delegate_passkey` is called, THE Contract SHALL require authorisation from the Owner.
3. IF the `passkey_hash` is not present in `VaultPasskeys(vault_id)`, THEN THE Contract SHALL return `ContractError::PasskeyNotFound`.
4. IF the `expires_at` timestamp is not strictly greater than the current ledger timestamp, THEN THE Contract SHALL return `ContractError::InvalidInterval`.
5. IF the `delegate` address equals the Owner address, THEN THE Contract SHALL return `ContractError::InvalidBeneficiary`.
6. WHEN a Delegate calls `check_in` with a `passkey_hash` for which a valid, non-expired Delegation exists, THE Contract SHALL accept the check-in as if the Owner performed it.
7. WHEN a Delegation's `expires_at` timestamp is reached or exceeded, THE Contract SHALL treat the Delegation as revoked and reject check-in attempts by the Delegate using that Passkey.
8. WHEN the Owner calls `revoke_passkey_delegation` with a valid `vault_id` and `passkey_hash`, THE Contract SHALL remove the Delegation record for that Passkey.
9. WHEN `revoke_passkey_delegation` is called, THE Contract SHALL require authorisation from the Owner.
10. WHEN a Delegation is created via `delegate_passkey`, THE Contract SHALL emit an event with topic `pk_del` containing `(vault_id, passkey_hash, delegate, expires_at)`.
11. WHEN a Delegation is revoked via `revoke_passkey_delegation`, THE Contract SHALL emit an event with topic `pk_del_rev` containing `(vault_id, passkey_hash, delegate)`.
12. THE Contract SHALL provide a `get_passkey_delegation` query that returns the Delegation record for a given `(vault_id, passkey_hash)`, or `None` if no active Delegation exists.

---

### Requirement 2: Passkey Audit Trail (Issue #558)

**User Story:** As a vault owner or auditor, I want every passkey operation (add, remove, rotate, use) to be recorded with a timestamp and actor address, so that I can reconstruct the full history of passkey activity for a vault.

#### Acceptance Criteria

1. WHEN a passkey is added via `add_passkey`, THE Contract SHALL append an Audit_Entry with `operation = "add"`, the caller's address, the passkey hash, and the current ledger timestamp to `PasskeyAuditLog(vault_id)`.
2. WHEN a passkey is removed via `remove_passkey`, THE Contract SHALL append an Audit_Entry with `operation = "remove"`, the caller's address, the passkey hash, and the current ledger timestamp to `PasskeyAuditLog(vault_id)`.
3. WHEN a passkey is rotated via `rotate_passkey`, THE Contract SHALL append two Audit_Entries: one with `operation = "remove"` for the old hash and one with `operation = "add"` for the new hash, both with the current ledger timestamp.
4. WHEN a check-in is performed using a passkey, THE Contract SHALL append an Audit_Entry with `operation = "use"`, the caller's address, the passkey hash, and the current ledger timestamp to `PasskeyAuditLog(vault_id)`.
5. THE Contract SHALL provide a `get_passkey_audit_log` query that returns the full `Vec<PasskeyAuditEntry>` for a given `vault_id`.
6. IF `get_passkey_audit_log` is called for a `vault_id` that does not exist, THEN THE Contract SHALL return `ContractError::VaultNotFound`.
7. THE Audit_Trail SHALL be append-only; THE Contract SHALL provide no function to delete or modify existing Audit_Entries.
8. WHEN any passkey operation appends to the Audit_Trail, THE Contract SHALL emit an event with topic `pk_audit` containing `(vault_id, operation, actor, passkey_hash, timestamp)`.
9. THE Contract SHALL store `PasskeyAuditLog(vault_id)` as a persistent entry with a TTL at least equal to the vault's check-in interval TTL.
10. FOR ALL sequences of passkey operations on a vault, THE number of Audit_Entries returned by `get_passkey_audit_log` SHALL equal the total number of individual passkey operations performed on that vault.

---

### Requirement 3: Passkey Escrow (Issue #559)

**User Story:** As a vault owner, I want to place a passkey in escrow with a designated recovery contact, so that the recovery contact can release the passkey for emergency vault access if I am unable to check in.

#### Acceptance Criteria

1. WHEN the Owner calls `escrow_passkey` with a valid `vault_id`, `passkey_hash`, and `recovery_contact` address, THE Contract SHALL create an Escrow_Record marking the Passkey as escrowed and storing the Recovery_Contact.
2. WHEN `escrow_passkey` is called, THE Contract SHALL require authorisation from the Owner.
3. IF the `passkey_hash` is not present in `VaultPasskeys(vault_id)`, THEN THE Contract SHALL return `ContractError::PasskeyNotFound`.
4. IF the `recovery_contact` address equals the Owner address, THEN THE Contract SHALL return `ContractError::InvalidBeneficiary`.
5. IF a Passkey is already in escrow, THEN THE Contract SHALL return `ContractError::AlreadyInEscrow` when `escrow_passkey` is called for the same `(vault_id, passkey_hash)`.
6. WHILE a Passkey is in escrow, THE Contract SHALL prevent the Owner from using that Passkey for check-in.
7. WHEN the Recovery_Contact calls `release_escrow_passkey` with a valid `vault_id` and `passkey_hash`, THE Contract SHALL remove the Escrow_Record and restore the Passkey to active status.
8. WHEN `release_escrow_passkey` is called, THE Contract SHALL require authorisation from the Recovery_Contact stored in the Escrow_Record.
9. IF `release_escrow_passkey` is called by an address that is not the Recovery_Contact for that Escrow_Record, THEN THE Contract SHALL return `ContractError::NotRecoveryContact`.
10. WHEN the Owner calls `cancel_passkey_escrow` with a valid `vault_id` and `passkey_hash`, THE Contract SHALL remove the Escrow_Record and restore the Passkey to active status.
11. WHEN `cancel_passkey_escrow` is called, THE Contract SHALL require authorisation from the Owner.
12. WHEN an Escrow_Record is created via `escrow_passkey`, THE Contract SHALL emit an event with topic `pk_esc` containing `(vault_id, passkey_hash, recovery_contact)`.
13. WHEN an escrow is released via `release_escrow_passkey`, THE Contract SHALL emit an event with topic `pk_esc_rel` containing `(vault_id, passkey_hash, recovery_contact)`.
14. WHEN an escrow is cancelled via `cancel_passkey_escrow`, THE Contract SHALL emit an event with topic `pk_esc_can` containing `(vault_id, passkey_hash)`.
15. THE Contract SHALL provide a `get_passkey_escrow` query that returns the Escrow_Record for a given `(vault_id, passkey_hash)`, or `None` if the Passkey is not in escrow.

---

### Requirement 4: Passkey Expiry Notifications (Issue #560)

**User Story:** As a vault owner, I want to receive a push notification when one of my passkeys is approaching its expiry timestamp, so that I can rotate or extend the passkey before it expires and disrupts vault access.

#### Acceptance Criteria

1. THE Notification_Service SHALL support a `PasskeyExpiringSoon` notification type in addition to the existing `NotificationType` variants.
2. WHEN the backend scheduler calls `check_passkey_expiry` for a vault, THE Notification_Service SHALL query all passkeys for that vault and their expiry timestamps.
3. WHEN a passkey's remaining time until expiry is less than or equal to the Expiry_Warning_Threshold, THE Notification_Service SHALL schedule a `PasskeyExpiringSoon` notification for the Owner.
4. IF the Owner has disabled expiry warnings in their `NotificationPreferences`, THEN THE Notification_Service SHALL not schedule a `PasskeyExpiringSoon` notification for that Owner.
5. THE Notification_Service SHALL not schedule duplicate `PasskeyExpiringSoon` notifications for the same `(vault_id, passkey_hash)` pair when one is already pending.
6. WHEN a `PasskeyExpiringSoon` notification is delivered, THE notification body SHALL include the vault ID, the passkey hash (truncated to the first 8 hex characters for readability), and the approximate time remaining until expiry in hours.
7. WHEN the Contract's `ping_expiry` function is called and a passkey's remaining TTL is below the Expiry_Warning_Threshold, THE Contract SHALL emit an event with topic `pk_exp_warn` containing `(vault_id, passkey_hash, seconds_remaining)`.
8. IF a passkey has already expired at the time `check_passkey_expiry` is called, THEN THE Notification_Service SHALL schedule a `PasskeyExpired` notification instead of a `PasskeyExpiringSoon` notification.
9. THE Notification_Service SHALL expose a `schedule_passkey_expiry_check` function that accepts a vault ID, owner address, passkey hash, and expiry timestamp, and schedules the appropriate notification.
10. FOR ALL passkeys with a configured expiry, THE Notification_Service SHALL deliver at most one `PasskeyExpiringSoon` notification per passkey per expiry cycle (i.e., between the time the passkey is added or rotated and the time it expires).
