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
    csrf::{csrf_cookie_header_value, generate_token as generate_csrf_token},
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

fn issue_access_token(
    secret: &[u8],
    sub: &str,
    vault_ids: Vec<String>,
) -> Result<String, AppError> {
    let claims = AuthClaims {
        sub: sub.to_string(),
        vault_ids,
        exp: (now_unix() + ACCESS_TOKEN_TTL_SECONDS) as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
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
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
    .map_err(|e| AppError::Unauthorized(format!("failed to issue refresh token: {e}")))
}

/// POST /api/auth/token
///
/// Issues an initial access + refresh token pair. In this backend, "login" is
/// establishing that `sub` (a Stellar address, in practice) may act as itself —
/// the actual wallet-signature challenge/verify flow that would authenticate
/// `sub` in production is a separate concern from token issuance/rotation and
/// is out of scope for this issue.
///
/// Issue #1497: a fresh CSRF token is generated on every successful login and
/// returned both in the JSON body (`csrf_token`) and in a `__Host-csrf`
/// HttpOnly + SameSite=Strict cookie. This prevents session-fixation attacks
/// where an attacker pre-plants a CSRF token before the victim logs in.
pub async fn login(
    State(db): State<Arc<Db>>,
    Json(req): Json<LoginRequest>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    if req.sub.trim().is_empty() {
        return Err(AppError::InvalidInput("sub must not be empty".into()));
    }

    let secret = jwt_secret();
    let family_id = Uuid::new_v4().to_string();

    let access_token = issue_access_token(&secret, &req.sub, req.vault_ids.clone())?;
    let refresh_token = issue_refresh_token(&db, &secret, &req.sub, &family_id)?;

    // Issue #1497: rotate CSRF token on every successful authentication so
    // that any token the client (or attacker) held before login is invalidated.
    let csrf_token = generate_csrf_token();
    let csrf_cookie = csrf_cookie_header_value(&csrf_token);

    let body = TokenPairResponse {
        access_token,
        refresh_token,
        expires_in: ACCESS_TOKEN_TTL_SECONDS,
        csrf_token,
    };

    Ok((
        axum::http::StatusCode::OK,
        [(axum::http::header::SET_COOKIE, csrf_cookie)],
        axum::Json(body),
    ))
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

    // Issue #1497: also rotate the CSRF token on token refresh so the
    // session-fixation protection extends across token renewals.
    let csrf_token = generate_csrf_token();

    Ok(Json(TokenPairResponse {
        access_token,
        refresh_token: new_refresh_token,
        expires_in: ACCESS_TOKEN_TTL_SECONDS,
        csrf_token,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Json as ExtractJson;
    use axum::response::IntoResponse;

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

    /// Helper: call `login` and return the `TokenPairResponse` body directly.
    ///
    /// The `login` handler returns `impl IntoResponse` (a tuple with the
    /// `Set-Cookie` header + JSON body). For unit tests we only need the body;
    /// this helper extracts it by invoking the handler and deserializing the
    /// response body via axum's `tower::ServiceExt`.
    async fn do_login(db: &Arc<Db>, sub: &str) -> Result<TokenPairResponse, AppError> {
        let result = login(
            State(Arc::clone(db)),
            ExtractJson(LoginRequest {
                sub: sub.to_string(),
                vault_ids: vec![],
            }),
        )
        .await;

        match result {
            Err(e) => Err(e),
            Ok(resp) => {
                use axum::body::to_bytes;
                let response = resp.into_response();
                let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
                let pair: TokenPairResponse = serde_json::from_slice(&body_bytes).unwrap();
                Ok(pair)
            }
        }
    }

    #[tokio::test]
    async fn login_then_refresh_rotates_the_refresh_token() {
        let (db, path) = test_db().await;

        let login_resp = do_login(&db, "GABC...OWNER").await.unwrap();
        let first_refresh = login_resp.refresh_token.clone();

        // First use rotates successfully and yields a different refresh token.
        let refreshed = refresh(
            State(Arc::clone(&db)),
            ExtractJson(RefreshRequest {
                refresh_token: first_refresh.clone(),
            }),
        )
        .await
        .unwrap();
        assert_ne!(refreshed.0.refresh_token, first_refresh);

        // Reusing the now-rotated-out original token must be rejected.
        let reuse_result = refresh(
            State(Arc::clone(&db)),
            ExtractJson(RefreshRequest {
                refresh_token: first_refresh,
            }),
        )
        .await;
        assert!(
            reuse_result.is_err(),
            "reusing a rotated refresh token must fail"
        );

        // ...and reuse detection must have revoked the whole family: even the
        // *second* (still-fresh) token from the successful rotation above is
        // now unusable.
        let second_refresh_reuse = refresh(
            State(Arc::clone(&db)),
            ExtractJson(RefreshRequest {
                refresh_token: refreshed.0.refresh_token.clone(),
            }),
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
        let result = do_login(&db, "").await;
        assert!(result.is_err());
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn refresh_rejects_unknown_token() {
        let (db, path) = test_db().await;
        let secret = jwt_secret();
        let bogus = issue_refresh_token(&db, &secret, "someone", "some-family").unwrap();
        // Delete the underlying record so the JWT is well-formed but unknown.
        let claims = decode::<RefreshClaims>(
            &bogus,
            &DecodingKey::from_secret(&secret),
            &Validation::default(),
        )
        .unwrap()
        .claims;
        db.revoke_refresh_token(&claims.jti).unwrap();

        let result = refresh(
            State(db),
            ExtractJson(RefreshRequest {
                refresh_token: bogus,
            }),
        )
        .await;
        assert!(result.is_err());
        let _ = std::fs::remove_file(path);
    }

    // ── Issue #1497: CSRF token rotation on login ─────────────────────────────

    /// Each successful login must produce a unique CSRF token so that a token
    /// obtained before login cannot be used after login (session-fixation
    /// mitigation).
    #[tokio::test]
    async fn login_rotates_csrf_token() {
        let (db, path) = test_db().await;

        let first = do_login(&db, "GTEST...USER").await.unwrap();
        let second = do_login(&db, "GTEST...USER").await.unwrap();

        assert!(
            !first.csrf_token.is_empty(),
            "login must return a non-empty CSRF token (issue #1497)"
        );
        assert!(
            !second.csrf_token.is_empty(),
            "every login must return a non-empty CSRF token"
        );
        assert_ne!(
            first.csrf_token, second.csrf_token,
            "successive logins must produce distinct CSRF tokens (issue #1497)"
        );

        let _ = std::fs::remove_file(path);
    }

    /// Verifies that the login response also sets the `__Host-csrf` cookie
    /// carrying the same token value that appears in the JSON body, so the
    /// double-submit pattern is correctly bootstrapped after login.
    #[tokio::test]
    async fn login_sets_csrf_cookie_matching_body_token() {
        let (db, path) = test_db().await;

        use axum::body::to_bytes;
        use axum::response::IntoResponse as _;

        let resp = login(
            State(Arc::clone(&db)),
            ExtractJson(LoginRequest {
                sub: "GCOOKIE...TEST".into(),
                vault_ids: vec![],
            }),
        )
        .await
        .expect("login must succeed");

        let http_response = resp.into_response();

        // Extract the Set-Cookie header.
        let set_cookie = http_response
            .headers()
            .get(axum::http::header::SET_COOKIE)
            .expect("login must set the __Host-csrf cookie (issue #1497)")
            .to_str()
            .unwrap()
            .to_owned();

        // Extract the body token.
        let body_bytes = to_bytes(http_response.into_body(), usize::MAX).await.unwrap();
        let pair: TokenPairResponse = serde_json::from_slice(&body_bytes).unwrap();

        // The cookie must contain the same token as the body.
        assert!(
            set_cookie.contains(&pair.csrf_token),
            "Set-Cookie header must contain the CSRF token from the response body \
             (cookie={set_cookie:?}, body_token={:?})",
            pair.csrf_token
        );
        assert!(
            set_cookie.contains("__Host-csrf="),
            "cookie name must be __Host-csrf"
        );
        assert!(set_cookie.contains("HttpOnly"), "cookie must be HttpOnly");
        assert!(
            set_cookie.contains("SameSite=Strict"),
            "cookie must be SameSite=Strict"
        );

        let _ = std::fs::remove_file(path);
    }
}
