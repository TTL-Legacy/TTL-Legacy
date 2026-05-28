use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use crate::{
    db::Db,
    error::AppError,
    models::{ReminderPreferences, SetPreferencesRequest},
};

pub async fn set_preferences(
    State(db): State<Arc<Db>>,
    Path(vault_id): Path<u64>,
    Json(body): Json<SetPreferencesRequest>,
) -> Result<Json<ReminderPreferences>, AppError> {
    if body.channels.is_empty() {
        return Err(AppError::InvalidInput("channels must not be empty".into()));
    }
    if body.hours_before_expiry == 0 {
        return Err(AppError::InvalidInput(
            "hours_before_expiry must be > 0".into(),
        ));
    }
    let prefs = ReminderPreferences {
        vault_id,
        channels: body.channels,
        hours_before_expiry: body.hours_before_expiry,
        frequency: body.frequency,
    };
    db.upsert(&prefs)?;
    Ok(Json(prefs))
}

pub async fn get_preferences(
    State(db): State<Arc<Db>>,
    Path(vault_id): Path<u64>,
) -> Result<Json<ReminderPreferences>, AppError> {
    let prefs = db.get(vault_id)?;
    Ok(Json(prefs))
}
