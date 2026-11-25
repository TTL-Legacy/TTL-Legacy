#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, Address,
    Bytes, BytesN, Env, String, Vec,
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
const PROOF_TYPE_REGISTERED_TOPIC: soroban_sdk::Symbol = symbol_short!("pt_reg");
const PROOF_TYPE_REMOVED_TOPIC: soroban_sdk::Symbol = symbol_short!("pt_rem");
const VAULT_PROOF_TYPE_SET_TOPIC: soroban_sdk::Symbol = symbol_short!("vpt_set");

/// Supported proof systems. Variants are stored by their u32 discriminant so
/// the on-chain representation is stable across upgrades.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofSystem {
    /// Groth16 — constant-size proofs, trusted-setup ceremony required.
    Groth16,
    /// PLONK — universal trusted-setup, slightly larger proofs than Groth16.
    Plonk,
    /// Halo2 — no trusted setup, recursive proofs supported.
    Halo2,
}

/// Registry entry for a single proof system.
#[contracttype]
#[derive(Clone)]
pub struct ProofTypeEntry {
    /// Human-readable name, e.g. "Groth16".
    pub name: String,
    /// Expected proof byte length for this system.
    pub expected_proof_size: u32,
    /// Expected claim (public input) byte length for this system.
    pub expected_claim_size: u32,
    /// Whether this proof system is currently accepted for new attestations.
    pub enabled: bool,
}

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
    /// The requested proof type is not registered in the registry.
    ProofTypeNotFound = 12,
    /// The requested proof type is registered but currently disabled.
    ProofTypeDisabled = 13,
    /// A vault already has a proof type assigned; use set_vault_proof_type to update.
    VaultProofTypeAlreadySet = 14,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Oracle(Address),
    Attestation(BytesN<32>, BytesN<32>),
    ReleaseReady,
    /// Registry entry for a named proof system.
    ProofType(ProofSystem),
    /// Per-vault required proof system (vault_id → ProofSystem).
    VaultProofType(u64),
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

    // ── Proof-type registry (Issue #2064) ────────────────────────────────────

    /// Register or update a proof system in the global registry. Admin only.
    ///
    /// `name` is a human-readable label stored on-chain for display purposes
    /// (e.g. `"Groth16"`). `expected_proof_size` and `expected_claim_size` set
    /// the byte-length constraints used when this system is active for a vault.
    pub fn register_proof_type(
        env: Env,
        proof_system: ProofSystem,
        name: String,
        expected_proof_size: u32,
        expected_claim_size: u32,
    ) {
        Self::require_admin(&env);
        let entry = ProofTypeEntry {
            name,
            expected_proof_size,
            expected_claim_size,
            enabled: true,
        };
        env.storage()
            .instance()
            .set(&DataKey::ProofType(proof_system.clone()), &entry);
        env.events()
            .publish((PROOF_TYPE_REGISTERED_TOPIC,), (proof_system,));
    }

    /// Disable a proof system in the registry. Admin only.
    ///
    /// Disabled systems can no longer be assigned to new vaults and will not
    /// accept new attestations, but existing vault assignments and already-stored
    /// attestations remain valid.
    pub fn disable_proof_type(env: Env, proof_system: ProofSystem) {
        Self::require_admin(&env);
        let key = DataKey::ProofType(proof_system.clone());
        let mut entry = env
            .storage()
            .instance()
            .get::<DataKey, ProofTypeEntry>(&key)
            .unwrap_or_else(|| panic_with_error!(&env, VerifierError::ProofTypeNotFound));
        entry.enabled = false;
        env.storage().instance().set(&key, &entry);
        env.events()
            .publish((PROOF_TYPE_REMOVED_TOPIC,), (proof_system,));
    }

    /// Return the registry entry for a proof system, or panic with
    /// `ProofTypeNotFound` if it has never been registered.
    pub fn get_proof_type(env: Env, proof_system: ProofSystem) -> ProofTypeEntry {
        env.storage()
            .instance()
            .get::<DataKey, ProofTypeEntry>(&DataKey::ProofType(proof_system))
            .unwrap_or_else(|| panic_with_error!(&env, VerifierError::ProofTypeNotFound))
    }

    /// Assign a required proof system to a vault. Admin only.
    ///
    /// Once set, `verify_claim_for_vault` will enforce the byte-length
    /// constraints of the assigned proof system instead of the global defaults.
    /// The proof system must already be registered and enabled.
    pub fn set_vault_proof_type(env: Env, vault_id: u64, proof_system: ProofSystem) {
        Self::require_admin(&env);
        let entry = env
            .storage()
            .instance()
            .get::<DataKey, ProofTypeEntry>(&DataKey::ProofType(proof_system.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, VerifierError::ProofTypeNotFound));
        if !entry.enabled {
            panic_with_error!(&env, VerifierError::ProofTypeDisabled);
        }
        env.storage()
            .instance()
            .set(&DataKey::VaultProofType(vault_id), &proof_system);
        env.events()
            .publish((VAULT_PROOF_TYPE_SET_TOPIC,), (vault_id, proof_system));
    }

    /// Return the proof system assigned to a vault, if any.
    pub fn get_vault_proof_type(env: Env, vault_id: u64) -> Option<ProofSystem> {
        env.storage()
            .instance()
            .get::<DataKey, ProofSystem>(&DataKey::VaultProofType(vault_id))
    }

    /// Verify a claim using vault-specific proof-system constraints when the
    /// vault has an assigned proof type, falling back to global defaults.
    ///
    /// This is the preferred entry point for vault-aware callers — it honours
    /// the per-vault proof system selection set by `set_vault_proof_type`.
    pub fn verify_claim_for_vault(env: Env, vault_id: u64, proof: Bytes, claim: Bytes) -> bool {
        // Look up vault-specific proof system constraints, or fall back to
        // the global constants if no system is assigned.
        let (expected_proof, expected_claim) = env
            .storage()
            .instance()
            .get::<DataKey, ProofSystem>(&DataKey::VaultProofType(vault_id))
            .and_then(|ps| {
                env.storage()
                    .instance()
                    .get::<DataKey, ProofTypeEntry>(&DataKey::ProofType(ps))
            })
            .map(|e| (e.expected_proof_size, e.expected_claim_size))
            .unwrap_or((EXPECTED_PROOF_SIZE, EXPECTED_CLAIM_SIZE));

        Self::validate_proof_with_sizes(&env, &proof, expected_proof);
        Self::validate_claim_with_sizes(&env, &claim, expected_claim);

        let proof_hash: BytesN<32> = env.crypto().sha256(&proof).into();
        let claim_hash: BytesN<32> = env.crypto().sha256(&claim).into();
        let is_attested = env
            .storage()
            .instance()
            .has(&DataKey::Attestation(proof_hash.clone(), claim_hash.clone()));

        env.storage()
            .instance()
            .set(&DataKey::ReleaseReady, &is_attested);
        env.events()
            .publish((VERIFY_CLAIM_TOPIC,), (is_attested, claim_hash));
        is_attested
    }

    /// Validate proof length against caller-supplied size constraints.
    fn validate_proof_with_sizes(env: &Env, proof: &Bytes, expected: u32) {
        if proof.is_empty() {
            panic_with_error!(env, VerifierError::EmptyProof);
        }
        if proof.len() > MAX_PROOF_SIZE {
            panic_with_error!(env, VerifierError::ProofTooLarge);
        }
        if proof.len() < expected {
            panic_with_error!(env, VerifierError::ProofTooShort);
        }
        if proof.len() != expected {
            panic_with_error!(env, VerifierError::InvalidProofLength);
        }
    }

    /// Validate claim length against caller-supplied size constraints.
    fn validate_claim_with_sizes(env: &Env, claim: &Bytes, expected: u32) {
        if claim.is_empty() {
            panic_with_error!(env, VerifierError::EmptyClaim);
        }
        if claim.len() > MAX_CLAIM_SIZE {
            panic_with_error!(env, VerifierError::ClaimTooLarge);
        }
        if claim.len() < expected {
            panic_with_error!(env, VerifierError::ClaimTooShort);
        }
        if claim.len() != expected {
            panic_with_error!(env, VerifierError::InvalidInputLength);
        }
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

/// Type alias kept for backwards compatibility with existing test references.
pub use VerifierError as Error;

#[cfg(test)]
mod test;
