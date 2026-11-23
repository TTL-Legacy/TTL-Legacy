use crate::audit;
use crate::db::*;
use crate::error::AppError;
use crate::models::*;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use std::sync::Arc;

// ── Passkey Recovery Flow Handlers (#1299) ─────────────────────────────────

use crate::models::{
    GenerateRecoveryCodesRequest, GenerateRecoveryCodesResponse, Passkey, RecoveryCodeSet,
    RecoveryMethod, RecoveryRequest, RecoveryResponse, RegisterPasskeyRequest,
    RegisterPasskeyResponse,
};
use sha2::{Digest, Sha256};

fn generate_recovery_codes(count: usize) -> Vec<String> {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();

    (0..count)
        .map(|_| {
            (0..6)
                .map(|_| {
                    let idx = rng.gen_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect()
        })
        .collect()
}

fn hash_code(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn verify_code(code: &str, hash: &str) -> bool {
    hash_code(code) == hash
}

pub async fn register_passkey_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterPasskeyRequest>,
) -> Result<(StatusCode, Json<RegisterPasskeyResponse>), AppError> {
    let passkey_id = uuid::Uuid::new_v4().to_string();
    let passkey = Passkey {
        passkey_id: passkey_id.clone(),
        owner: body.owner.clone(),
        vault_id: body.vault_id.clone(),
        credential_id: body.credential_id,
        device_name: body.device_name.clone(),
        friendly_name: None,
        registered_at: Utc::now(),
        last_used: None,
        is_backup: body.is_backup.unwrap_or(false),
    };

    let mut store = state.passkey_store.lock().unwrap();
    store.push(passkey);
    drop(store);

    audit::log_action(
        &state.audit_store,
        "register_passkey",
        &body.owner,
        serde_json::json!({
            "vault_id": body.vault_id,
            "device_name": body.device_name,
            "is_backup": body.is_backup.unwrap_or(false),
        }),
    );

    Ok((
        StatusCode::CREATED,
        Json(RegisterPasskeyResponse {
            passkey_id,
            vault_id: body.vault_id,
            device_name: body.device_name,
            registered_at: Utc::now(),
        }),
    ))
}

pub async fn generate_recovery_codes_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GenerateRecoveryCodesRequest>,
) -> Result<Json<GenerateRecoveryCodesResponse>, AppError> {
    let codes = generate_recovery_codes(10);
    let set_id = uuid::Uuid::new_v4().to_string();

    let code_set = RecoveryCodeSet {
        set_id: set_id.clone(),
        owner: body.owner.clone(),
        vault_id: body.vault_id.clone(),
        codes: codes.clone(),
        generated_at: Utc::now(),
        codes_used: 0,
        total_codes: codes.len() as u32,
    };

    let mut recovery_sets = state.recovery_code_set_store.lock().unwrap();
    recovery_sets.push(code_set);
    drop(recovery_sets);

    let mut recovery_codes = state.recovery_code_store.lock().unwrap();
    for (i, code) in codes.iter().enumerate() {
        let code_id = uuid::Uuid::new_v4().to_string();
        let recovery_code = crate::models::RecoveryCode {
            code_id,
            owner: body.owner.clone(),
            vault_id: body.vault_id.clone(),
            code_hash: hash_code(code),
            generated_at: Utc::now(),
            used_at: None,
        };
        recovery_codes.push(recovery_code);
    }
    drop(recovery_codes);

    audit::log_action(
        &state.audit_store,
        "generate_recovery_codes",
        &body.owner,
        serde_json::json!({
            "vault_id": body.vault_id,
            "count": codes.len(),
        }),
    );

    Ok(Json(GenerateRecoveryCodesResponse {
        set_id,
        vault_id: body.vault_id,
        recovery_codes: codes,
        generated_at: Utc::now(),
        note: "Store these codes in a safe place. Each code can only be used once.".to_string(),
    }))
}

pub async fn recover_with_credential_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RecoveryRequest>,
) -> Result<Json<RecoveryResponse>, AppError> {
    let recovery_id = uuid::Uuid::new_v4().to_string();

    match body.recovery_method {
        RecoveryMethod::BackupPasskey => {
            let store = state.passkey_store.lock().unwrap();
            let passkey = store
                .iter()
                .find(|pk| {
                    pk.owner == body.owner
                        && pk.vault_id == body.vault_id
                        && pk.is_backup
                        && pk.credential_id == body.recovery_credential
                })
                .ok_or(AppError::NotFound("Backup passkey not found".to_string()))?;

            let passkey_id = passkey.passkey_id.clone();
            drop(store);

            let mut store = state.passkey_store.lock().unwrap();
            if let Some(pk) = store.iter_mut().find(|pk| pk.passkey_id == passkey_id) {
                pk.last_used = Some(Utc::now());
            }
            drop(store);

            audit::log_action(
                &state.audit_store,
                "recovery_backup_passkey",
                &body.owner,
                serde_json::json!({
                    "vault_id": body.vault_id,
                    "recovery_id": recovery_id,
                }),
            );
        }
        RecoveryMethod::RecoveryCode => {
            let mut recovery_codes = state.recovery_code_store.lock().unwrap();
            let code_entry = recovery_codes
                .iter_mut()
                .find(|rc| {
                    rc.owner == body.owner
                        && rc.vault_id == body.vault_id
                        && verify_code(&body.recovery_credential, &rc.code_hash)
                        && rc.used_at.is_none()
                })
                .ok_or(AppError::NotFound(
                    "Invalid or expired recovery code".to_string(),
                ))?;

            code_entry.used_at = Some(Utc::now());
            drop(recovery_codes);

            audit::log_action(
                &state.audit_store,
                "recovery_code_used",
                &body.owner,
                serde_json::json!({
                    "vault_id": body.vault_id,
                    "recovery_id": recovery_id,
                }),
            );
        }
    }

    Ok(Json(RecoveryResponse {
        recovery_id,
        vault_id: body.vault_id,
        owner: body.owner,
        recovery_method: body.recovery_method,
        authenticated_at: Utc::now(),
    }))
}

