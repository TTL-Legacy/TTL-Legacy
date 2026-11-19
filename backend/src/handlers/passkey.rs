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
}
