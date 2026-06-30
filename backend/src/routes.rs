use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;

use crate::{
    db::AppState,
    error::AppError,
    handlers,
    models::{
        GenerateTokenRequest, ReminderPreferences, RevokeTokenRequest,
        SetPreferencesRequest, ShareRequest, ShareTokenResponse, VaultShare, ShareToken,
    },
};

#[derive(Deserialize)]
pub struct RemindersQuery {
    pub include_deleted: Option<bool>,
}

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

pub async fn delete_preferences(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<u64>,
) -> Result<StatusCode, AppError> {
    state.db.soft_delete_reminder(vault_id)?;
    Ok(StatusCode::NO_CONTENT)
}

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

// ── Vault Sharing endpoints ─────────────────────────────────────────────────

/// POST /api/vaults/{vault_id}/share
pub async fn share_vault(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<String>,
    Json(body): Json<ShareRequest>,
) -> Result<Json<VaultShare>, AppError> {
    handlers::share_vault_handler(
        &state.vault_store,
        &state.share_store,
        &state.share_token_store,
        &state.audit_store,
        &vault_id,
        body,
    )
    .map(Json)
    .map_err(|e| AppError::InvalidInput(e))
}

/// POST /api/vaults/{vault_id}/share/tokens
pub async fn generate_share_token(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<String>,
    Json(body): Json<GenerateTokenRequest>,
) -> Result<Json<ShareTokenResponse>, AppError> {
    handlers::generate_share_token_handler(
        &state.vault_store,
        &state.share_store,
        &state.share_token_store,
        &state.audit_store,
        &vault_id,
        body,
    )
    .map(Json)
    .map_err(|e| AppError::InvalidInput(e))
}

/// POST /api/vaults/{vault_id}/share/tokens/revoke
pub async fn revoke_share_token(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<String>,
    Json(body): Json<RevokeTokenRequest>,
) -> Result<Json<ShareToken>, AppError> {
    handlers::revoke_share_token_handler(
        &state.vault_store,
        &state.share_token_store,
        &state.audit_store,
        &vault_id,
        body,
    )
    .map(Json)
    .map_err(|e| AppError::InvalidInput(e))
}

/// GET /api/vaults/{vault_id}/shares
pub async fn list_vault_shares(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<String>,
) -> Result<Json<Vec<VaultShare>>, AppError> {
    let shares = handlers::list_vault_shares_handler(&state.share_store, &vault_id);
    Ok(Json(shares))
}

/// GET /api/vaults/{vault_id}/share/tokens
pub async fn list_share_tokens(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<String>,
) -> Result<Json<Vec<ShareToken>>, AppError> {
    let tokens = handlers::list_share_tokens_handler(&state.share_token_store, &vault_id);
    Ok(Json(tokens))
}

/// GET /api/shared/vaults/{token} — read-only vault access via share token
pub async fn access_shared_vault(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<Json<crate::models::Vault>, AppError> {
    handlers::access_vault_via_share_handler(
        &state.vault_store,
        &state.share_token_store,
        &state.audit_store,
        &token,
    )
    .map(Json)
    .map_err(|e| AppError::InvalidInput(e))
}

#[derive(Deserialize)]
pub struct ExportFormatQuery {
    pub format: Option<String>,
}

/// GET /api/shared/vaults/{token}/export — read-only export via share token
pub async fn access_shared_vault_export(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    Query(query): Query<ExportFormatQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let fmt = query.format.as_deref().unwrap_or("json");
    let result = handlers::access_vault_export_via_share_handler(
        &state.vault_store,
        &state.event_store,
        &state.audit_store,
        &state.share_token_store,
        &token,
        fmt,
    )
    .map_err(|e| AppError::InvalidInput(e))?;

    // Try to parse as JSON; otherwise return as raw text
    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(val) => Ok(Json(val)),
        Err(_) => Ok(Json(serde_json::json!({ "data": result }))),
    }
}

