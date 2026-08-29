//! JWT access + refresh token issuance with refresh-token rotation — Issue #1177.
//!
//! The backend previously only *validated* JWTs (see `websocket::validate_ws_token`)
//! and had no endpoint that issued them, so every client had to obtain a token some
//! other way and, once it expired, had no way to get a new one short of
//! re-authenticating from scratch. This module adds:
//!
//! - `POST /api/auth/token`   — issue an initial access + refresh token pair.
//! - `POST /api/auth/refresh` — exchange a refresh token for a new pair, rotating
//!   (single-use) the refresh token in the process.
//!
//! Rotation + reuse detection: each refresh token is single-use. Presenting an
//! already-rotated (revoked) refresh token — which can only happen if a token was
//! copied/stolen and the legitimate client already rotated past it — revokes every
//! token in that token's family, forcing a fresh login.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{extract::State, Json};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use uuid::Uuid;

use crate::{
    db::Db,
    error::AppError,
    models::{AuthClaims, LoginRequest, RefreshClaims, RefreshRequest, TokenPairResponse},
};

/// Access tokens are short-lived by design (Issue #1177's whole premise is that
/// clients should rely on refresh, not long-lived access tokens).
const ACCESS_TOKEN_TTL_SECONDS: i64 = 15 * 60; // 15 minutes
const REFRESH_TOKEN_TTL_SECONDS: i64 = 30 * 24 * 60 * 60; // 30 days

fn jwt_secret() -> Vec<u8> {
    match std::env::var("JWT_SECRET") {
        Ok(s) if !s.is_empty() => s.into_bytes(),
        _ => {
            tracing::warn!(
                "JWT_SECRET is not set — using an insecure development-only default. \
                 Set JWT_SECRET before deploying."
            );
            b"insecure-dev-only-jwt-secret".to_vec()
        }
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn issue_access_token(secret: &[u8], sub: &str, vault_ids: Vec<String>) -> Result<String, AppError> {
    let claims = AuthClaims {
        sub: sub.to_string(),
        vault_ids,
        exp: (now_unix() + ACCESS_TOKEN_TTL_SECONDS) as usize,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret))
        .map_err(|e| AppError::Unauthorized(format!("failed to issue access token: {e}")))
}

/// Issues a new refresh token, persists its record, and returns the signed JWT.
fn issue_refresh_token(
    db: &Db,
    secret: &[u8],
    sub: &str,
    family_id: &str,
) -> Result<String, AppError> {
    let jti = Uuid::new_v4().to_string();
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(REFRESH_TOKEN_TTL_SECONDS);

    db.insert_refresh_token(&jti, family_id, sub, expires_at)
        .map_err(|e| AppError::Unauthorized(format!("failed to persist refresh token: {e}")))?;

    let claims = RefreshClaims {
        sub: sub.to_string(),
        jti,
        family_id: family_id.to_string(),
        exp: (now_unix() + REFRESH_TOKEN_TTL_SECONDS) as usize,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret))
        .map_err(|e| AppError::Unauthorized(format!("failed to issue refresh token: {e}")))
}