pub async fn list_passkeys_handler(
    State(state): State<Arc<AppState>>,
    Path((vault_id, owner)): Path<(String, String)>,
) -> Result<Json<Vec<Passkey>>, AppError> {
    let store = state.passkey_store.lock().unwrap();
    let passkeys: Vec<Passkey> = store
        .iter()
        .filter(|pk| pk.vault_id == vault_id && pk.owner == owner)
        .cloned()
        .collect();
    Ok(Json(passkeys))
}

// ── Passkey friendly_name editing (Issue #2147) ─────────────────────────────

/// Maximum length of a user-provided `friendly_name` (Issue #2147).
pub const MAX_PASSKEY_FRIENDLY_NAME_LEN: usize = 64;

/// Extracts the authenticated user id from the request's `Bearer` access
/// token — the same JWTs issued by `POST /api/auth/token` (see `crate::auth`).
///
/// Returns `AppError::Unauthorized` when the token is missing or invalid.
fn authenticated_user(headers: &axum::http::HeaderMap) -> Result<String, AppError> {
    use jsonwebtoken::{decode, DecodingKey, Validation};

    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("authentication required".into()))?;

    let secret = crate::auth::jwt_secret();
    let claims = decode::<AuthClaims>(
        token,
        &DecodingKey::from_secret(&secret),
        &Validation::default(),
    )
    .map_err(|e| AppError::Unauthorized(format!("invalid access token: {e}")))?
    .claims;

    let sub = claims.sub.trim().to_string();
    if sub.is_empty() {
        return Err(AppError::Unauthorized("authentication required".into()));
    }
    Ok(sub)
}

