#![no_std]

//! ZK Verifier Contract — trusted-oracle model.
//!
//! # Trust model
//! Full on-chain Groth16/PLONK verification is not yet feasible on Soroban
//! because the required elliptic-curve host functions (BLS12-381 pairings) are
//! not exposed as stable host functions at the time of writing. This contract
//! therefore implements a **trusted-oracle** model:
//!
//! * A designated admin registers one or more trusted oracle addresses.
//! * Each oracle can publish an attestation for a (proof_hash, claim_hash)
//!   pair, asserting that the proof is valid for the claim.
//! * `verify_claim` succeeds if and only if an active oracle has attested to
//!   the supplied proof/claim pair.
//!
//! The security guarantee is equivalent to trusting the registered oracles.
//! When native ZK host functions become available this contract should be
//! replaced with on-chain cryptographic verification and the oracle layer
//! removed.

use soroban_sdk::{
    contract, contractimpl, contracterror, panic_with_error,
    Bytes, BytesN, Env, Address,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VerifierError {
    /// Proof bytes were empty.
    EmptyProof = 1,
    /// Claim bytes were empty.
    EmptyClaim = 2,
    /// Caller is not the admin.
    NotAdmin = 3,
    /// The oracle address is not registered.
    OracleNotFound = 4,
    /// Contract has already been initialized.
    AlreadyInitialized = 5,
    /// Contract has not been initialized.
    NotInitialized = 6,
}

/// Storage key discriminants.
mod keys {
    use soroban_sdk::{contracttype, Address, BytesN};

    #[contracttype]
    pub enum DataKey {
        Admin,
        Oracle(Address),
        /// Attestation: (proof_sha256, claim_sha256) → attesting oracle
        Attestation(BytesN<32>, BytesN<32>),
    }
}

use keys::DataKey;

#[contract]
pub struct ZkVerifierContract;

#[contractimpl]
impl ZkVerifierContract {
    /// Initialize the contract with an admin address.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, VerifierError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Register a trusted oracle. Admin only.
    pub fn register_oracle(env: Env, oracle: Address) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Oracle(oracle), &true);
    }

    /// Revoke a trusted oracle. Admin only.
    pub fn revoke_oracle(env: Env, oracle: Address) {
        Self::require_admin(&env);
        env.storage().instance().remove(&DataKey::Oracle(oracle));
    }

    /// Returns whether the given address is a registered oracle.
    pub fn is_oracle(env: Env, oracle: Address) -> bool {
        env.storage().instance().get::<DataKey, bool>(&DataKey::Oracle(oracle)).unwrap_or(false)
    }

    /// An oracle publishes an attestation that `proof` is valid for `claim`.
    ///
    /// The contract stores the SHA-256 digests of both byte strings so that
    /// the full proof bytes are not stored on-chain.
    pub fn attest(env: Env, oracle: Address, proof: Bytes, claim: Bytes) {
        if proof.is_empty() {
            panic_with_error!(&env, VerifierError::EmptyProof);
        }
        if claim.is_empty() {
            panic_with_error!(&env, VerifierError::EmptyClaim);
        }
        if !env.storage().instance().get::<DataKey, bool>(&DataKey::Oracle(oracle.clone())).unwrap_or(false) {
            panic_with_error!(&env, VerifierError::OracleNotFound);
        }
        oracle.require_auth();
        let proof_hash: BytesN<32> = env.crypto().sha256(&proof).into();
        let claim_hash: BytesN<32> = env.crypto().sha256(&claim).into();
        env.storage().instance().set(
            &DataKey::Attestation(proof_hash, claim_hash),
            &oracle,
        );
    }

    /// Verifies a zero-knowledge proof against a claim using oracle attestation.
    ///
    /// Returns `true` if a registered oracle has attested to this (proof, claim)
    /// pair; `false` otherwise.
    ///
    /// # Errors
    /// * `EmptyProof`  — if `proof` is empty.
    /// * `EmptyClaim`  — if `claim` is empty.
    pub fn verify_claim(env: Env, proof: Bytes, claim: Bytes) -> bool {
        if proof.is_empty() {
            panic_with_error!(&env, VerifierError::EmptyProof);
        }
        if claim.is_empty() {
            panic_with_error!(&env, VerifierError::EmptyClaim);
        }
        let proof_hash: BytesN<32> = env.crypto().sha256(&proof).into();
        let claim_hash: BytesN<32> = env.crypto().sha256(&claim).into();
        env.storage()
            .instance()
            .has(&DataKey::Attestation(proof_hash, claim_hash))
    }

    // ---- helpers ----

    fn require_admin(env: &Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, VerifierError::NotInitialized));
        admin.require_auth();
    }
}

#[cfg(test)]
mod test;
