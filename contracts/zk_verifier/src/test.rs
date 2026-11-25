#![cfg(test)]

use super::*;
use soroban_sdk::{bytes, testutils::Events as _, Bytes, Env};

fn setup() -> (Env, Address, ZkVerifierContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let id = env.register_contract(None, ZkVerifierContract);
    let client = ZkVerifierContractClient::new(&env, &id);
    client.initialize(&admin);
    let client: ZkVerifierContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, admin, client)
}

// ---- #1811: empty inputs return typed errors instead of panicking ----

#[test]
fn test_malformed_empty_proof_returns_typed_error() {
    let (env, _, client) = setup();
    let proof = bytes!(&env,);
    let claim = bytes!(&env, 0xcafebabe);
    assert_eq!(
        client.try_verify_claim(&proof, &claim),
        Err(Ok(Error::ProofTooShort))
    );
}

#[test]
fn test_malformed_empty_claim_returns_typed_error() {
    let (env, _, client) = setup();
    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env,);
    assert_eq!(
        client.try_verify_claim(&proof, &claim),
        Err(Ok(Error::ClaimTooShort))
    );
}

// ---- oracle model tests ----

#[test]
fn test_unattested_proof_returns_false() {
    let (env, _, client) = setup();
    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env, 0xcafebabe);
    assert!(!client.verify_claim(&proof, &claim));
}

// ── Existing correctness tests ────────────────────────────────────────────────

/// Valid proof and claim — must return true.
#[test]
fn test_attested_proof_returns_true() {
    let (env, _, client) = setup();
    let oracle = Address::generate(&env);
    client.register_oracle(&oracle);

    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env, 0xcafebabe);
    client.attest(&oracle, &proof, &claim);

    assert!(client.verify_claim(&proof, &claim));
}

#[test]
fn test_query_release_tracks_proof_verification_result() {
    let (env, _, client) = setup();
    let oracle = Address::generate(&env);
    client.register_oracle(&oracle);
    let proof = Bytes::from_slice(&env, &[1u8; EXPECTED_PROOF_SIZE as usize]);
    let claim = Bytes::from_slice(&env, &[2u8; EXPECTED_CLAIM_SIZE as usize]);

    assert!(!client.query_release());
    client.attest(&oracle, &proof, &claim);
    assert!(client.verify_claim(&proof, &claim));
    assert!(client.query_release());
    assert!(!client.query_release());
}

#[test]
fn test_different_proof_not_validated_after_attestation() {
    let (env, _, client) = setup();
    let oracle = Address::generate(&env);
    client.register_oracle(&oracle);

    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env, 0xcafebabe);
    client.attest(&oracle, &proof, &claim);

    let other_proof = bytes!(&env, 0x1234);
    assert!(!client.verify_claim(&other_proof, &claim));
}

#[test]
fn test_revoked_oracle_attestation_no_longer_accepted() {
    let (env, _, client) = setup();
    let oracle = Address::generate(&env);
    client.register_oracle(&oracle);

    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env, 0xcafebabe);
    client.attest(&oracle, &proof, &claim);

    // Attestation was stored before revocation — verify it persists
    assert!(client.verify_claim(&proof, &claim));

    // Revoking the oracle does not retroactively remove stored attestations;
    // it only prevents future attest() calls.
    client.revoke_oracle(&oracle);
    assert!(!client.is_oracle(&oracle));
    // Existing attestation still validates (attested fact is immutable).
    assert!(client.verify_claim(&proof, &claim));
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_unregistered_oracle_cannot_attest() {
    let (env, _, client) = setup();
    let rogue = Address::generate(&env);

    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env, 0xcafebabe);
    client.attest(&rogue, &proof, &claim);
}

#[test]
fn test_register_and_is_oracle() {
    let (env, _, client) = setup();
    let oracle = Address::generate(&env);

    assert!(!client.is_oracle(&oracle));
    client.register_oracle(&oracle);
    assert!(client.is_oracle(&oracle));
    client.revoke_oracle(&oracle);
    assert!(!client.is_oracle(&oracle));
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_double_initialize_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let id = env.register_contract(None, ZkVerifierContract);
    let client = ZkVerifierContractClient::new(&env, &id);
    client.initialize(&admin);
    client.initialize(&admin);
}

