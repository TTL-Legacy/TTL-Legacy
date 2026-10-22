use std::sync::Arc;

use actix_web::{
    http::StatusCode,
    web::{Data, Json, Path, Query},
    HttpRequest, HttpResponse, Responder,
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

// ── CORS configuration (#1490) ───────────────────────────────────────────────

/// Environment variable holding the comma-separated list of allowed CORS origins.
///
/// Example: `CORS_ALLOWED_ORIGINS=https://app.example.com,https://admin.example.com`
///
/// When unset or empty, no cross-origin requests are allowed (secure default).
/// A wildcard (`*`) is only permitted when credentials are disabled; combining
/// `*` with credentials is rejected because browsers refuse such responses and
/// it would silently expose authenticated endpoints.
pub const CORS_ALLOWED_ORIGINS_ENV: &str = "CORS_ALLOWED_ORIGINS";

/// Parsed CORS configuration derived from the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorsConfig {
    /// Explicit list of allowed origins. Empty means "deny all cross-origin".
    pub allowed_origins: Vec<String>,
    /// Whether the wildcard origin was requested.
    pub allow_any_origin: bool,
    /// Whether credentialed requests are permitted.
    pub allow_credentials: bool,
}

impl CorsConfig {
    /// Build a [`CorsConfig`] from the raw `CORS_ALLOWED_ORIGINS` value.
    ///
    /// Returns an error when a wildcard origin is combined with credentials,
    /// since that combination is both insecure and rejected by browsers.
    pub fn from_env_value(
        raw: Option<&str>,
        allow_credentials: bool,
    ) -> Result<Self, AppError> {
        let mut allowed_origins = Vec::new();
        let mut allow_any_origin = false;

        if let Some(raw) = raw {
            for origin in raw.split(',').map(str::trim).filter(|o| !o.is_empty()) {
                if origin == "*" {
                    allow_any_origin = true;
                } else {
                    allowed_origins.push(origin.to_string());
                }
            }
        }

        if allow_any_origin && allow_credentials {
            return Err(AppError::InvalidInput(
                "CORS_ALLOWED_ORIGINS must not contain '*' when credentials are enabled".into(),
            ));
        }

        Ok(Self {
            allowed_origins,
            allow_any_origin,
            allow_credentials,
        })
    }

    /// Load the CORS configuration from the process environment.
    pub fn from_env(allow_credentials: bool) -> Result<Self, AppError> {
        let raw = std::env::var(CORS_ALLOWED_ORIGINS_ENV).ok();
        Self::from_env_value(raw.as_deref(), allow_credentials)
    }

    /// Returns `true` when the given origin is permitted by this configuration.
    pub fn is_origin_allowed(&self, origin: &str) -> bool {
        if self.allow_any_origin {
            return true;
        }
        self.allowed_origins.iter().any(|o| o == origin)
    }
}

// ── Health & readiness probes (#1489) ────────────────────────────────────────

/// GET /health
///
/// Cheap liveness probe: confirms the process is up and serving requests.
/// Intentionally performs no dependency checks so it stays fast and cannot
/// flap when the DB or RPC is temporarily unreachable.
#[instrument]
pub async fn health() -> StatusCode {
    StatusCode::OK
}

#[derive(serde::Serialize)]
pub struct ReadinessResponse {
    pub status: &'static str,
    pub checks: ReadinessChecks,
}

#[derive(serde::Serialize)]
pub struct ReadinessChecks {
    pub db: &'static str,
    pub rpc: &'static str,
}

/// GET /ready
///
/// Readiness probe: verifies that dependencies (DB connection and Soroban RPC)
/// are reachable before the instance is considered ready to serve traffic.
#[instrument(skip(state))]
pub async fn ready(
    state: Data<Arc<AppState>>,
) -> Result<Json<ReadinessResponse>, AppError> {
    let db_ok = state.db.ping().is_ok();
    let rpc_ok = state.rpc.ping().await.is_ok();

    let checks = ReadinessChecks {
        db: if db_ok { "ok" } else { "unavailable" },
        rpc: if rpc_ok { "ok" } else { "unavailable" },
    };

    if db_ok && rpc_ok {
        Ok(Json(ReadinessResponse {
            status: "ready",
            checks,
        }))
    } else {
        Err(AppError::ServiceUnavailable(Json(ReadinessResponse {
            status: "not_ready",
            checks,
        })))
    }
}

#[derive(Deserialize)]
pub struct RemindersQuery {
    pub include_deleted: Option<bool>,
}

#[instrument(skip(state), fields(vault_id = %vault_id))]
pub async fn list_vault_reminders(
    state: Data<Arc<AppState>>,
    vault_id: Path<u64>,
    query: Query<RemindersQuery>,
) -> Result<Json<Vec<ReminderPreferences>>, AppError> {
    let db = &state.db;
    let records = if query.include_deleted.unwrap_or(false) {
        db.all_reminders_including_deleted(vault_id.into_inner())?
    } else {
        match db.get(vault_id.into_inner()) {
            Ok(p) => vec![p],
            Err(_) => vec![],
        }
    };
    Ok(Json(records))
}

#[instrument(skip(state), fields(vault_id = %vault_id))]
pub async fn delete_preferences(
    state: Data<Arc<AppState>>,
    vault_id: Path<u64>,
) -> Result<StatusCode, AppError> {
    state.db.soft_delete_reminder(vault_id.into_inner())?;
    Ok(StatusCode::NO_CONTENT)
}

