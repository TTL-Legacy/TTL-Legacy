// Minimal oracle module for external release condition queries
use soroban_sdk::{Env, Address, Symbol, symbol_short, contracterror, panic_with_error, Val};
use crate::types::ContractError;

pub fn query(env: &Env, address: &Address) -> Result<bool, ContractError> {
    // Expect the external oracle contract to expose a `query_release` function returning a boolean
    // indicating whether the release condition is met.
    // This call may fail; propagate as a generic error.
    let result = env.invoke_contract(address, &symbol!("query_release"), &[]);
    match result {
        Ok(val) => {
            // Attempt to convert the returned value into a bool. If conversion fails, treat as false.
            match bool::try_from(val) {
                Ok(b) => Ok(b),
                Err(_) => Ok(false),
            }
        }
        Err(_) => Ok(false), // On failure, treat as condition not met to avoid unintended releases.
    }
}
