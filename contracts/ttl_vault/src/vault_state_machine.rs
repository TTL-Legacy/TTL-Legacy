//! Vault lifecycle state machine (issue #2110).
//!
//! The `TtlVaultContract` guards its state transitions (`Active → Paused`,
//! `Active → Released`, `Active → Archived`, …) inline, scattered across
//! `pause_vault`, `resume_vault`, `cancel_vault`, `trigger_release_internal`,
//! `archive_vault` and `prune_vault`. This module centralises those guards
//! behind a single [`transition`] function so the allowed-transitions table
//! lives in one place and the call sites only perform the actual mutation.
//!
//! ## Design
//!
//! * [`VaultState`] is the *observable* lifecycle state, derived from the
//!   vault's [`ReleaseStatus`] field and its per-vault pause flag. `Locked`
//!   splits into [`VaultState::Active`] / [`VaultState::Paused`]; the other
//!   statuses map one-to-one. [`VaultState::Archived`] and
//!   [`VaultState::Pruned`] are terminal *targets* only — archiving/pruning
//!   copies/removes the entry but leaves `status` untouched, so [`state_of`]
//!   never yields them.
//! * [`TransitionContext`] carries the extra booleans the release guard needs
//!   (admin freeze, owner lock, prior release attempt, raw pause flag). For
//!   non-release transitions the default (all-false) context is sufficient.
//! * [`transition`] validates `(from, to)` against the table below and returns
//!   the **exact** [`ContractError`] the inline guards used to return, in the
//!   same order, so this is a behaviour-preserving refactor.
//!
//! ```text
//! Active    → Paused        (pause)
//! Paused    → Active        (resume)
//! Active    → Released      (release; extra guards via context)
//! Active|Paused → Cancelled (owner cancel; EmergencyFrozen → VaultFrozen)
//! terminal  → Archived      (archive; Active|Paused → NotExpired)
//! terminal  → Pruned        (prune;   Active|Paused → NotExpired)
//! ```

use crate::{ContractError, ReleaseStatus};

/// Observable lifecycle state of a vault.
///
/// Derived from a vault's `ReleaseStatus` plus its `is_paused` flag by
/// [`state_of`]. `Archived` and `Pruned` are terminal *targets* that
/// [`state_of`] never produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaultState {
    /// `status == Locked && !is_paused` — the normal operating state.
    Active,
    /// `status == Locked && is_paused` — owner-paused, operations blocked.
    Paused,
    /// `status == Released` — funds released to the beneficiary.
    Released,
    /// `status == Cancelled` — owner-cancelled, funds refunded.
    Cancelled,
    /// `status == EmergencyFrozen` — admin emergency freeze.
    EmergencyFrozen,
    /// `status == Failed` — release attempted but the transfer failed.
    Failed,
    /// Terminal target: vault snapshot moved to archived storage.
    Archived,
    /// Terminal target: vault entry removed from storage.
    Pruned,
}

/// Extra state needed to evaluate the release transition. The release guard
/// inspects several independent flags in addition to the status; this struct
/// bundles them so [`transition`] can mirror the original guard order.
///
/// For non-release transitions a default (all-`false`) context is sufficient.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransitionContext {
    /// Raw `vault.is_paused` flag. Checked first by the release guard so a
    /// paused vault (including one paused before being cancelled) reports
    /// `VaultPaused` rather than a status-based error.
    pub is_paused: bool,
    /// Admin per-vault freeze (`StorageKey::VaultFrozen`). Blocks release with
    /// `VaultFrozen`.
    pub is_admin_frozen: bool,
    /// Owner lock (issue #1853). Blocks beneficiary-triggered release with
    /// `VaultOwnerLocked`.
    pub is_owner_locked: bool,
    /// A release was already attempted (`StorageKey::ReleaseAttempted`).
    /// Blocks a second release with `AlreadyReleased`.
    pub release_attempted: bool,
}

/// Derive the observable [`VaultState`] from a vault's status and pause flag.
///
/// `Locked` is split into [`VaultState::Active`] / [`VaultState::Paused`];
/// every other status maps one-to-one. The terminal targets
/// ([`VaultState::Archived`] / [`VaultState::Pruned`]) are never produced here
/// because archiving and pruning do not change `status`.
#[must_use]
pub fn state_of(status: &ReleaseStatus, is_paused: bool) -> VaultState {
    match (status, is_paused) {
        (ReleaseStatus::Locked, true) => VaultState::Paused,
        (ReleaseStatus::Locked, false) => VaultState::Active,
        (ReleaseStatus::Released, _) => VaultState::Released,
        (ReleaseStatus::Cancelled, _) => VaultState::Cancelled,
        (ReleaseStatus::EmergencyFrozen, _) => VaultState::EmergencyFrozen,
        (ReleaseStatus::Failed, _) => VaultState::Failed,
    }
}

/// Validate a state transition, returning the [`ContractError`] the previous
/// inline guards returned (and in the same order), or `Ok(())` if allowed.
///
/// Call sites derive `from` via [`state_of`], then perform the actual mutation
/// only when this returns `Ok(())`. The guard order for `→ Released` mirrors
/// `trigger_release_internal` exactly:
///
/// 1. `is_paused` → [`ContractError::VaultPaused`]
/// 2. `is_admin_frozen` → [`ContractError::VaultFrozen`]
/// 3. `is_owner_locked` → [`ContractError::VaultOwnerLocked`]
/// 4. `release_attempted` → [`ContractError::AlreadyReleased`]
/// 5. status not `Active` → [`ContractError::AlreadyReleased`]
pub fn transition(
    from: VaultState,
    to: VaultState,
    context: TransitionContext,
) -> Result<(), ContractError> {
    use VaultState::{Active, Archived, Cancelled, EmergencyFrozen, Paused, Pruned, Released};

    match to {
        // Release is the most heavily guarded transition. Mirrors the exact
        // guard order in `trigger_release_internal` so the error codes match.
        Released => {
            if context.is_paused {
                return Err(ContractError::VaultPaused);
            }
            if context.is_admin_frozen {
                return Err(ContractError::VaultFrozen);
            }
            if context.is_owner_locked {
                return Err(ContractError::VaultOwnerLocked);
            }
            if context.release_attempted {
                return Err(ContractError::AlreadyReleased);
            }
            match from {
                Active => Ok(()),
                _ => Err(ContractError::AlreadyReleased),
            }
        }
        // Pause / resume: only a Locked vault (active or already paused) may
        // toggle its pause flag. Any terminal status is already released.
        Paused | Active => match from {
            Active | Paused => Ok(()),
            _ => Err(ContractError::AlreadyReleased),
        },
        // Owner-initiated cancel: a frozen vault cannot be cancelled, and only
        // a Locked vault may be cancelled.
        Cancelled => match from {
            EmergencyFrozen => Err(ContractError::VaultFrozen),
            Active | Paused => Ok(()),
            _ => Err(ContractError::AlreadyReleased),
        },
        // Archive / prune: only a terminal vault (released, cancelled, frozen
        // or failed) may be archived or pruned. An active/paused vault still
        // holds owner funds and must expire first.
        Archived | Pruned => match from {
            Active | Paused => Err(ContractError::NotExpired),
            _ => Ok(()),
        },
        // `Failed` and `EmergencyFrozen` are never reached as a guarded *target*
        // (those statuses are set unconditionally by release-failure / emergency
        // paths), so any remaining target is treated as already-released.
        _ => Err(ContractError::AlreadyReleased),
    }
}