// ── #817: Input size limit tests ──────────────────────────────────────────────

/// Proof at exactly MAX_PROOF_SIZE — must succeed.
#[test]
fn test_proof_at_max_size_succeeds() {
    let (env, client) = setup();
    let data = [0xffu8; MAX_PROOF_SIZE as usize];
    let proof = Bytes::from_slice(&env, &data);
    let claim = bytes!(&env, 0xcafebabe);
    assert!(client.verify_claim(&proof, &claim));
}

/// Proof one byte over MAX_PROOF_SIZE — must panic with ProofTooLarge (#3).
#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_proof_exceeds_max_size_panics() {
    let (env, client) = setup();
    let data = [0xffu8; MAX_PROOF_SIZE as usize + 1];
    let proof = Bytes::from_slice(&env, &data);
    let claim = bytes!(&env, 0xcafebabe);
    client.verify_claim(&proof, &claim);
}

/// Claim at exactly MAX_CLAIM_SIZE — must succeed.
#[test]
fn test_claim_at_max_size_succeeds() {
    let (env, client) = setup();
    let proof = bytes!(&env, 0xdeadbeef);
    let data = [0xaau8; MAX_CLAIM_SIZE as usize];
    let claim = Bytes::from_slice(&env, &data);
    assert!(client.verify_claim(&proof, &claim));
}

/// Claim one byte over MAX_CLAIM_SIZE — must panic with ClaimTooLarge (#4).
#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_claim_exceeds_max_size_panics() {
    let (env, client) = setup();
    let proof = bytes!(&env, 0xdeadbeef);
    let data = [0xaau8; MAX_CLAIM_SIZE as usize + 1];
    let claim = Bytes::from_slice(&env, &data);
    client.verify_claim(&proof, &claim);
}

// ── #1518 / #1811: Input length validation tests ──────────────────────────────

/// Proof shorter than MIN_PROOF_SIZE — must return ProofTooShort (#6).
#[test]
fn test_proof_below_min_size_returns_typed_error() {
    let (env, client) = setup();
    let data = [0xffu8; MIN_PROOF_SIZE as usize - 1];
    let proof = Bytes::from_slice(&env, &data);
    let claim = bytes!(&env, 0xcafebabe);
    assert_eq!(
        client.try_verify_claim(&proof, &claim),
        Err(Ok(Error::ProofTooShort))
    );
}

/// Proof at exactly MIN_PROOF_SIZE — must succeed.
#[test]
fn test_proof_at_min_size_succeeds() {
    let (env, client) = setup();
    let data = [0xffu8; MIN_PROOF_SIZE as usize];
    let proof = Bytes::from_slice(&env, &data);
    let claim = bytes!(&env, 0xcafebabe);
    assert!(client.verify_claim(&proof, &claim));
}

/// Claim shorter than MIN_CLAIM_SIZE — must return ClaimTooShort (#7).
#[test]
fn test_claim_below_min_size_returns_typed_error() {
    let (env, client) = setup();
    let proof = bytes!(&env, 0xdeadbeef);
    let data = [0xaau8; MIN_CLAIM_SIZE as usize - 1];
    let claim = Bytes::from_slice(&env, &data);
    assert_eq!(
        client.try_verify_claim(&proof, &claim),
        Err(Ok(Error::ClaimTooShort))
    );
}

/// Claim at exactly MIN_CLAIM_SIZE — must succeed.
#[test]
fn test_claim_at_min_size_succeeds() {
    let (env, client) = setup();
    let proof = bytes!(&env, 0xdeadbeef);
    let data = [0xaau8; MIN_CLAIM_SIZE as usize];
    let claim = Bytes::from_slice(&env, &data);
    assert!(client.verify_claim(&proof, &claim));
}

