#![cfg(test)]

use super::*;
use soroban_sdk::{bytes, Env};

fn setup() -> (Env, ZkVerifierContractClient<'static>) {
    let env = Env::default();
    let id = env.register_contract(None, ZkVerifierContract);
    let client = ZkVerifierContractClient::new(&env, &id);
    (env, client)
}

/// Valid proof and claim — must return true.
#[test]
fn test_valid_proof_returns_true() {
    let (env, client) = setup();
    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env, 0xcafebabe);
    assert!(client.verify_claim(&proof, &claim));
}

/// Invalid proof (0x00 sentinel) — must return false.
#[test]
fn test_invalid_proof_returns_false() {
    let (env, client) = setup();
    let proof = bytes!(&env, 0x00); // known-invalid sentinel
    let claim = bytes!(&env, 0xcafebabe);
    assert!(!client.verify_claim(&proof, &claim));
}

/// Malformed input: empty proof — must panic with EmptyProof (#1).
#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_malformed_empty_proof_panics() {
    let (env, client) = setup();
    let proof = bytes!(&env,);
    let claim = bytes!(&env, 0xcafebabe);
    client.verify_claim(&proof, &claim);
}

/// Malformed input: empty claim — must panic with EmptyClaim (#2).
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_malformed_empty_claim_panics() {
    let (env, client) = setup();
    let proof = bytes!(&env, 0xdeadbeef);
    let claim = bytes!(&env,);
    client.verify_claim(&proof, &claim);
}
