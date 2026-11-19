use crate::db::*;
use crate::models::*;
use chrono::Utc;

// ── Task 3: Sharing & Collaboration ──────────────────────────────────────────

const DEFAULT_TOKEN_EXPIRY_SECONDS: u64 = 604800; // 7 days

/// POST /vaults/{id}/share
pub fn share_vault_handler(
    store: &VaultStore,
    share_store: &ShareStore,
    token_store: &ShareTokenStore,
    audit_store: &AuditStore,
    vault_id: &str,
    request: ShareRequest,
) -> Result<VaultShare, String> {
    // Verify vault exists
    let vault = store
        .lock()
        .unwrap()
        .get(vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    let share = VaultShare {
        share_id: uuid::Uuid::new_v4().to_string(),
        vault_id: vault_id.to_string(),
        shared_with: request.shared_with.clone(),
        permission: request.permission,
        created_at: Utc::now(),
    };

    add_vault_share(share_store, share.clone());

    // Audit log
    append_audit_entry(
        audit_store,
        "vault_shared",
        &vault.owner,
        serde_json::json!({
            "vault_id": vault_id,
            "share_id": share.share_id,
            "shared_with": request.shared_with,
            "permission": share.permission,
        }),
    );

    Ok(share)
}

/// POST /vaults/{id}/share/tokens
pub fn generate_share_token_handler(
    store: &VaultStore,
    share_store: &ShareStore,
    token_store: &ShareTokenStore,
    audit_store: &AuditStore,
    vault_id: &str,
    request: GenerateTokenRequest,
) -> Result<ShareTokenResponse, String> {
    let vault = store
        .lock()
        .unwrap()
        .get(vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    let permission = request.permission.unwrap_or(SharePermission::ViewOnly);
    let expires_at = Utc::now()
        + chrono::Duration::seconds(
            request
                .expiry_seconds
                .unwrap_or(DEFAULT_TOKEN_EXPIRY_SECONDS) as i64,
        );

    // Create a VaultShare entry (reuses existing share infrastructure)
    let share = VaultShare {
        share_id: uuid::Uuid::new_v4().to_string(),
        vault_id: vault_id.to_string(),
        shared_with: request.shared_with.clone(),
        permission: permission.clone(),
        created_at: Utc::now(),
    };
    add_vault_share(share_store, share.clone());

    // Generate the access token
    let token = ShareToken {
        token: uuid::Uuid::new_v4().to_string(),
        share_id: share.share_id.clone(),
        vault_id: vault_id.to_string(),
        shared_with: request.shared_with,
        permission,
        created_at: Utc::now(),
        expires_at,
        revoked: false,
    };
    add_share_token(token_store, token.clone());

    let access_url = format!("/api/shared/vaults/{}", token.token);

    // Audit log
    append_audit_entry(
        audit_store,
        "share_token_generated",
        &vault.owner,
        serde_json::json!({
            "vault_id": vault_id,
            "share_id": share.share_id,
            "token": token.token,
            "expires_at": token.expires_at,
        }),
    );

    Ok(ShareTokenResponse {
        share,
        token,
        access_url,
    })
}

/// POST /vaults/{id}/share/tokens/revoke
pub fn revoke_share_token_handler(
    store: &VaultStore,
    token_store: &ShareTokenStore,
    audit_store: &AuditStore,
    vault_id: &str,
    request: RevokeTokenRequest,
) -> Result<ShareToken, String> {
    let vault = store
        .lock()
        .unwrap()
        .get(vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    let token = revoke_share_token(token_store, &request.token)
        .ok_or_else(|| "Share token not found".to_string())?;

    if token.vault_id != vault_id {
        return Err("Token does not belong to this vault".to_string());
    }

    append_audit_entry(
        audit_store,
        "share_token_revoked",
        &vault.owner,
        serde_json::json!({
            "vault_id": vault_id,
            "token": token.token,
            "share_id": token.share_id,
        }),
    );

    Ok(token)
}

/// GET /vaults/{id}/share/tokens
pub fn list_share_tokens_handler(token_store: &ShareTokenStore, vault_id: &str) -> Vec<ShareToken> {
    get_vault_share_tokens(token_store, vault_id)
}

// ── Read-only access via share token ─────────────────────────────────────────

/// GET /shared/vaults/{token}
pub fn access_vault_via_share_handler(
    store: &VaultStore,
    token_store: &ShareTokenStore,
    audit_store: &AuditStore,
    token: &str,
) -> Result<Vault, String> {
    let share_token = validate_share_token(token_store, token)?;

    let vault = store
        .lock()
        .unwrap()
        .get(&share_token.vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    append_audit_entry(
        audit_store,
        "vault_accessed_via_share",
        &share_token.shared_with,
        serde_json::json!({
            "vault_id": share_token.vault_id,
            "token": token,
        }),
    );

    Ok(vault)
}

/// GET /shared/vaults/{token}/export
pub fn access_vault_export_via_share_handler(
    store: &VaultStore,
    event_store: &EventStore,
    audit_store: &AuditStore,
    token_store: &ShareTokenStore,
    token: &str,
    format: &str,
) -> Result<String, String> {
    let share_token = validate_share_token(token_store, token)?;

    let vault = store
        .lock()
        .unwrap()
        .get(&share_token.vault_id)
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    let history = get_vault_history(event_store, &share_token.vault_id);
    let audit_log = get_vault_audit_log(audit_store, &share_token.vault_id);

    let export_data = ExportData {
        vault,
        history,
        audit_log,
    };

    append_audit_entry(
        audit_store,
        "vault_exported_via_share",
        &share_token.shared_with,
        serde_json::json!({
            "vault_id": share_token.vault_id,
            "token": token,
            "format": format,
        }),
    );

    match format {
        "json" => Ok(serde_json::to_string_pretty(&export_data).map_err(|e| e.to_string())?),
        "csv" => export_to_csv(&export_data),
        _ => Err("Unsupported format".to_string()),
    }
}

fn validate_share_token(token_store: &ShareTokenStore, token: &str) -> Result<ShareToken, String> {
    let share_token =
        get_share_token(token_store, token).ok_or_else(|| "Invalid share token".to_string())?;

    if share_token.revoked {
        return Err("Share token has been revoked".to_string());
    }

    if Utc::now() > share_token.expires_at {
        return Err("Share token has expired".to_string());
    }

    if share_token.permission != SharePermission::ViewOnly {
        return Err("Share token does not have ViewOnly permission".to_string());
    }

    Ok(share_token)
}

/// GET /vaults/{id}/shares  (convenience accessor used in tests)
pub fn list_vault_shares_handler(share_store: &ShareStore, vault_id: &str) -> Vec<VaultShare> {
    get_vault_shares(share_store, vault_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Task 3: Sharing tests ─────────────────────────────────────────────────

    #[test]
    fn test_share_vault_creates_share() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 0,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        let req = ShareRequest {
            shared_with: "trusted@example.com".to_string(),
            permission: SharePermission::ViewOnly,
        };
        let result =
            share_vault_handler(&store, &share_store, &token_store, &audit_store, "v1", req);
        assert!(result.is_ok());
        let share = result.unwrap();
        assert_eq!(share.vault_id, "v1");
        assert_eq!(share.permission, SharePermission::ViewOnly);

        // Verify audit entry created
        assert!(audit_store
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.action == "vault_shared"));
    }

    #[test]
    fn test_share_vault_not_found() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        let req = ShareRequest {
            shared_with: "someone".to_string(),
            permission: SharePermission::Edit,
        };
        assert!(share_vault_handler(
            &store,
            &share_store,
            &token_store,
            &audit_store,
            "missing",
            req
        )
        .is_err());
    }

    #[test]
    fn test_list_vault_shares() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 0,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        share_vault_handler(
            &store,
            &share_store,
            &token_store,
            &audit_store,
            "v1",
            ShareRequest {
                shared_with: "a@example.com".to_string(),
                permission: SharePermission::ViewOnly,
            },
        )
        .unwrap();
        share_vault_handler(
            &store,
            &share_store,
            &token_store,
            &audit_store,
            "v1",
            ShareRequest {
                shared_with: "b@example.com".to_string(),
                permission: SharePermission::Admin,
            },
        )
        .unwrap();

        let shares = list_vault_shares_handler(&share_store, "v1");
        assert_eq!(shares.len(), 2);
    }

    #[test]
    fn test_share_permission_levels() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 0,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        for perm in [
            SharePermission::ViewOnly,
            SharePermission::Edit,
            SharePermission::Admin,
        ] {
            let req = ShareRequest {
                shared_with: "x".to_string(),
                permission: perm.clone(),
            };
            let share =
                share_vault_handler(&store, &share_store, &token_store, &audit_store, "v1", req)
                    .unwrap();
            assert_eq!(share.permission, perm);
        }
    }

    // ── Share token handler tests (#966) ──────────────────────────────────────

    #[test]
    fn test_generate_share_token_creates_token() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 1000,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        let result = generate_share_token_handler(
            &store,
            &share_store,
            &token_store,
            &audit_store,
            "v1",
            GenerateTokenRequest {
                shared_with: "family@example.com".to_string(),
                permission: None,
                expiry_seconds: Some(3600),
            },
        );
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.share.vault_id, "v1");
        assert_eq!(resp.token.permission, SharePermission::ViewOnly);
        assert_eq!(resp.token.revoked, false);
        assert!(resp.access_url.contains(&resp.token.token));

        // Verify persistence
        let stored = get_share_token(&token_store, &resp.token.token);
        assert!(stored.is_some());
        assert!(!stored.unwrap().revoked);
    }

    #[test]
    fn test_generate_share_token_vault_not_found() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        let result = generate_share_token_handler(
            &store,
            &share_store,
            &token_store,
            &audit_store,
            "nonexistent",
            GenerateTokenRequest {
                shared_with: "x@example.com".to_string(),
                permission: None,
                expiry_seconds: None,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_revoke_share_token_revokes() {
        let store = create_vault_store();
        let share_store = create_share_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 100,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        // Seed a token
        add_share_token(
            &token_store,
            ShareToken {
                token: "tok-1".to_string(),
                share_id: "s1".to_string(),
                vault_id: "v1".to_string(),
                shared_with: "test@example.com".to_string(),
                permission: SharePermission::ViewOnly,
                created_at: Utc::now(),
                expires_at: Utc::now() + chrono::Duration::days(7),
                revoked: false,
            },
        );

        let result = revoke_share_token_handler(
            &store,
            &token_store,
            &audit_store,
            "v1",
            RevokeTokenRequest {
                token: "tok-1".to_string(),
            },
        );
        assert!(result.is_ok());
        let token = result.unwrap();
        assert!(token.revoked);

        // Verify storage updated
        let stored = get_share_token(&token_store, "tok-1").unwrap();
        assert!(stored.revoked);
    }

    #[test]
    fn test_revoke_nonexistent_token_returns_error() {
        let store = create_vault_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 100,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        let result = revoke_share_token_handler(
            &store,
            &token_store,
            &audit_store,
            "v1",
            RevokeTokenRequest {
                token: "does-not-exist".to_string(),
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_revoke_token_wrong_vault_returns_error() {
        let store = create_vault_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 100,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        add_share_token(
            &token_store,
            ShareToken {
                token: "tok-other".to_string(),
                share_id: "s1".to_string(),
                vault_id: "other-vault".to_string(),
                shared_with: "test@example.com".to_string(),
                permission: SharePermission::ViewOnly,
                created_at: Utc::now(),
                expires_at: Utc::now() + chrono::Duration::days(7),
                revoked: false,
            },
        );

        let result = revoke_share_token_handler(
            &store,
            &token_store,
            &audit_store,
            "v1",
            RevokeTokenRequest {
                token: "tok-other".to_string(),
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_access_vault_via_valid_token() {
        let store = create_vault_store();
        let token_store = create_share_token_store();
        let audit_store = create_audit_store();
        store.lock().unwrap().insert(
            "v1".to_string(),
            Vault {
                id: "v1".to_string(),
                owner: "owner1".to_string(),
                beneficiary: "ben1".to_string(),
                balance: 5000,
                check_in_interval: 86400,
                last_check_in: Utc::now(),
                created_at: Utc::now(),
                status: VaultStatus::Active,
                ttl_remaining: Some(86400),
            },
        );

        add_share_token(
            &token_store,
            ShareToken {
                token: "valid-tok".to_string(),
                share_id: "s1".to_string(),
                vault_id: "v1".to_string(),
                shared_with: "reader@example.com".to_string(),
                permission: SharePermission::ViewOnly,
                created_at: Utc::now(),
                expires_at: Utc::now() + chrono::Duration::days(7),
                revoked: false,
            },
        );

        let result =
            access_vault_via_share_handler(&store, &token_store, &audit_store, "valid-tok");
        assert!(result.is_ok());
        let vault = result.unwrap();
        assert_eq!(vault.balance, 5000);
        assert_eq!(vault.owner, "owner1");

        // Audit log written
        assert!(audit_store
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.action == "vault_accessed_via_share"));
    }
}

// ── Issue #1852: Beneficiary proof-of-life periodic vote ─────────────────────

use serde::Deserialize;

#[derive(Deserialize)]
pub struct SubmitProofOfLifeRequest {
    pub vault_id: u64,
    pub beneficiary: String,
    pub proof: String,
    pub validity_seconds: Option<u64>,
}

/// POST /vaults/{id}/proof-of-life
///
/// Submits a beneficiary proof-of-life attestation for a vault.
/// The backend records the proof and can be used by a periodic scheduler
/// to request re-attestation from beneficiaries for high-value vaults.
#[instrument(skip(store), fields(vault_id = %vault_id))]
pub fn submit_proof_of_life_handler(
    store: &VaultStore,
    audit_store: &AuditStore,
    vault_id: u64,
    request: SubmitProofOfLifeRequest,
) -> Result<(), String> {
    let vaults = store.lock().unwrap();
    let vault = vaults
        .get(&vault_id.to_string())
        .cloned()
        .ok_or_else(|| "Vault not found".to_string())?;

    if vault.beneficiary != request.beneficiary {
        return Err("Only the beneficiary can submit proof of life".to_string());
    }

    let validity = request.validity_seconds.unwrap_or(86400 * 30); // default 30 days

    append_audit_entry(
        audit_store,
        "proof_of_life_submitted",
        &request.beneficiary,
        serde_json::json!({
            "vault_id": vault_id,
            "beneficiary": request.beneficiary,
            "validity_seconds": validity,
            "submitted_at": chrono::Utc::now().to_rfc3339(),
        }),
    );

    Ok(())
}
