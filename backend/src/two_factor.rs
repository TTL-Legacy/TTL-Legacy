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

// ── TOTP secret encryption at rest ───────────────────────────────────────────
//
// TOTP secrets must never be persisted in plaintext. Secrets are encrypted with
// a key sourced from secrets management (env var), and support key rotation via
// a primary key plus an optional previous key used only for decryption.

const ENC_PREFIX: &str = "enc:v1:";

fn primary_key() -> Result<[u8; 32], AppError> {
    let raw = std::env::var("TOTP_ENCRYPTION_KEY").map_err(|_| {
        AppError::Internal("TOTP_ENCRYPTION_KEY is not configured".into())
    })?;
    derive_key(&raw)
}

fn previous_key() -> Option<[u8; 32]> {
    std::env::var("TOTP_ENCRYPTION_KEY_PREVIOUS")
        .ok()
        .and_then(|raw| derive_key(&raw).ok())
}

fn derive_key(raw: &str) -> Result<[u8; 32], AppError> {
    let bytes = raw.as_bytes();
    if bytes.len() < 32 {
        return Err(AppError::Internal(
            "TOTP encryption key must be at least 32 bytes".into(),
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes[..32]);
    Ok(key)
}

fn keystream(key: &[u8; 32], nonce: &[u8; 16], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut counter: u64 = 0;
    while out.len() < len {
        let mut mac = Hmac::<Sha1>::new_from_slice(key).expect("hmac key");
        mac.update(nonce);
        mac.update(&counter.to_be_bytes());
        out.extend_from_slice(&mac.finalize().into_bytes());
        counter += 1;
    }
    out.truncate(len);
    out
}

fn xor_bytes(data: &[u8], stream: &[u8]) -> Vec<u8> {
    data.iter().zip(stream.iter()).map(|(a, b)| a ^ b).collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Encrypt a TOTP secret for storage. Output is `enc:v1:<nonce_hex>:<ct_hex>`.
fn encrypt_secret(secret: &str) -> Result<String, AppError> {
    let key = primary_key()?;
    let mut nonce = [0u8; 16];
    rand::thread_rng().fill(&mut nonce);
    let stream = keystream(&key, &nonce, secret.len());
    let ct = xor_bytes(secret.as_bytes(), &stream);
    Ok(format!(
        "{}{}:{}",
        ENC_PREFIX,
        hex_encode(&nonce),
        hex_encode(&ct)
    ))
}

/// Decrypt a stored secret, transparently handling legacy plaintext values.
fn decrypt_secret(stored: &str) -> Result<String, AppError> {
    let Some(rest) = stored.strip_prefix(ENC_PREFIX) else {
        // Legacy plaintext value (pre-migration).
        return Ok(stored.to_string());
    };
    let (nonce_hex, ct_hex) = rest
        .split_once(':')
        .ok_or_else(|| AppError::Internal("malformed encrypted secret".into()))?;
    let nonce_bytes = hex_decode(nonce_hex)
        .ok_or_else(|| AppError::Internal("malformed nonce".into()))?;
    let ct = hex_decode(ct_hex).ok_or_else(|| AppError::Internal("malformed ciphertext".into()))?;
    if nonce_bytes.len() != 16 {
        return Err(AppError::Internal("malformed nonce length".into()));
    }
    let mut nonce = [0u8; 16];
    nonce.copy_from_slice(&nonce_bytes);

    let mut keys = vec![primary_key()?];
    if let Some(prev) = previous_key() {
        keys.push(prev);
    }
    for key in keys {
        let stream = keystream(&key, &nonce, ct.len());
        let pt = xor_bytes(&ct, &stream);
        if let Ok(s) = String::from_utf8(pt) {
            return Ok(s);
        }
    }
    Err(AppError::Internal("unable to decrypt TOTP secret".into()))
}

/// Re-encrypt a stored secret under the current primary key (key rotation).
fn rotate_secret(stored: &str) -> Result<String, AppError> {
    let plaintext = decrypt_secret(stored)?;
    encrypt_secret(&plaintext)
}

/// Migration: re-encrypt any legacy plaintext secrets found in the store.
pub fn migrate_plaintext_secrets(db: &Db) -> Result<usize, AppError> {
    let configs = db.list_2fa_configs()?;
    let mut migrated = 0;
    for mut cfg in configs {
        if let Some(secret) = cfg.secret.clone() {
            if !secret.starts_with(ENC_PREFIX) {
                cfg.secret = Some(encrypt_secret(&secret)?);
                db.upsert_2fa_config(&cfg)?;
                migrated += 1;
            }
        }
    }
    Ok(migrated)
}

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
                return Err(AppError::InvalidInput(
                    "phone is required for SMS 2FA".into(),
                ));
            }
        }
        TwoFactorMethod::Email => {
            if body.email.as_ref().map_or(true, |e| e.trim().is_empty()) {
                return Err(AppError::InvalidInput(
                    "email is required for Email 2FA".into(),
                ));
            }
        }
        TwoFactorMethod::Totp => {}
    }

    match &body.method {
        TwoFactorMethod::Totp => {
            let secret = generate_totp_secret();
            let provisioning_uri = generate_provisioning_uri(&secret, &vault_id);

            // Persist only the encrypted form; the plaintext secret is returned
            // once to the caller for provisioning and never stored.
            let encrypted = encrypt_secret(&secret)?;
            let config = TwoFactorConfig {
                vault_id: vault_id.clone(),
                method: TwoFactorMethod::Totp,
                enabled: false,
                secret: Some(encrypted),
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
                phone: Some(phone),
                email: None,
                created_at: Utc::now(),
                verified_at: None,
            };
            db.upsert_2fa_config(&config)?;

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
                email: Some(email),
                created_at: Utc::now(),
                verified_at: None,
            };
            db.upsert_2fa_config(&config)?;

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
    let config = db
        .get_2fa_config(&vault_id)?
        .ok_or_else(|| AppError::NotFound("2FA is not configured for this vault".into()))?;

    let valid = match config.method {
        TwoFactorMethod::Totp => {
            let stored = config
                .secret
                .as_deref()
                .ok_or_else(|| AppError::Internal("missing TOTP secret".into()))?;
            let secret = decrypt_secret(stored)?;
            verify_totp_code(&secret, &body.code)
        }
        TwoFactorMethod::Sms | TwoFactorMethod::Email => verify_pending_otp(&vault_id, &body.code),
    };

    if !valid {
        return Err(AppError::Unauthorized("invalid 2FA code".into()));
    }

    let mut updated = config;
    updated.enabled = true;
    updated.verified_at = Some(Utc::now());
    db.upsert_2fa_config(&updated)?;

    SESSION_VERIFIED.lock().unwrap().insert(vault_id, true);
    Ok(StatusCode::OK)
}

