#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, Address,
    Bytes, BytesN, Env, Vec,
};

pub const MAX_PROOF_SIZE: u32 = 4096;
pub const MAX_CLAIM_SIZE: u32 = 1024;

/// Expected byte length of a well-formed proof.
pub const EXPECTED_PROOF_SIZE: u32 = 256;
/// Expected byte length of a well-formed public input (claim).
pub const EXPECTED_CLAIM_SIZE: u32 = 32;
/// Minimum byte length of a well-formed proof.
pub const MIN_PROOF_SIZE: u32 = EXPECTED_PROOF_SIZE;
/// Minimum byte length of a well-formed public input (claim).
pub const MIN_CLAIM_SIZE: u32 = EXPECTED_CLAIM_SIZE;

const VERIFY_CLAIM_TOPIC: soroban_sdk::Symbol = symbol_short!("vfy_claim");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VerifierError {
    /// Proof bytes were empty.
    EmptyProof = 1,
    /// Claim bytes were empty.
    EmptyClaim = 2,
    /// Proof bytes exceed MAX_PROOF_SIZE.
    ProofTooLarge = 3,
    /// Claim bytes exceed MAX_CLAIM_SIZE.
    ClaimTooLarge = 4,
    /// The contract is already initialized.
    AlreadyInitialized = 5,
    /// The sender is not the registered admin.
    NotAdmin = 6,
    /// The given oracle address is not registered.
    OracleNotFound = 7,
    /// Proof byte length does not match EXPECTED_PROOF_SIZE.
    InvalidProofLength = 8,
    /// Public input (claim) byte length does not match EXPECTED_CLAIM_SIZE.
    InvalidInputLength = 9,
    /// Proof byte length is below MIN_PROOF_SIZE.
    ProofTooShort = 10,
    /// Public input (claim) byte length is below MIN_CLAIM_SIZE.
    ClaimTooShort = 11,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Oracle(Address),
    Attestation(BytesN<32>, BytesN<32>),
    ReleaseReady,
}

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

    fn require_admin(env: &Env) {
        let Some(admin) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin)
        else {
            panic_with_error!(env, VerifierError::NotAdmin);
        };
        admin.require_auth();
    }

    /// Register a trusted oracle. Admin only.
    pub fn register_oracle(env: Env, oracle: Address) {
        Self::require_admin(&env);
        env.storage()
            .instance()
            .set(&DataKey::Oracle(oracle), &true);
    }

    /// Revoke a trusted oracle. Admin only.
    pub fn revoke_oracle(env: Env, oracle: Address) {
        Self::require_admin(&env);
        env.storage().instance().remove(&DataKey::Oracle(oracle));
        env.storage().instance().set(&DataKey::ReleaseReady, &false);
    }

    /// Returns whether the given address is a registered oracle.
    pub fn is_oracle(env: Env, oracle: Address) -> bool {
        env.storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Oracle(oracle))
            .unwrap_or(false)
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
        if !env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Oracle(oracle.clone()))
            .unwrap_or(false)
        {
            panic_with_error!(&env, VerifierError::OracleNotFound);
        }
        oracle.require_auth();
        let proof_hash: BytesN<32> = env.crypto().sha256(&proof).into();
        let claim_hash: BytesN<32> = env.crypto().sha256(&claim).into();
        env.storage()
            .instance()
            .set(&DataKey::Attestation(proof_hash, claim_hash), &oracle);
        env.storage().instance().set(&DataKey::ReleaseReady, &false);
    }

    /// Verifies a zero-knowledge proof against a claim using oracle attestation.
    ///
    /// Returns `true` only when a registered oracle attested this proof and claim.
    ///
    /// Emits a `vfy_claim` event with `(result, claim_hash)` on every call
    /// that passes input validation.
    pub fn verify_claim(env: Env, proof: Bytes, claim: Bytes) -> bool {
        Self::validate_proof(&env, &proof);
        Self::validate_claim(&env, &claim);

        let proof_hash: BytesN<32> = env.crypto().sha256(&proof).into();
        let claim_hash: BytesN<32> = env.crypto().sha256(&claim).into();
        let is_attested = env
            .storage()
            .instance()
            .has(&DataKey::Attestation(proof_hash.clone(), claim_hash.clone()));
        let result = is_attested;
        env.storage().instance().set(&DataKey::ReleaseReady, &result);

        env.events()
            .publish((VERIFY_CLAIM_TOPIC,), (result, claim_hash));

        result
    }

    /// Validates proof byte length before any hashing/deserialization.
    fn validate_proof(env: &Env, proof: &Bytes) {
        if proof.is_empty() {
            panic_with_error!(env, VerifierError::EmptyProof);
        }
        if proof.len() > MAX_PROOF_SIZE {
            panic_with_error!(env, VerifierError::ProofTooLarge);
        }
        if proof.len() < MIN_PROOF_SIZE {
            panic_with_error!(env, VerifierError::ProofTooShort);
        }
        if proof.len() != EXPECTED_PROOF_SIZE {
            panic_with_error!(env, VerifierError::InvalidProofLength);
        }
    }

    /// Validates claim byte length before any hashing/deserialization.
    fn validate_claim(env: &Env, claim: &Bytes) {
        if claim.is_empty() {
            panic_with_error!(env, VerifierError::EmptyClaim);
        }
        if claim.len() > MAX_CLAIM_SIZE {
            panic_with_error!(env, VerifierError::ClaimTooLarge);
        }
        if claim.len() < MIN_CLAIM_SIZE {
            panic_with_error!(env, VerifierError::ClaimTooShort);
        }
        if claim.len() != EXPECTED_CLAIM_SIZE {
            panic_with_error!(env, VerifierError::InvalidInputLength);
        }
    }

    /// Verifies multiple proofs in a single invocation, amortising gas costs.
    ///
    /// Each proof is length-validated before verification. Returns `true` only
    /// when every proof is attested; panics with the first typed error found.
    pub fn verify_batch_proofs(env: Env, proofs: Vec<Bytes>) -> bool {
        let mut all_valid = true;
        for proof in proofs.iter() {
            Self::validate_proof(&env, &proof);
            let proof_hash: BytesN<32> = env.crypto().sha256(&proof).into();
            let is_attested = env
                .storage()
                .instance()
                .has(&DataKey::Attestation(proof_hash, BytesN::from_array(&env, &[0u8; 32])));
            if !is_attested {
                all_valid = false;
            }
        }
        env.storage()
            .instance()
            .set(&DataKey::ReleaseReady, &all_valid);
        all_valid
    }

    /// Implements the vault oracle interface for proof-triggered releases.
    pub fn query_release(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::ReleaseReady)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod test;
