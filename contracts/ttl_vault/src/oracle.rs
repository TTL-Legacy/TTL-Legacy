use soroban_sdk::{contractclient, Address, Env};
use crate::ContractError;

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