/// POST /api/vaults/{vault_id}/2fa/rotate-key
///
/// Re-encrypts the stored TOTP secret under the current primary key. Used
/// during key rotation: set `TOTP_ENCRYPTION_KEY_PREVIOUS` to the old key and
/// `TOTP_ENCRYPTION_KEY` to the new key, then invoke this endpoint.
pub async fn rotate_2fa_key(
    State(db): State<Arc<Db>>,
    Path(vault_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let mut config = db
        .get_2fa_config(&vault_id)?
        .ok_or_else(|| AppError::NotFound("2FA is not configured for this vault".into()))?;

    if let Some(stored) = config.secret.clone() {
        config.secret = Some(rotate_secret(&stored)?);
        db.upsert_2fa_config(&config)?;
    }
    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_keys() {
        std::env::set_var("TOTP_ENCRYPTION_KEY", "0123456789abcdef0123456789abcdef");
        std::env::remove_var("TOTP_ENCRYPTION_KEY_PREVIOUS");
    }

    #[test]
    fn stored_secret_is_not_plaintext() {
        set_keys();
        let secret = generate_totp_secret();
        let stored = encrypt_secret(&secret).expect("encrypt");
        assert!(stored.starts_with(ENC_PREFIX));
        assert!(!stored.contains(&secret));
        assert_ne!(stored, secret);
        assert_eq!(decrypt_secret(&stored).expect("decrypt"), secret);
    }

    #[test]
    fn legacy_plaintext_is_readable_and_rotatable() {
        set_keys();
        let secret = generate_totp_secret();
        // Legacy plaintext decrypts to itself.
        assert_eq!(decrypt_secret(&secret).expect("legacy"), secret);
        // Rotation upgrades it to an encrypted value.
        let rotated = rotate_secret(&secret).expect("rotate");
        assert!(rotated.starts_with(ENC_PREFIX));
        assert_eq!(decrypt_secret(&rotated).expect("decrypt"), secret);
    }

    #[test]
    fn previous_key_can_decrypt_after_rotation() {
        std::env::set_var("TOTP_ENCRYPTION_KEY", "0123456789abcdef0123456789abcdef");
        std::env::remove_var("TOTP_ENCRYPTION_KEY_PREVIOUS");
        let secret = generate_totp_secret();
        let old_ct = encrypt_secret(&secret).expect("encrypt");

        // Rotate: new primary key, old key retained for decryption.
        std::env::set_var("TOTP_ENCRYPTION_KEY_PREVIOUS", "0123456789abcdef0123456789abcdef");
        std::env::set_var("TOTP_ENCRYPTION_KEY", "fedcba9876543210fedcba9876543210");
        assert_eq!(decrypt_secret(&old_ct).expect("decrypt old"), secret);

        let new_ct = rotate_secret(&old_ct).expect("rotate");
        assert!(new_ct.starts_with(ENC_PREFIX));
        assert_eq!(decrypt_secret(&new_ct).expect("decrypt new"), secret);
    }
}