/// Validates/normalizes a `friendly_name` payload (Issue #2147): trims
/// surrounding whitespace, treats an empty value as `None` (clearing the
/// custom name), and rejects names longer than
/// [`MAX_PASSKEY_FRIENDLY_NAME_LEN`] characters with `AppError::InvalidInput`.
pub fn normalize_friendly_name(raw: Option<&str>) -> Result<Option<String>, AppError> {
    match raw {
        None => Ok(None),
        Some(name) => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else if trimmed.chars().count() > MAX_PASSKEY_FRIENDLY_NAME_LEN {
                Err(AppError::InvalidInput(format!(
                    "friendly_name must be between 1 and {MAX_PASSKEY_FRIENDLY_NAME_LEN} characters"
                )))
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
    }
}

/// `PUT /auth/passkeys/{id}/name` — sets or clears a passkey's
/// `friendly_name` (Issue #2147).
///
/// Requires an authenticated session and verifies the passkey belongs to the
/// authenticated user; other users' passkeys (or nonexistent ids) return
/// `AppError::NotFound` so ownership cannot be probed.
pub async fn update_friendly_name_handler(
    State(state): State<Arc<AppState>>,
    Path(passkey_id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(body): Json<UpdatePasskeyFriendlyNameRequest>,
) -> Result<Json<Passkey>, AppError> {
    let owner = authenticated_user(&headers)?;
    let friendly_name = normalize_friendly_name(body.friendly_name.as_deref())?;

    let mut store = state.passkey_store.lock().unwrap();
    let passkey = store
        .iter_mut()
        .find(|pk| pk.passkey_id == passkey_id && pk.owner == owner)
        .ok_or(AppError::NotFound)?;
    passkey.friendly_name = friendly_name;
    let updated = passkey.clone();
    drop(store);

    audit::log_action(
        &state.audit_store,
        "update_passkey_friendly_name",
        &owner,
        serde_json::json!({
            "passkey_id": passkey_id,
            "friendly_name": updated.friendly_name,
        }),
    );

    Ok(Json(updated))
}

/// Axum router for the passkey management API (Issue #2147).
///
/// * `GET /auth/passkeys/:vault_id/:owner` — list the owner's passkeys.
/// * `PUT /auth/passkeys/:id/name` — set or clear a passkey's `friendly_name`.
pub fn passkey_routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route(
            "/auth/passkeys/:vault_id/:owner",
            axum::routing::get(list_passkeys_handler),
        )
        .route(
            "/auth/passkeys/:id/name",
            axum::routing::put(update_friendly_name_handler),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Passkey Recovery Flow Tests (#1299) ─────────────────────────────────

    #[test]
    fn test_generate_recovery_codes() {
        let codes = generate_recovery_codes(10);
        assert_eq!(codes.len(), 10);
        for code in &codes {
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|c| c.is_alphanumeric()));
        }
        // All codes should be unique
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), codes.len());
    }

    #[test]
    fn test_hash_and_verify_recovery_code() {
        let code = "ABC123";
        let hash = hash_code(code);
        assert!(verify_code(code, &hash));
        assert!(!verify_code("wrong_code", &hash));
    }

    #[test]
    fn test_register_backup_passkey() {
        let passkey = Passkey {
            passkey_id: "pk-1".to_string(),
            owner: "owner1".to_string(),
            vault_id: "vault-1".to_string(),
            credential_id: "cred-abc".to_string(),
            device_name: "iPhone 15".to_string(),
            friendly_name: None,
            registered_at: Utc::now(),
            last_used: None,
            is_backup: true,
        };

        assert!(passkey.is_backup);
        assert_eq!(passkey.owner, "owner1");
        assert_eq!(passkey.vault_id, "vault-1");
        assert!(passkey.last_used.is_none());
    }

    #[test]
    fn test_recovery_code_single_use() {
        let code = RecoveryCode {
            code_id: "rc-1".to_string(),
            owner: "owner1".to_string(),
            vault_id: "vault-1".to_string(),
            code_hash: hash_code("ABC123"),
            generated_at: Utc::now(),
            used_at: None,
        };

        assert!(code.used_at.is_none());

        let mut used_code = code.clone();
        used_code.used_at = Some(Utc::now());
        assert!(used_code.used_at.is_some());
    }

    #[test]
    fn test_multiple_passkeys_per_owner() {
        let vault_id = "vault-1";
        let owner = "owner1";

        let pk1 = Passkey {
            passkey_id: "pk-1".to_string(),
            owner: owner.to_string(),
            vault_id: vault_id.to_string(),
            credential_id: "cred-1".to_string(),
            device_name: "Primary Phone".to_string(),
            friendly_name: None,
            registered_at: Utc::now(),
            last_used: Some(Utc::now()),
            is_backup: false,
        };

        let pk2 = Passkey {
            passkey_id: "pk-2".to_string(),
            owner: owner.to_string(),
            vault_id: vault_id.to_string(),
            credential_id: "cred-2".to_string(),
            device_name: "Security Key".to_string(),
            friendly_name: None,
            registered_at: Utc::now(),
            last_used: None,
            is_backup: true,
        };

        let mut passkeys = vec![pk1, pk2];
        let primary_count = passkeys.iter().filter(|p| !p.is_backup).count();
        let backup_count = passkeys.iter().filter(|p| p.is_backup).count();

        assert_eq!(primary_count, 1);
        assert_eq!(backup_count, 1);
        assert_eq!(passkeys.len(), 2);

        // Test filtering by backup status
        let backups: Vec<_> = passkeys
            .iter()
            .filter(|p| p.is_backup && p.owner == owner)
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].device_name, "Security Key");
    }

    #[test]
    fn test_recovery_code_expiry_tracking() {
        let now = Utc::now();
        let code_set = RecoveryCodeSet {
            set_id: "rcs-1".to_string(),
            owner: "owner1".to_string(),
            vault_id: "vault-1".to_string(),
            codes: vec!["ABC123".to_string(), "DEF456".to_string()],
            generated_at: now,
            codes_used: 0,
            total_codes: 2,
        };

        assert_eq!(code_set.total_codes, 2);
        assert_eq!(code_set.codes_used, 0);
        assert!((Utc::now() - code_set.generated_at).num_seconds() < 2);
    }

    #[test]
    fn test_lost_authenticator_recovery_scenario() {
        // Scenario: Owner loses primary authenticator, needs to recover with backup passkey
        let vault_id = "vault-1";
        let owner = "owner1";

        // Original primary passkey (now lost)
        let _primary = Passkey {
            passkey_id: "pk-primary".to_string(),
            owner: owner.to_string(),
            vault_id: vault_id.to_string(),
            credential_id: "lost-device".to_string(),
            device_name: "Lost iPhone".to_string(),
            friendly_name: None,
            registered_at: Utc::now(),
            last_used: None,
            is_backup: false,
        };

        // Backup passkey available for recovery
        let backup = Passkey {
            passkey_id: "pk-backup".to_string(),
            owner: owner.to_string(),
            vault_id: vault_id.to_string(),
            credential_id: "security-key-123".to_string(),
            device_name: "YubiKey 5".to_string(),
            friendly_name: None,
            registered_at: Utc::now(),
            last_used: None,
            is_backup: true,
        };

        assert!(backup.is_backup);
        assert_eq!(backup.credential_id, "security-key-123");
    }

    #[test]
    fn test_recovery_code_generation_for_new_vault() {
        // Scenario: During vault creation, generate recovery codes
        let codes = generate_recovery_codes(10);

        assert_eq!(codes.len(), 10);

        // Simulate storage of recovery codes
        let mut stored_codes: Vec<RecoveryCode> = Vec::new();
        for (i, code) in codes.iter().enumerate() {
            stored_codes.push(RecoveryCode {
                code_id: format!("rc-{}", i),
                owner: "owner1".to_string(),
                vault_id: "vault-1".to_string(),
                code_hash: hash_code(code),
                generated_at: Utc::now(),
                used_at: None,
            });
        }

        assert_eq!(stored_codes.len(), 10);

        // All codes should be marked as unused
        let unused = stored_codes
            .iter()
            .filter(|rc| rc.used_at.is_none())
            .count();
        assert_eq!(unused, 10);
    }

    #[test]
    fn test_recovery_code_consumption() {
        // Scenario: User uses recovery codes one at a time
        let code1 = "ABC123";
        let code2 = "DEF456";

        let mut code_entry1 = RecoveryCode {
            code_id: "rc-1".to_string(),
            owner: "owner1".to_string(),
            vault_id: "vault-1".to_string(),
            code_hash: hash_code(code1),
            generated_at: Utc::now(),
            used_at: None,
        };

        let mut code_entry2 = RecoveryCode {
            code_id: "rc-2".to_string(),
            owner: "owner1".to_string(),
            vault_id: "vault-1".to_string(),
            code_hash: hash_code(code2),
            generated_at: Utc::now(),
            used_at: None,
        };

        // First use
        code_entry1.used_at = Some(Utc::now());
        assert!(verify_code(code1, &code_entry1.code_hash));
        assert!(code_entry1.used_at.is_some());

        // Second code still available
        assert!(code_entry2.used_at.is_none());
        assert!(verify_code(code2, &code_entry2.code_hash));

        // Second use
        code_entry2.used_at = Some(Utc::now());
        assert!(code_entry2.used_at.is_some());
    }

    // ── Passkey friendly_name editing tests (Issue #2147) ───────────────────

    use crate::auth::jwt_secret;
    use axum::http::{HeaderMap, HeaderValue};
    use jsonwebtoken::{encode, EncodingKey, Header as JwtHeader};
    use std::sync::Mutex;

    fn passkey_test_state(owner: &str, passkey_id: &str) -> Arc<AppState> {
        let passkey = Passkey {
            passkey_id: passkey_id.to_string(),
            owner: owner.to_string(),
            vault_id: "vault-1".to_string(),
            credential_id: "cred-1".to_string(),
            device_name: "YubiKey 5".to_string(),
            friendly_name: None,
            registered_at: Utc::now(),
            last_used: None,
            is_backup: false,
        };
        Arc::new(AppState {
            db: Arc::new(Db::open(":memory:").unwrap()),
            vault_store: create_vault_store(),
            event_store: create_event_store(),
            audit_store: create_audit_store(),
            share_store: create_share_store(),
            share_token_store: create_share_token_store(),
            passkey_store: Arc::new(Mutex::new(vec![passkey])),
            recovery_code_store: Arc::new(Mutex::new(Vec::new())),
            recovery_code_set_store: Arc::new(Mutex::new(Vec::new())),
        })
    }

    fn access_token_headers(sub: &str) -> HeaderMap {
        let claims = AuthClaims {
            sub: sub.to_string(),
            vault_ids: Vec::new(),
            exp: (Utc::now() + chrono::Duration::seconds(3600)).timestamp() as usize,
        };
        let token = encode(
            &JwtHeader::default(),
            &claims,
            &EncodingKey::from_secret(&jwt_secret()),
        )
        .expect("encode access token");
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).expect("valid header value"),
        );
        headers
    }

    #[test]
    fn test_friendly_name_defaults_to_none_for_existing_records() {
        // Passkeys serialized before `friendly_name` existed (Issue #2147)
        // must deserialize with `None` and serialize back with a null value.
        let legacy_json = r#"{
            "passkey_id": "pk-legacy",
            "owner": "owner1",
            "vault_id": "vault-1",
            "credential_id": "cred-1",
            "device_name": "YubiKey 5",
            "registered_at": "2026-01-01T00:00:00Z",
            "last_used": null,
            "is_backup": false
        }"#;
        let passkey: Passkey =
            serde_json::from_str(legacy_json).expect("legacy passkey should deserialize");
        assert_eq!(passkey.friendly_name, None);
        let serialized = serde_json::to_value(&passkey).expect("passkey should serialize");
        assert!(serialized["friendly_name"].is_null());
    }

    #[test]
    fn test_normalize_friendly_name() {
        assert_eq!(normalize_friendly_name(None).unwrap(), None);
        assert_eq!(normalize_friendly_name(Some("")).unwrap(), None);
        assert_eq!(normalize_friendly_name(Some("   ")).unwrap(), None);
        assert_eq!(
            normalize_friendly_name(Some("  My Key  ")).unwrap(),
            Some("My Key".to_string())
        );
        let max = "x".repeat(MAX_PASSKEY_FRIENDLY_NAME_LEN);
        assert_eq!(normalize_friendly_name(Some(&max)).unwrap(), Some(max));
        let too_long = "x".repeat(MAX_PASSKEY_FRIENDLY_NAME_LEN + 1);
        assert!(normalize_friendly_name(Some(&too_long)).is_err());
    }

    #[tokio::test]
    async fn test_update_friendly_name_persists_and_appears_in_list() {
        let state = passkey_test_state("owner1", "pk-1");
        let headers = access_token_headers("owner1");

        let result = update_friendly_name_handler(
            State(Arc::clone(&state)),
            Path("pk-1".to_string()),
            headers,
            Json(UpdatePasskeyFriendlyNameRequest {
                friendly_name: Some("My Key".to_string()),
            }),
        )
        .await;
        let passkey = result.expect("rename succeeds").0;
        assert_eq!(passkey.friendly_name.as_deref(), Some("My Key"));

        let listed = list_passkeys_handler(
            State(Arc::clone(&state)),
            Path(("vault-1".to_string(), "owner1".to_string())),
        )
        .await
        .expect("list succeeds");
        assert!(listed
            .0
            .iter()
            .any(|pk| pk.passkey_id == "pk-1" && pk.friendly_name.as_deref() == Some("My Key")));
    }

    #[tokio::test]
    async fn test_update_friendly_name_clears_with_empty_value() {
        let state = passkey_test_state("owner1", "pk-1");
        let headers = access_token_headers("owner1");

        update_friendly_name_handler(
            State(Arc::clone(&state)),
            Path("pk-1".to_string()),
            headers,
            Json(UpdatePasskeyFriendlyNameRequest {
                friendly_name: Some("  ".to_string()),
            }),
        )
        .await
        .expect("clearing the name succeeds");

        let store = state.passkey_store.lock().unwrap();
        assert_eq!(store[0].friendly_name, None);
    }

    #[tokio::test]
    async fn test_update_friendly_name_requires_authentication() {
        let state = passkey_test_state("owner1", "pk-1");

        let result = update_friendly_name_handler(
            State(Arc::clone(&state)),
            Path("pk-1".to_string()),
            HeaderMap::new(),
            Json(UpdatePasskeyFriendlyNameRequest {
                friendly_name: Some("My Key".to_string()),
            }),
        )
        .await;
        assert!(matches!(result, Err(AppError::Unauthorized(_))));
    }

    #[tokio::test]
    async fn test_update_friendly_name_rejects_cross_user_access() {
        let state = passkey_test_state("owner1", "pk-1");
        let headers = access_token_headers("owner2");

        let result = update_friendly_name_handler(
            State(Arc::clone(&state)),
            Path("pk-1".to_string()),
            headers,
            Json(UpdatePasskeyFriendlyNameRequest {
                friendly_name: Some("Hijacked".to_string()),
            }),
        )
        .await;
        assert!(matches!(result, Err(AppError::NotFound)));

        let store = state.passkey_store.lock().unwrap();
        assert_eq!(store[0].friendly_name, None);
    }

    #[tokio::test]
    async fn test_update_friendly_name_rejects_oversized_name() {
        let state = passkey_test_state("owner1", "pk-1");
        let headers = access_token_headers("owner1");

        let result = update_friendly_name_handler(
            State(Arc::clone(&state)),
            Path("pk-1".to_string()),
            headers,
            Json(UpdatePasskeyFriendlyNameRequest {
                friendly_name: Some("x".repeat(MAX_PASSKEY_FRIENDLY_NAME_LEN + 1)),
            }),
        )
        .await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }
}
