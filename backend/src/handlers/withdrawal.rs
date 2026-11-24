use crate::db::*;
use crate::error::AppError;
use crate::models::*;
use axum::http::HeaderMap;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tracing::instrument;

// --- Issue #1143: Vesting Bonus Backend API ---

/// Claim vesting bonus for a vault.
///
/// Validates the claim, relays the beneficiary-signed transaction to Soroban,
/// and records the network-reported transaction hash.
#[instrument(skip(db, headers, req), fields(vault_id = %vault_id))]
pub async fn claim_vesting_bonus_handler(
    db: Arc<Db>,
    headers: HeaderMap,
    vault_id: &str,
    req: crate::models::ClaimBonusRequest,
    rpc_url: Option<&str>,
) -> Result<crate::models::ClaimBonusResponse, AppError> {
    let vault = db.get_vault(vault_id).ok_or(AppError::NotFound)?;

    if vault.beneficiary != req.beneficiary {
        return Err(AppError::Forbidden(
            "caller is not the beneficiary for this vault".to_string(),
        ));
    }

    let bonus_config = db
        .get_vesting_bonus(vault_id)
        .map_err(AppError::from)?
        .ok_or(AppError::NotFound)?;

    if req.transaction_xdr.trim().is_empty() || req.transaction_xdr.len() > 2_000_000 {
        return Err(AppError::InvalidInput(
            "transaction_xdr must contain a signed transaction under 2 MB".to_string(),
        ));
    }

    let base_amount = vault.balance / 2;
    let bonus_amount = base_amount
        .checked_mul(bonus_config.bonus_bps as i128)
        .ok_or_else(|| AppError::InvalidInput("claim amount exceeds supported range".into()))?
        / 10_000;
    let claimed_amount = base_amount
        .checked_add(bonus_amount)
        .ok_or_else(|| AppError::InvalidInput("claim amount exceeds supported range".into()))?;

    let rpc_url = rpc_url
        .filter(|url| !url.is_empty())
        .ok_or(AppError::RpcUnavailable)?;
    let tx_hash = submit_signed_transaction(rpc_url, &req.transaction_xdr).await?;

    crate::audit::log_state_modification(
        &db,
        "vesting_bonus_claim",
        &format!("/api/vaults/{}/vesting/claim-bonus", vault_id),
        "success",
        &headers,
        Some(serde_json::json!({
            "vault_id": vault_id,
            "beneficiary": req.beneficiary,
            "bonus_bps": bonus_config.bonus_bps,
            "claimed_amount": claimed_amount,
            "bonus_amount": bonus_amount,
            "tx_hash": tx_hash,
        })),
    );

    Ok(crate::models::ClaimBonusResponse {
        vault_id: vault_id.to_string(),
        claimed_amount,
        bonus_amount,
        tx_hash,
        claimed_at: Utc::now(),
    })
}

pub async fn submit_signed_transaction(
    rpc_url: &str,
    transaction_xdr: &str,
) -> Result<String, AppError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| AppError::Rpc(format!("failed to create RPC client: {error}")))?;
    let response = client
        .post(rpc_url)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendTransaction",
            "params": { "transaction": transaction_xdr },
        }))
        .send()
        .await
        .map_err(|error| AppError::Rpc(format!("transaction submission failed: {error}")))?;

    if !response.status().is_success() {
        return Err(AppError::Rpc(format!(
            "RPC returned HTTP {}",
            response.status()
        )));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|error| AppError::Rpc(format!("invalid RPC response: {error}")))?;
    if let Some(error) = body.get("error") {
        return Err(AppError::Rpc(error.to_string()));
    }

    let result = body
        .get("result")
        .ok_or_else(|| AppError::Rpc("RPC response is missing result".to_string()))?;
    if result.get("status").and_then(serde_json::Value::as_str) == Some("ERROR") {
        return Err(AppError::Rpc(
            result
                .get("errorResult")
                .map(ToString::to_string)
                .unwrap_or_else(|| "transaction rejected".to_string()),
        ));
    }

    let tx_hash = result
        .get("hash")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| AppError::Rpc("RPC response is missing transaction hash".to_string()))?;
    if tx_hash.len() != 64 || !tx_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::Rpc(
            "RPC returned an invalid transaction hash".to_string(),
        ));
    }

    Ok(tx_hash.to_ascii_lowercase())
}

/// Retrieve the vesting bonus configuration for a vault.
pub fn get_vesting_bonus_handler(
    db: Arc<Db>,
    vault_id: &str,
) -> Result<crate::models::VestingBonusResponse, AppError> {
    let _vault = db
        .get_vault(vault_id)
        .ok_or(AppError::NotFound)?;

    let bonus_config = db
        .get_vesting_bonus(vault_id)
        .map_err(AppError::from)?;

    Ok(crate::models::VestingBonusResponse {
        vault_id: vault_id.to_string(),
        configured: bonus_config.is_some(),
        bonus_bps: bonus_config.as_ref().map(|c| c.bonus_bps),
        on_time_window_seconds: bonus_config.as_ref().map(|c| c.on_time_window_seconds),
    })
}

// --- Issue #1760: Re-check withdrawal whitelist TTL expiry at enforcement time ---

/// Verify that `address` is currently allowed to withdraw from `vault_id`.
///
/// Whitelist entries carry an optional TTL (`expires_at`). The expiry is
/// validated when the entry is created, but it must also be re-checked here at
/// enforcement time using the current ledger timestamp so that an entry which
/// expired after being added can no longer authorize a withdrawal.
pub fn enforce_withdrawal_whitelist(
    db: &Db,
    vault_id: &str,
    address: &str,
) -> Result<(), AppError> {
    let entry = db
        .get_withdrawal_whitelist_entry(vault_id, address)
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::Forbidden("address is not on the withdrawal whitelist".to_string())
        })?;

    if let Some(expires_at) = entry.expires_at {
        let now = Utc::now();
        if now >= expires_at {
            return Err(AppError::Forbidden(
                "withdrawal whitelist entry has expired".to_string(),
            ));
        }
    }

    Ok(())
}