#[instrument(skip(state, req), fields(vault_id = %vault_id))]
pub async fn set_preferences(
    state: Data<Arc<AppState>>,
    vault_id: Path<u64>,
    req: HttpRequest,
    body: Json<SetPreferencesRequest>,
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
    if let Some(idem_key) = req
        .headers()
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(cached) = db.check_idempotency(idem_key) {
            let cached_prefs: ReminderPreferences =
                serde_json::from_str(&cached.response_body).unwrap();
            return Ok((StatusCode::OK, Json(cached_prefs)));
        }
    }

    let prefs = ReminderPreferences {
        vault_id: vault_id.into_inner(),
        channels: body.channels.clone(),
        hours_before_expiry: body.hours_before_expiry,
        frequency: body.frequency.clone(),
        deleted_at: None,
    };
    db.upsert(&prefs)?;

    // Store idempotency record if key was provided
    if let Some(idem_key) = req
        .headers()
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
    {
        let body_json = serde_json::to_string(&prefs).unwrap();
        db.store_idempotency(idem_key, 200, &body_json);
    }

    Ok((StatusCode::OK, Json(prefs)))
}

#[instrument(skip(state), fields(vault_id = %vault_id))]
pub async fn get_preferences(
    state: Data<Arc<AppState>>,
    vault_id: Path<u64>,
) -> Result<Json<ReminderPreferences>, AppError> {
    let db = &state.db;
    match db.get(vault_id.into_inner()) {
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
    state: Data<Arc<AppState>>,
    query: Query<UnsubscribeQuery>,
) -> Result<HttpResponse, AppError> {
    let db = &state.db;
    match db.process_unsubscribe(&query.token) {
        Ok(owner) => Ok(HttpResponse::Ok()
            .body(format!("You ({owner}) have been unsubscribed from reminder emails."))),
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
    state: Data<Arc<AppState>>,
    query: Query<ReminderTokenQuery>,
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
    state: Data<Arc<AppState>>,
    vault_id: Path<u64>,
    body: Json<SetSubscriptionRequest>,
) -> Result<(StatusCode, Json<Subscription>), AppError> {
    if body.channels.is_empty() {
        return Err(AppError::InvalidInput("channels must not be empty".into()));
    }

    let sub = Subscription {
        vault_id: vault_id.into_inner(),
        owner: body.owner.clone(),
        channels: body.channels.clone(),
        frequency: body.frequency.clone(),
    };
    state.db.upsert_subscription(&sub)?;
    Ok((StatusCode::OK, Json(sub)))
}

/// DELETE /api/vaults/:vault_id/subscriptions
///
/// Remove vault-level notification subscription settings.
#[instrument(skip(state), fields(vault_id = %vault_id))]
pub async fn delete_subscription(
    state: Data<Arc<AppState>>,
    vault_id: Path<u64>,
) -> Result<StatusCode, AppError> {
    state.db.delete_subscription(vault_id.into_inner())?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/vaults/:vault_id/vesting/claim-bonus
pub async fn claim_vesting_bonus(
    axum::extract::State(db): axum::extract::State<Arc<Db>>,
    axum::extract::Path(vault_id): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
    axum::Json(body): axum::Json<ClaimBonusRequest>,
) -> Result<axum::Json<crate::models::ClaimBonusResponse>, AppError> {
    let rpc_url = std::env::var("SOROBAN_RPC_URL")
        .or_else(|_| std::env::var("STELLAR_RPC_URL"))
        .ok();
    let response =
        claim_vesting_bonus_handler(db, headers, &vault_id, body, rpc_url.as_deref()).await?;
    Ok(axum::Json(response))
}

/// GET /api/vaults/:vault_id/vesting/bonus
pub async fn get_vesting_bonus(
    axum::extract::State(db): axum::extract::State<Arc<Db>>,
    axum::extract::Path(vault_id): axum::extract::Path<String>,
) -> Result<axum::Json<crate::models::VestingBonusResponse>, AppError> {
    let response = get_vesting_bonus_handler(db, &vault_id)?;
    Ok(axum::Json(response))
}

// ── Audit log export endpoint (#1493) ────────────────────────────────────────

#[derive(Deserialize)]
pub struct AuditExportQuery {
    /// Export format: `csv` (default) or `json`.
    pub format: Option<String>,
}

/// GET /vaults/{id}/audit/export?format=csv|json
///
/// Exports the vault's audit trail. Only the vault owner may export. The

/* … truncated 3428 chars — edit only what you need near the top … */

// ── Issue #1852: Beneficiary proof-of-life endpoint ────────────────────────

#[derive(serde::Deserialize)]
pub struct ProofOfLifeSubmitRequest {
    pub beneficiary: String,
    pub proof: String,
    pub validity_seconds: Option<u64>,
}

/// POST /api/vaults/:vault_id/proof-of-life
///
/// Submits a beneficiary proof-of-life attestation. The backend records
/// the proof so that a periodic scheduler can request re-attestation from
/// beneficiaries for high-value vaults.
#[instrument(skip(state), fields(vault_id = %vault_id))]
pub async fn submit_proof_of_life(
    state: Data<Arc<AppState>>,
    vault_id: Path<u64>,
    body: Json<ProofOfLifeSubmitRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let vault_id = vault_id.into_inner();
    let db = &state.db;

    // Verify vault exists
    match db.get(vault_id) {
        Ok(vault) => {
            if vault.beneficiary != body.beneficiary {
                return Err(AppError::InvalidInput(
                    "Only the beneficiary can submit proof of life".into(),
                ));
            }
        }
        Err(_) => return Err(AppError::NotFound),
    }

    // Record proof of life submission
    db.store_proof_of_life(
        vault_id,
        &body.beneficiary,
        &body.proof,
        body.validity_seconds.unwrap_or(86400 * 30),
    )?;

    Ok(Json(serde_json::json!({
        "status": "submitted",
        "vault_id": vault_id,
        "beneficiary": body.beneficiary,
    })))
}
