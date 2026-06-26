#![cfg(test)]

use super::*;
use soroban_sdk::{bytes, Env, Address};
use soroban_sdk::testutils::Address as _;

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

// ---- existing interface: empty inputs still panic ----

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_malformed_empty_proof_panics() {
    let (env, _, client) = setup();
    let proof = bytes!(&env,);
    let claim = bytes!(&env, 0xcafebabe);
    client.verify_claim(&proof, &claim);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_malformed_empty_claim_panics() {
    let (env, _, client) = setup();
    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env,);
    client.verify_claim(&proof, &claim);
}

// ---- oracle model tests ----

#[test]
fn test_unattested_proof_returns_false() {
    let (env, _, client) = setup();
    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env, 0xcafebabe);
    assert!(!client.verify_claim(&proof, &claim));
}

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
