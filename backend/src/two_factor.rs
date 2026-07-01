use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use rand::Rng;
use sha1::Sha1;

use crate::{
    db::Db,
    error::AppError,
    models::{
        Enable2FARequest, Enable2FAResponse, TwoFactorConfig, TwoFactorMethod,
        TwoFactorStatusResponse, Verify2FARequest,
    },
};

// ── Global stores ────────────────────────────────────────────────────────────

struct PendingOtp {
    code: String,
    expires_at: u64,
}

static PENDING_OTPS: once_cell::sync::Lazy<Mutex<HashMap<String, Vec<PendingOtp>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

static SESSION_VERIFIED: once_cell::sync::Lazy<Mutex<HashMap<String, bool>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

// ── Helpers ──────────────────────────────────────────────────────────────────

fn generate_otp_code() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..1_000_000))
}

fn generate_totp_secret() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..20).map(|_| rng.gen()).collect();
    base32_encode(&bytes)
}

fn base32_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::new();
    let mut buffer = 0u64;
    let mut bits = 0;
    for &byte in input {
        buffer = (buffer << 8) | byte as u64;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 0x1F) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 0x1F) as usize] as char);
    }
    out
}

fn generate_provisioning_uri(secret: &str, label: &str) -> String {
    let encoded_label: String = label
        .chars()
        .map(|c| match c {
            ':' | ' ' => '_',
            _ => c,
        })
        .collect();
    format!(
        "otpauth://totp/{}?secret={}&issuer=TTL-Legacy&algorithm=SHA1&digits=6&period=30",
        encoded_label, secret
    )
}

