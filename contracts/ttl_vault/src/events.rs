use soroban_sdk::{Address, Env};

use crate::types::FUNDS_CLAWEDBACK_TOPIC;

/// Emits a clawback event when a beneficiary's allocation is reclaimed.
pub fn emit_clawback(env: &Env, vault_id: u64, beneficiary: &Address, amount: u32) {
    env.events().publish(
        (FUNDS_CLAWEDBACK_TOPIC, vault_id),
        (beneficiary.clone(), amount),
    );
}