/// Malformed proof of unexpected (too short) length must be rejected before
/// verification, returning the typed ProofTooShort error.
#[test]
fn test_malformed_short_proof_rejected() {
    let (env, client) = setup();
    let proof = bytes!(&env, 0x01);
    let claim = bytes!(&env, 0xcafebabe);
    assert_eq!(
        client.try_verify_claim(&proof, &claim),
        Err(Ok(Error::ProofTooShort))
    );
}

/// Malformed claim of unexpected (too short) length must be rejected before
/// verification, returning the typed ClaimTooShort error.
#[test]
fn test_malformed_short_claim_rejected() {
    let (env, client) = setup();
    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env, 0x01);
    assert_eq!(
        client.try_verify_claim(&proof, &claim),
        Err(Ok(Error::ClaimTooShort))
    );
}

// ── #818: Event emission tests ────────────────────────────────────────────────

/// verify_claim with a valid proof must emit exactly one vfy_claim event.
#[test]
fn test_verify_claim_emits_event_on_true_result() {
    let (env, client) = setup();
    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env, 0xcafebabe);
    let result = client.verify_claim(&proof, &claim);
    assert!(result);
    assert_eq!(env.events().all().len(), 1);
}

/// verify_claim with the 0x00 sentinel must emit exactly one vfy_claim event
/// even when the result is false.
#[test]
fn test_verify_claim_emits_event_on_false_result() {
    let (env, client) = setup();
    let proof = bytes!(&env, 0x00); // known-invalid sentinel → result = false
    let claim = bytes!(&env, 0xcafebabe);
    let result = client.verify_claim(&proof, &claim);
    assert!(!result);
    assert_eq!(env.events().all().len(), 1);
}

// ── #1812: verify_batch_proofs tests ──────────────────────────────────────────

/// A batch of valid proofs must all verify successfully.
#[test]
fn test_verify_batch_proofs_all_valid() {
    let (env, _, client) = setup();
    let oracle = Address::generate(&env);
    client.register_oracle(&oracle);

    let proof_a = bytes!(&env, 0xdeadbeef);
    let proof_b = bytes!(&env, 0xfeedface);
    let claim = bytes!(&env, 0xcafebabe);
    client.attest(&oracle, &proof_a, &claim);
    client.attest(&oracle, &proof_b, &claim);

    let mut proofs = Vec::new(&env);
    proofs.push_back(proof_a);
    proofs.push_back(proof_b);

    assert!(client.verify_batch_proofs(&proofs));
}

/// A batch containing an undersized proof must return the first typed error
/// (ProofTooShort) rather than panicking.
#[test]
fn test_verify_batch_proofs_mixed_validity_returns_first_error() {
    let (env, _, client) = setup();
    let oracle = Address::generate(&env);
    client.register_oracle(&oracle);

    let proof_a = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env, 0xcafebabe);
    client.attest(&oracle, &proof_a, &claim);

    let short_proof = bytes!(&env, 0x01);

    let mut proofs = Vec::new(&env);
    proofs.push_back(proof_a);
    proofs.push_back(short_proof);

    assert_eq!(
        client.try_verify_batch_proofs(&proofs),
        Err(Ok(Error::ProofTooShort))
    );
}

/// A batch containing an oversized proof must return ProofTooLarge.
#[test]
fn test_verify_batch_proofs_oversized_returns_typed_error() {
    let (env, _, client) = setup();
    let data = [0xffu8; MAX_PROOF_SIZE as usize + 1];
    let oversized = Bytes::from_slice(&env, &data);

    let mut proofs = Vec::new(&env);
    proofs.push_back(oversized);

    assert_eq!(
        client.try_verify_batch_proofs(&proofs),
        Err(Ok(Error::ProofTooLarge))
    );
}

/// An empty batch must succeed trivially (vacuously true).
#[test]
fn test_verify_batch_proofs_empty_batch_succeeds() {
    let (env, _, client) = setup();
    let proofs: Vec<Bytes> = Vec::new(&env);
    assert!(client.verify_batch_proofs(&proofs));
}
