use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, Response, StatusCode},
    middleware::Next,
    Json,
};
use chrono::DateTime;
use serde::Deserialize;
use tracing::instrument;

use crate::{
    audit,
    db::{AppState, Db},
    error::AppError,
    handlers::{
        claim_vesting_bonus_handler, get_vesting_bonus_handler, parse_scenario_types,
        simulate_release_handler,
    },
    models::{
        AuditLogEntry, ClaimBonusRequest, ReminderPreferences, SetPreferencesRequest,
        SetSubscriptionRequest, SimulateReleaseQuery, SimulateReleaseResponse, Subscription,
        VaultReleaseHistory,
    },
};

#[derive(Deserialize)]
pub struct RemindersQuery {
    pub include_deleted: Option<bool>,
}

#[instrument(skip(state), fields(vault_id = %vault_id))]
pub async fn list_vault_reminders(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<u64>,
    Query(query): Query<RemindersQuery>,
) -> Result<Json<Vec<ReminderPreferences>>, AppError> {
    let db = &state.db;
    let records = if query.include_deleted.unwrap_or(false) {
        db.all_reminders_including_deleted(vault_id)?
    } else {
        match db.get(vault_id) {
            Ok(p) => vec![p],
            Err(_) => vec![],
        }
    };
    Ok(Json(records))
}

