use soroban_sdk::contracterror;

/// Errors returned by the zk_verifier contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    /// The provided proof does not have the expected byte length.
    InvalidProofLength = 1,
    /// The provided public inputs do not have the expected length.
    InvalidInputLength = 2,
    /// The proof failed cryptographic verification.
    VerificationFailed = 3,
}