fn verify_totp_code(secret: &str, code: &str) -> bool {
    let secret_bytes = match base32_decode(secret) {
        Some(b) => b,
        None => return false,
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let time_step = now / 30;

    for offset in [0u64, 1, u64::MAX] {
        let counter = if offset == u64::MAX {
            if time_step == 0 {
                continue;
            }
            time_step - 1
        } else {
            time_step + offset
        };

        let counter_be = counter.to_be_bytes();
        let mut mac = match Hmac::<Sha1>::new_from_slice(&secret_bytes) {
            Ok(m) => m,
            Err(_) => return false,
        };
        mac.update(&counter_be);
        let result = mac.finalize();
        let hash = result.into_bytes();

        let offset = (hash[19] & 0x0F) as usize;
        let binary = ((hash[offset] & 0x7F) as u32) << 24
            | (hash[offset + 1] as u32) << 16
            | (hash[offset + 2] as u32) << 8
            | (hash[offset + 3] as u32);
        let totp = binary % 1_000_000;

        if format!("{:06}", totp) == code {
            return true;
        }
    }
    false
}

fn base32_decode(input: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let cleaned = cleaned.to_uppercase();

    let mut out = Vec::new();
    let mut buffer = 0u64;
    let mut bits = 0;

    for c in cleaned.chars() {
        let val = match ALPHABET.iter().position(|&a| a as char == c) {
            Some(v) => v as u64,
            None => return None,
        };
        buffer = (buffer << 5) | val;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn verify_pending_otp(vault_id: &str, code: &str) -> bool {
    let mut store = PENDING_OTPS.lock().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if let Some(codes) = store.get_mut(vault_id) {
        codes.retain(|otp| otp.expires_at > now);
        if let Some(pos) = codes.iter().position(|otp| otp.code == code) {
            codes.remove(pos);
            return true;
        }
    }
    false
}

// ── Route handlers ───────────────────────────────────────────────────────────

/// GET /api/vaults/{vault_id}/2fa/status
pub async fn get_2fa_status(
    State(db): State<Arc<Db>>,
    Path(vault_id): Path<String>,
) -> Result<Json<TwoFactorStatusResponse>, AppError> {
    let config = db.get_2fa_config(&vault_id)?;
    let session_verified = SESSION_VERIFIED
        .lock()
        .unwrap()
        .get(&vault_id)
        .copied()
        .unwrap_or(false);

    match config {
        Some(cfg) => Ok(Json(TwoFactorStatusResponse {
            vault_id: cfg.vault_id,
            enabled: cfg.enabled,
            method: Some(cfg.method),
            verified: session_verified,
            phone: cfg.phone,
            email: cfg.email,
        })),
        None => Ok(Json(TwoFactorStatusResponse {
            vault_id,
            enabled: false,
            method: None,
            verified: false,
            phone: None,
            email: None,
        })),
    }
}

/// POST /api/vaults/{vault_id}/2fa/enable
pub async fn enable_2fa(
    State(db): State<Arc<Db>>,
    Path(vault_id): Path<String>,
    Json(body): Json<Enable2FARequest>,
) -> Result<Json<Enable2FAResponse>, AppError> {
    match &body.method {
        TwoFactorMethod::Sms => {
            if body.phone.as_ref().map_or(true, |p| p.trim().is_empty()) {
                return Err(AppError::InvalidInput("phone is required for SMS 2FA".into()));
            }
        }
        TwoFactorMethod::Email => {
            if body.email.as_ref().map_or(true, |e| e.trim().is_empty()) {
                return Err(AppError::InvalidInput("email is required for Email 2FA".into()));
            }
        }
        TwoFactorMethod::Totp => {}
    }

    match &body.method {
        TwoFactorMethod::Totp => {
            let secret = generate_totp_secret();
            let provisioning_uri = generate_provisioning_uri(&secret, &vault_id);

            let config = TwoFactorConfig {
                vault_id: vault_id.clone(),
                method: TwoFactorMethod::Totp,
                enabled: false,
                secret: Some(secret.clone()),
                phone: None,
                email: None,
                created_at: Utc::now(),
                verified_at: None,
            };
            db.upsert_2fa_config(&config)?;

            Ok(Json(Enable2FAResponse {
                vault_id,
                method: TwoFactorMethod::Totp,
                secret: Some(secret),
                provisioning_uri: Some(provisioning_uri),
            }))
        }
        TwoFactorMethod::Sms => {
            let phone = body.phone.unwrap_or_default();
            let code = generate_otp_code();
            let expires_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 300;

            PENDING_OTPS
                .lock()
                .unwrap()
                .entry(vault_id.clone())
                .or_default()
                .push(PendingOtp {
                    code: code.clone(),
                    expires_at,
                });

            let config = TwoFactorConfig {
                vault_id: vault_id.clone(),
                method: TwoFactorMethod::Sms,
                enabled: false,
                secret: None,
                phone: Some(phone.clone()),
                email: None,
                created_at: Utc::now(),
                verified_at: None,
            };
            db.upsert_2fa_config(&config)?;

            tracing::info!(vault_id, phone, code, "SMS OTP sent");

            Ok(Json(Enable2FAResponse {
                vault_id,
                method: TwoFactorMethod::Sms,
                secret: None,
                provisioning_uri: None,
            }))
        }
        TwoFactorMethod::Email => {
            let email = body.email.unwrap_or_default();
            let code = generate_otp_code();
            let expires_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 300;

            PENDING_OTPS
                .lock()
                .unwrap()
                .entry(vault_id.clone())
                .or_default()
                .push(PendingOtp {
                    code: code.clone(),
                    expires_at,
                });

            let config = TwoFactorConfig {
                vault_id: vault_id.clone(),
                method: TwoFactorMethod::Email,
                enabled: false,
                secret: None,
                phone: None,
                email: Some(email.clone()),
                created_at: Utc::now(),
                verified_at: None,
            };
            db.upsert_2fa_config(&config)?;

            tracing::info!(vault_id, email, code, "Email OTP sent");

            Ok(Json(Enable2FAResponse {
                vault_id,
                method: TwoFactorMethod::Email,
                secret: None,
                provisioning_uri: None,
            }))
        }
    }
}

/// POST /api/vaults/{vault_id}/2fa/verify
pub async fn verify_2fa(
    State(db): State<Arc<Db>>,
    Path(vault_id): Path<String>,
    Json(body): Json<Verify2FARequest>,
) -> Result<StatusCode, AppError> {
    let config = db.get_2fa_config(&vault_id)?.ok_or(AppError::NotFound)?;

    let valid = match &config.method {
        TwoFactorMethod::Totp => {
            let secret = config.secret.as_ref().ok_or_else(|| {
                AppError::InvalidInput("TOTP secret not found".into())
            })?;
            verify_totp_code(secret, &body.otp)
        }
        TwoFactorMethod::Sms | TwoFactorMethod::Email => {
            verify_pending_otp(&vault_id, &body.otp)
        }
    };

    if !valid {
        return Err(AppError::InvalidInput("Invalid or expired OTP".into()));
    }

    let updated = TwoFactorConfig {
        enabled: true,
        verified_at: Some(Utc::now()),
        ..config
    };
    db.upsert_2fa_config(&updated)?;

    SESSION_VERIFIED.lock().unwrap().insert(vault_id, true);

    Ok(StatusCode::OK)
}

/// POST /api/vaults/{vault_id}/2fa/disable
pub async fn disable_2fa(
    State(db): State<Arc<Db>>,
    Path(vault_id): Path<String>,
) -> Result<StatusCode, AppError> {
    db.delete_2fa_config(&vault_id)?;
    SESSION_VERIFIED.lock().unwrap().remove(&vault_id);
    PENDING_OTPS.lock().unwrap().remove(&vault_id);
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/vaults/{vault_id}/2fa/challenge
pub async fn challenge_2fa(
    State(db): State<Arc<Db>>,
    Path(vault_id): Path<String>,
) -> Result<Json<TwoFactorStatusResponse>, AppError> {
    let config = db.get_2fa_config(&vault_id)?;
    let session_verified = SESSION_VERIFIED
        .lock()
        .unwrap()
        .get(&vault_id)
        .copied()
        .unwrap_or(false);

    match config {
        Some(cfg) => {
            let requires_2fa = cfg.enabled && !session_verified;
            Ok(Json(TwoFactorStatusResponse {
                vault_id: cfg.vault_id,
                enabled: cfg.enabled,
                method: Some(cfg.method),
                verified: !requires_2fa,
                phone: cfg.phone,
                email: cfg.email,
            }))
        }
        None => Ok(Json(TwoFactorStatusResponse {
            vault_id,
            enabled: false,
            method: None,
            verified: true,
            phone: None,
            email: None,
        })),
    }
}

/// POST /api/vaults/{vault_id}/2fa/session/clear
pub async fn clear_2fa_session(
    Path(vault_id): Path<String>,
) -> Result<StatusCode, AppError> {
    SESSION_VERIFIED.lock().unwrap().remove(&vault_id);
    Ok(StatusCode::OK)
}