/// POST /api/auth/token
///
/// Issues an initial access + refresh token pair. In this backend, "login" is
/// establishing that `sub` (a Stellar address, in practice) may act as itself —
/// the actual wallet-signature challenge/verify flow that would authenticate
/// `sub` in production is a separate concern from token issuance/rotation and
/// is out of scope for this issue.
pub async fn login(
    State(db): State<Arc<Db>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<TokenPairResponse>, AppError> {
    if req.sub.trim().is_empty() {
        return Err(AppError::InvalidInput("sub must not be empty".into()));
    }

    let secret = jwt_secret();
    let family_id = Uuid::new_v4().to_string();

    let access_token = issue_access_token(&secret, &req.sub, req.vault_ids.clone())?;
    let refresh_token = issue_refresh_token(&db, &secret, &req.sub, &family_id)?;

    Ok(Json(TokenPairResponse {
        access_token,
        refresh_token,
        expires_in: ACCESS_TOKEN_TTL_SECONDS,
    }))
}

/// POST /api/auth/refresh
///
/// Exchanges a valid, not-yet-used refresh token for a new access + refresh
/// pair, revoking the presented refresh token (rotation). If the presented
/// token was already revoked (i.e. already rotated once before), the entire
/// token family is revoked as a stolen-token countermeasure and the request
/// is rejected — the client must log in again.
pub async fn refresh(
    State(db): State<Arc<Db>>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<TokenPairResponse>, AppError> {
    let secret = jwt_secret();
    let validation = Validation::default();

    let token_data = decode::<RefreshClaims>(
        &req.refresh_token,
        &DecodingKey::from_secret(&secret),
        &validation,
    )
    .map_err(|e| AppError::Unauthorized(format!("invalid refresh token: {e}")))?;
    let claims = token_data.claims;

    let record = db
        .get_refresh_token(&claims.jti)
        .map_err(|e| AppError::Unauthorized(format!("failed to look up refresh token: {e}")))?
        .ok_or_else(|| AppError::Unauthorized("unknown refresh token".into()))?;
    let (family_id, sub, revoked) = record;

    if revoked {
        // Reuse of an already-rotated token — treat the whole family as
        // compromised and force re-authentication.
        let _ = db.revoke_refresh_token_family(&family_id);
        return Err(AppError::Unauthorized(
            "refresh token reuse detected; all sessions in this family have been revoked".into(),
        ));
    }

    db.revoke_refresh_token(&claims.jti)
        .map_err(|e| AppError::Unauthorized(format!("failed to rotate refresh token: {e}")))?;

    let access_token = issue_access_token(&secret, &sub, vec![])?;
    let new_refresh_token = issue_refresh_token(&db, &secret, &sub, &family_id)?;

    Ok(Json(TokenPairResponse {
        access_token,
        refresh_token: new_refresh_token,
        expires_in: ACCESS_TOKEN_TTL_SECONDS,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Json as ExtractJson;

    /// Refresh tokens live in a table created by the sqlx migration set
    /// (migrations/0007_refresh_tokens.sql), which is independent of
    /// Db::migrate's legacy Rust-array migrations. sqlx and rusqlite each
    /// get their own isolated database when pointed at ":memory:", so this
    /// test uses a real (temp) file both migration systems and the test's
    /// own Db connection all agree on.
    async fn test_db() -> (Arc<Db>, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("ttl_auth_test_{}.sqlite", Uuid::new_v4()));
        let path_str = path.to_str().unwrap();

        crate::db::run_sqlx_migrations(path_str).await.unwrap();

        let db = Db::open(path_str).unwrap();
        (Arc::new(db), path)
    }

    #[tokio::test]
    async fn login_then_refresh_rotates_the_refresh_token() {
        let (db, path) = test_db().await;

        let login_resp = login(
            State(Arc::clone(&db)),
            ExtractJson(LoginRequest { sub: "GABC...OWNER".into(), vault_ids: vec!["v1".into()] }),
        )
        .await
        .unwrap();
        let first_refresh = login_resp.0.refresh_token.clone();

        // First use rotates successfully and yields a different refresh token.
        let refreshed = refresh(State(Arc::clone(&db)), ExtractJson(RefreshRequest { refresh_token: first_refresh.clone() }))
            .await
            .unwrap();
        assert_ne!(refreshed.0.refresh_token, first_refresh);

        // Reusing the now-rotated-out original token must be rejected.
        let reuse_result = refresh(State(Arc::clone(&db)), ExtractJson(RefreshRequest { refresh_token: first_refresh })).await;
        assert!(reuse_result.is_err(), "reusing a rotated refresh token must fail");

        // ...and reuse detection must have revoked the whole family: even the
        // *second* (still-fresh) token from the successful rotation above is
        // now unusable.
        let second_refresh_reuse = refresh(
            State(Arc::clone(&db)),
            ExtractJson(RefreshRequest { refresh_token: refreshed.0.refresh_token.clone() }),
        )
        .await;
        assert!(
            second_refresh_reuse.is_err(),
            "reuse detection must revoke the entire token family, not just the reused token"
        );

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn login_rejects_empty_sub() {
        let (db, path) = test_db().await;
        let result = login(State(db), ExtractJson(LoginRequest { sub: "".into(), vault_ids: vec![] })).await;
        assert!(result.is_err());
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn refresh_rejects_unknown_token() {
        let (db, path) = test_db().await;
        let secret = jwt_secret();
        let bogus = issue_refresh_token(&db, &secret, "someone", "some-family").unwrap();
        // Delete the underlying record so the JWT is well-formed but unknown.
        let claims = decode::<RefreshClaims>(&bogus, &DecodingKey::from_secret(&secret), &Validation::default())
            .unwrap()
            .claims;
        db.revoke_refresh_token(&claims.jti).unwrap();

        let result = refresh(State(db), ExtractJson(RefreshRequest { refresh_token: bogus })).await;
        assert!(result.is_err());
        let _ = std::fs::remove_file(path);
    }
}
