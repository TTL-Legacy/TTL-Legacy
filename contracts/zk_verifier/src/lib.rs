#![no_std]

use soroban_sdk::{contract, contractimpl, contracterror, panic_with_error, Bytes, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VerifierError {
    /// Proof bytes were empty.
    EmptyProof = 1,
    /// Claim bytes were empty.
    EmptyClaim = 2,
}

#[contract]
pub struct ZkVerifierContract;

#[contractimpl]
impl ZkVerifierContract {
    /// Verifies a zero-knowledge proof against a claim.
    ///
    /// # STUB
    /// Real ZK proof verification (e.g. Groth16, PLONK) requires a verifier
    /// circuit and cryptographic primitives not yet available as Soroban host
    /// functions. This implementation is a non-empty bytes guard that acts as
    /// a placeholder until a native ZK host function is exposed.
    ///
    /// Returns `true` when both `proof` and `claim` are non-empty.
    pub fn verify_claim(env: Env, proof: Bytes, claim: Bytes) -> bool {
        // STUB: replace with real ZK verification once host functions are available.
        if proof.is_empty() {
            panic_with_error!(&env, VerifierError::EmptyProof);
        }
        if claim.is_empty() {
            panic_with_error!(&env, VerifierError::EmptyClaim);
        }
        // STUB: a single 0x00 byte is treated as a known-invalid proof sentinel.
        // Real ZK verification would replace this with cryptographic validation.
        if proof.len() == 1 && proof.get(0) == Some(0x00) {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod test;
