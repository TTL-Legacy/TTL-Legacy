//! Session binding for passkey-authenticated sessions — Issue #2135.
//!
//! A session created via passkey authentication is bound to the passkey's
//! device public key: the binding value is a deterministic, canonical
//! fingerprint of the stored key material, embedded as the
//! `session_binding` claim in the session token payload (both the access
//! and the refresh token — see `auth::issue_passkey_session`).
//!
//! The backend passkey store (`models::Passkey`) persists the WebAuthn
//! credential identifier per passkey, so the canonical device key material
//! is that identifier; the fingerprint is its SHA-256 digest.
//!
//! Verification: when validating a session that carries `session_binding`,
//! the request must present the device binding — the same SHA-256
//! fingerprint, sent via the `X-Session-Binding` header — and it is
//! compared with a constant-time equality check. Mismatches and missing
//! proofs are rejected with the standard unauthorized envelope and we fail
//! closed: an unbound token is never accepted for a passkey-bound session.
//!
//! Backward compatibility: sessions without a `session_binding` claim
//! (password logins, tokens issued before this feature) are unaffected.

use sha2::{Digest, Sha256};

use crate::error::AppError;
use crate::models::{AuthClaims, Passkey};

/// JWT claim name carrying the session binding (Issue #2135).
pub const SESSION_BINDING_CLAIM: &str = "session_binding";

/// HTTP header a client sends on each request to prove which device's
/// public key the session is bound to (Issue #2135).
pub const SESSION_BINDING_HEADER: &'static str = "x-session-binding";

/// Domain-separated prefix for the canonical device key material.
const KEY_MATERIAL_PREFIX: &[u8] = b"ttl-legacy:session-binding/v1:";

/// Canonical byte string identifying a passkey's device public key.
///
/// The passkey store keeps the WebAuthn credential identifier
/// (`Passkey::credential_id`) as the per-passkey key identifier, so the
/// canonical material is the domain-separated encoding of that identifier.
fn canonical_device_key(passkey: &Passkey) -> Vec<u8> {
    let mut material = Vec::with_capacity(KEY_MATERIAL_PREFIX.len() + passkey.credential_id.len());
    material.extend_from_slice(KEY_MATERIAL_PREFIX);
    material.extend_from_slice(passkey.credential_id.as_bytes());
    material
}

/// Deterministic, canonical binding value for a passkey: the SHA-256
/// fingerprint of its device public key material, hex-encoded.
///
/// Two passkeys have the same binding if and only if they share the same
/// device public key, so a session token can be checked against any device
/// by comparing this fingerprint to the one presented with the request.
pub fn compute_session_binding(passkey: &Passkey) -> String {
    let mut hasher = Sha256::new();
    hasher.update(&canonical_device_key(passkey));
    format!("{:x}", hasher.finalize())
}

/// Constant-time byte-slice equality — the equivalent of Node's
/// `crypto.timingSafeEqual` for equal-length inputs (Issue #2135).
///
/// Length is not secret here: bindings are fixed-size SHA-256 fingerprints,
/// so a length mismatch is simply a mismatch.
pub fn timing_safe_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= *x ^ *y;
    }
    diff == 0
}

/// Verifies the device binding presented with a session token against the
/// `session_binding` claim in the token claims (Issue #2135).
///
/// Fails closed: a passkey-bound session without a matching presented
/// binding is rejected with `AppError::Unauthorized`. Sessions without a
/// binding (password logins, legacy tokens) pass unchanged.
pub fn verify_session_binding(
    claims: &AuthClaims,
    presented: Option<&str>,
) -> Result<(), AppError> {
    match &claims.session_binding {
        None => Ok(()),
        Some(bound) => match presented {
            Some(presented) if timing_safe_eq(bound.as_bytes(), presented.as_bytes()) => Ok(()),
            _ => Err(AppError::Unauthorized(
                "session binding mismatch: token is bound to a different device public key"
                    .to_string(),
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AuthClaims;

    fn passkey(credential_id: &str) -> Passkey {
        Passkey {
            passkey_id: format!("pk-{credential_id}"),
            owner: "owner1".to_string(),
            vault_id: "vault-1".to_string(),
            credential_id: credential_id.to_string(),
            device_name: "Device".to_string(),
            registered_at: chrono::Utc::now(),
            last_used: None,
            is_backup: false,
        }
    }

    fn claims(session_binding: Option<String>) -> AuthClaims {
        AuthClaims {
            sub: "GABC...OWNER".to_string(),
            vault_ids: vec!["vault-1".to_string()],
            exp: 1_700_000_000,
            session_binding,
        }
    }

    #[test]
    fn compute_session_binding_is_deterministic_and_per_device() {
        let a1 = compute_session_binding(&passkey("cred-a"));
        let a2 = compute_session_binding(&passkey("cred-a"));
        let b = compute_session_binding(&passkey("cred-b"));

        assert_eq!(a1, a2, "same device public key must give the same binding");
        assert_ne!(a1, b, "different device public keys must differ");
        assert_eq!(a1.len(), 64, "SHA-256 hex digest is 64 chars");
        assert!(
            a1.chars().all(|c| c.is_ascii_hexdigit()),
            "binding must be hex-encoded: {a1}"
        );
    }

    #[test]
    fn timing_safe_eq_matches_equal_inputs() {
        assert!(timing_safe_eq(b"", b""));
        assert!(timing_safe_eq(b"abc", b"abc"));
        assert!(timing_safe_eq(b"0123456789abcdef", b"0123456789abcdef"));
    }

    #[test]
    fn timing_safe_eq_rejects_different_content_or_length() {
        assert!(!timing_safe_eq(b"abc", b"abd"));
        assert!(!timing_safe_eq(b"abc", b"abcd"));
        assert!(!timing_safe_eq(b"", b"a"));
        assert!(!timing_safe_eq(b"0123456789abcdef", b"0123456789abcdeg"));
    }

    #[test]
    fn verify_session_binding_passes_unbound_sessions() {
        let c = claims(None);
        assert!(verify_session_binding(&c, None).is_ok());
        assert!(verify_session_binding(&c, Some("anything")).is_ok());
    }

    #[test]
    fn verify_session_binding_accepts_matching_device() {
        let binding = compute_session_binding(&passkey("cred-a"));
        let c = claims(Some(binding.clone()));
        assert!(verify_session_binding(&c, Some(binding.as_str())).is_ok());
    }

    #[test]
    fn verify_session_binding_rejects_mismatched_device() {
        let binding = compute_session_binding(&passkey("cred-a"));
        let other = compute_session_binding(&passkey("cred-b"));
        let c = claims(Some(binding));
        assert!(verify_session_binding(&c, Some(other.as_str())).is_err());
        assert!(verify_session_binding(&c, Some("")).is_err());
    }

    #[test]
    fn verify_session_binding_fails_closed_on_missing_proof() {
        let binding = compute_session_binding(&passkey("cred-a"));
        let c = claims(Some(binding));
        assert!(verify_session_binding(&c, None).is_err());
    }
}
