use crate::ContractError;
use soroban_sdk::{contractclient, Address, Env};

/// Minimal oracle interface for conditional release queries.
/// External oracle contracts must expose a `query_release` function returning a boolean
/// indicating whether the release condition is met.
#[contractclient(name = "OracleClient")]
pub trait OracleInterface {
    fn query_release(env: Env) -> bool;
}

/// Queries an external oracle contract at `address` to determine if release conditions are satisfied.
/// Returns `Ok(true)` if the oracle confirms the condition is met, and `Ok(false)` if not met or on call failure.
pub fn query(env: &Env, address: &Address) -> Result<bool, ContractError> {
    let client = OracleClient::new(env, address);
    match client.try_query_release() {
        Ok(Ok(val)) => Ok(val),
        _ => Ok(false),
    }
}

/// Beneficiary identity oracle interface for KYC-gated claims.
/// Identity oracle contracts must expose an `is_verified` function returning whether
/// the given beneficiary address has been attested as identity-verified.
#[contractclient(name = "IdentityOracleClient")]
pub trait IdentityOracleInterface {
    fn is_verified(env: Env, beneficiary: Address) -> bool;
}

/// Queries an identity oracle contract at `address` to see if `beneficiary` has been attested.
/// Returns `Ok(true)` if the oracle attests the beneficiary, and `Ok(false)` if not or on call failure.
pub fn is_identity_verified(
    env: &Env,
    address: &Address,
    beneficiary: &Address,
) -> Result<bool, ContractError> {
    let client = IdentityOracleClient::new(env, address);
    match client.try_is_verified(beneficiary) {
        Ok(Ok(val)) => Ok(val),
        _ => Ok(false),
    }
}