#[instrument(skip(state), fields(vault_id = %vault_id))]
pub async fn delete_preferences(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<u64>,
) -> Result<StatusCode, AppError> {
    state.db.soft_delete_reminder(vault_id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[instrument(skip(state, headers), fields(vault_id = %vault_id))]
pub async fn set_preferences(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<u64>,
    headers: HeaderMap,
    Json(body): Json<SetPreferencesRequest>,
) -> Result<(StatusCode, Json<ReminderPreferences>), AppError> {
    let db = &state.db;
    if body.channels.is_empty() {
        return Err(AppError::InvalidInput("channels must not be empty".into()));
    }
    if body.hours_before_expiry == 0 {
        return Err(AppError::InvalidInput(
            "hours_before_expiry must be > 0".into(),
        ));
    }

    // #825: Idempotency key support
    if let Some(idem_key) = headers.get("idempotency-key").and_then(|v| v.to_str().ok()) {
        if let Some(cached) = db.check_idempotency(idem_key) {
            let cached_prefs: ReminderPreferences =
                serde_json::from_str(&cached.response_body).unwrap();
            return Ok((StatusCode::OK, Json(cached_prefs)));
        }
    }

    let prefs = ReminderPreferences {
        vault_id,
        channels: body.channels,
        hours_before_expiry: body.hours_before_expiry,
        frequency: body.frequency,
        deleted_at: None,
    };
    db.upsert(&prefs)?;

    // Store idempotency record if key was provided
    if let Some(idem_key) = headers.get("idempotency-key").and_then(|v| v.to_str().ok()) {
        let body_json = serde_json::to_string(&prefs).unwrap();
        db.store_idempotency(idem_key, 200, &body_json);
    }

    Ok((StatusCode::OK, Json(prefs)))
}

#[instrument(skip(state), fields(vault_id = %vault_id))]
pub async fn get_preferences(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<u64>,
) -> Result<Json<ReminderPreferences>, AppError> {
    let db = &state.db;
    match db.get(vault_id) {
        Ok(prefs) => Ok(Json(prefs)),
        Err(_e) => Err(AppError::NotFound),
    }
}

// ── Unsubscribe endpoint (#828) ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UnsubscribeQuery {
    pub token: String,
}

#[instrument(skip(state))]
pub async fn unsubscribe(
    State(state): State<Arc<AppState>>,
    Query(query): Query<UnsubscribeQuery>,
) -> Result<(StatusCode, String), AppError> {
    let db = &state.db;
    match db.process_unsubscribe(&query.token) {
        Ok(owner) => Ok((
            StatusCode::OK,
            format!("You ({owner}) have been unsubscribed from reminder emails."),
        )),
        Err(_) => Err(AppError::InvalidInput(
            "Invalid or expired unsubscribe token".into(),
        )),
    }
}

// ── Token-based reminder check-in endpoint (#1286) ──────────────────────────

#[derive(Deserialize)]
pub struct ReminderTokenQuery {
    pub token: String,
}

#[derive(serde::Serialize)]
pub struct ResolveReminderTokenResponse {
    pub vault_id: String,
    pub owner: String,
}

#[instrument(skip(state))]
pub async fn resolve_reminder_token(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ReminderTokenQuery>,
) -> Result<Json<ResolveReminderTokenResponse>, AppError> {
    let db = &state.db;
    match db.resolve_reminder_token(&query.token) {
        Ok((vault_id, owner)) => Ok(Json(ResolveReminderTokenResponse { vault_id, owner })),
        Err(_) => Err(AppError::InvalidInput(
            "Invalid or expired reminder token".into(),
        )),
    }
}

// ── Vault subscription endpoints ─────────────────────────────────────────────

/// POST /api/vaults/:vault_id/subscriptions
///
/// Create or update vault-level notification subscription settings.
#[instrument(skip(state), fields(vault_id = %vault_id))]
pub async fn set_subscription(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<u64>,
    Json(body): Json<SetSubscriptionRequest>,
) -> Result<(StatusCode, Json<Subscription>), AppError> {
    if body.channels.is_empty() {
        return Err(AppError::InvalidInput("channels must not be empty".into()));
    }

    let sub = Subscription {
        vault_id,
        owner: body.owner,
        channels: body.channels,
        frequency: body.frequency,
    };
    state.db.upsert_subscription(&sub)?;
    Ok((StatusCode::OK, Json(sub)))
}

/// DELETE /api/vaults/:vault_id/subscriptions
///
/// Remove vault-level notification subscription settings.
#[instrument(skip(state), fields(vault_id = %vault_id))]
pub async fn delete_subscription(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<u64>,
) -> Result<StatusCode, AppError> {
    state.db.delete_subscription(vault_id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Release Simulator endpoint ────────────────────────────────────────────────

/// GET /api/vaults/:vault_id/simulate-release?scenarios=no_check_ins,consistent_check_ins,missed_check_in_dates&missed_count=2
#[instrument(skip(db), fields(vault_id = %vault_id))]
pub async fn simulate_release(
    State(db): State<Arc<Db>>,
    Path(vault_id): Path<String>,
    Query(query): Query<SimulateReleaseQuery>,
) -> Result<Json<SimulateReleaseResponse>, AppError> {
    let scenarios = parse_scenario_types(query.scenarios.as_deref());
    if scenarios.is_empty() {
        return Err(AppError::InvalidInput(
            "No valid scenarios requested. Use: no_check_ins, consistent_check_ins, missed_check_in_dates".into(),
        ));
    }

    let missed_count = query.missed_count.unwrap_or(1);

    let result = simulate_release_handler(&db.vault_store, &vault_id, scenarios, missed_count)
        .map_err(|_| AppError::NotFound)?;

    Ok(Json(result))
}

// ── Sponsored Release endpoints (#1122) ──────────────────────────────────────

use crate::fee_sponsorship::{SponsoredReleaseRequest, SponsoredReleaseResponse};
use crate::handlers::{
    get_sponsored_release_handler, list_sponsored_releases_handler, sponsored_release_handler,
};

/// POST /api/vaults/:vault_id/sponsored-release
/// Create a sponsored release transaction for a beneficiary.
pub async fn create_sponsored_release(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<String>,
    Json(req): Json<SponsoredReleaseRequest>,
) -> Result<(StatusCode, Json<SponsoredReleaseResponse>), AppError> {
    let result =
        sponsored_release_handler(&state.db.vault_store, Arc::clone(&state.db), &vault_id, req)
            .map_err(|e| AppError::InvalidInput(e))?;

    Ok((StatusCode::CREATED, Json(result)))
}

/// GET /api/vaults/:vault_id/sponsored-release
/// List all sponsored releases for a vault.
pub async fn get_sponsored_releases(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<String>,
) -> Result<Json<Vec<crate::fee_sponsorship::SponsoredRelease>>, AppError> {
    let result = list_sponsored_releases_handler(Arc::clone(&state.db), &vault_id)
        .map_err(|e| AppError::InvalidInput(e))?;

    Ok(Json(result))
}

// --- Issue #1143: Vesting Bonus endpoints ---

/// POST /api/vaults/:vault_id/vesting/claim-bonus
/// Claim vesting bonus on behalf of the beneficiary.
pub async fn claim_vesting_bonus(
    State(db): State<Arc<Db>>,
    Path(vault_id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<ClaimBonusRequest>,
) -> Result<(StatusCode, Json<crate::models::ClaimBonusResponse>), AppError> {
    let result = claim_vesting_bonus_handler(Arc::clone(&db), headers, &vault_id, req)
        .map_err(|e| AppError::InvalidInput(e))?;
    Ok((StatusCode::OK, Json(result)))
}

/// GET /api/vaults/:vault_id/vesting/bonus
/// Return current vesting bonus configuration for the vault.
pub async fn get_vesting_bonus(
    State(db): State<Arc<Db>>,
    Path(vault_id): Path<String>,
) -> Result<Json<crate::models::VestingBonusResponse>, AppError> {
    let result = get_vesting_bonus_handler(Arc::clone(&db), &vault_id)
        .map_err(|e| AppError::InvalidInput(e))?;
    Ok(Json(result))
}

// ── Notification subscriptions (email/SMS/webhook channels) ─────────────────
// Referenced from main.rs's router table but previously never implemented —
// filled in as part of Issue #1174 (idempotency-key support for reminder
// endpoints), which needs a real POST handler to attach that support to.

#[instrument(skip(state, headers), fields(vault_id = %vault_id))]
pub async fn set_subscription(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<u64>,
    headers: HeaderMap,
    Json(body): Json<SetSubscriptionRequest>,
) -> Result<(StatusCode, Json<Subscription>), AppError> {
    let db = &state.db;
    if body.channels.is_empty() {
        return Err(AppError::InvalidInput("channels must not be empty".into()));
    }

    // Issue #1174: idempotency-key support, mirroring set_preferences (#825)
    // exactly so retried/duplicated subscription requests (email or SMS
    // channel changes) can't double-apply.
    if let Some(idem_key) = headers.get("idempotency-key").and_then(|v| v.to_str().ok()) {
        if let Some(cached) = db.check_idempotency(idem_key) {
            let cached_sub: Subscription = serde_json::from_str(&cached.response_body).unwrap();
            return Ok((StatusCode::OK, Json(cached_sub)));
        }
    }

    let sub = Subscription {
        vault_id,
        owner: body.owner,
        channels: body.channels,
        frequency: body.frequency,
    };
    db.upsert_subscription(&sub)?;

    if let Some(idem_key) = headers.get("idempotency-key").and_then(|v| v.to_str().ok()) {
        let body_json = serde_json::to_string(&sub).unwrap();
        db.store_idempotency(idem_key, 200, &body_json);
    }

    Ok((StatusCode::OK, Json(sub)))
}

#[instrument(skip(state), fields(vault_id = %vault_id))]
pub async fn delete_subscription(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<u64>,
) -> Result<StatusCode, AppError> {
    state.db.delete_subscription(vault_id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Vault release history (Issue #1173) ──────────────────────────────────────

/// GET /api/vaults/:vault_id/release-history
/// Combines audit-logged requests against this vault's release-related
/// endpoints with its recorded sponsored-release attempts/completions, so
/// beneficiaries and auditors don't have to parse Stellar transaction
/// history by hand to answer "what release activity has happened here."
#[instrument(skip(state), fields(vault_id = %vault_id))]
pub async fn get_vault_release_history(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<String>,
) -> Result<Json<VaultReleaseHistory>, AppError> {
    let db = &state.db;
    let audit_entries: Vec<AuditLogEntry> = db.get_vault_release_audit_logs(&vault_id)?;
    let sponsored_releases = db.list_sponsored_releases_for_vault(&vault_id)?;

    Ok(Json(VaultReleaseHistory {
        vault_id,
        audit_entries,
        sponsored_releases,
    }))
}

// ── Check-in endpoint (Issue: per-user rate limiting) ────────────────────────

#[derive(serde::Serialize)]
pub struct CheckInResponse {
    pub vault_id: String,
    pub checked_in_at: chrono::DateTime<chrono::Utc>,
    pub message: String,
}

/// POST /api/vaults/:vault_id/check-in
///
/// Records a vault owner check-in, resetting the TTL countdown.  Each vault
/// is limited to **1 check-in per 60 seconds** — enforced by the per-user
/// `checkin_rate_limit_middleware` applied to this route.  Excess requests
/// receive `429 Too Many Requests` with a `Retry-After` header.
#[instrument(fields(vault_id = %vault_id))]
pub async fn check_in(
    Path(vault_id): Path<String>,
) -> Result<(StatusCode, Json<CheckInResponse>), AppError> {
    if vault_id.is_empty() {
        return Err(AppError::InvalidInput("vault_id must not be empty".into()));
    }

    Ok((
        StatusCode::OK,
        Json(CheckInResponse {
            vault_id,
            checked_in_at: chrono::Utc::now(),
            message: "Check-in recorded successfully".to_string(),
        }),
    ))
}
