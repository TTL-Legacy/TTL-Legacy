#![cfg(test)]

//! Issue #1281: `get_release_status` did not have a way to represent a
//! release that was initiated (conditions met, release attempted) but whose
//! token transfer failed, leaving the balance stuck in the contract instead
//! of either still-Locked or fully Released.
//!
//! `FailingToken` is a minimal token double that accepts inbound transfers
//! (deposits, where `to` is the vault contract) but panics on any other
//! transfer (releases, where `to` is the beneficiary). Using `try_transfer`
//! against it lets `trigger_release` observe a real, catchable transfer
//! failure without depending on an unrelated insufficient-balance scenario.

extern crate alloc;

use super::*;
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger},
    Address, Env,
};

#[contract]
pub struct FailingToken;

#[contractimpl]
impl FailingToken {
    /// Records the one address `transfer` is allowed to send to (the vault
    /// contract, so deposits still work); any other destination panics.
    pub fn init(env: Env, allowed_to: Address) {
        env.storage().instance().set(&symbol_short!("allow"), &allowed_to);
    }

    pub fn balance(_env: Env, _id: Address) -> i128 {
        i128::MAX
    }

    pub fn transfer(env: Env, _from: Address, to: Address, _amount: i128) {
        let allowed: Address = env
            .storage()
            .instance()
            .get(&symbol_short!("allow"))
            .expect("FailingToken not initialized");
        if to != allowed {
            panic!("FailingToken rejects outbound transfers");
        }
    }
}

fn setup() -> (Env, Address, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    let contract_id = env.register_contract(None, TtlVaultContract);
    let token_id = env.register_contract(None, FailingToken);
    FailingTokenClient::new(&env, &token_id).init(&contract_id);

    let client = TtlVaultContractClient::new(&env, &contract_id);
    client.initialize(&token_id, &admin);
    let client: TtlVaultContractClient<'static> =
        unsafe { core::mem::transmute(client) };

    (env, owner, beneficiary, client)
}

#[test]
fn test_trigger_release_sets_failed_status_on_transfer_error() {
    let (env, owner, beneficiary, client) = setup();

    let interval = 3600u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.set_release_conditions(
        &vault_id,
        &owner,
        &soroban_sdk::vec![&env, ReleaseCondition::TTLExpiry],
    );
    client.deposit(&vault_id, &owner, &1_000i128);

    // Fast-forward past expiry so trigger_release's condition/grace checks pass.
    env.ledger().with_mut(|l| l.timestamp += interval + 1);

    // The release is attempted, but FailingToken rejects the outbound
    // transfer to `beneficiary`. trigger_release should record the failure
    // rather than trap the whole invocation.
    let result = client.try_trigger_release(&vault_id);
    assert!(
        result.is_ok(),
        "trigger_release should complete normally even when the transfer fails"
    );

    assert_eq!(client.get_release_status(&vault_id), ReleaseStatus::Failed);
    // The vault's internal accounting still reflects the un-transferred funds.
    assert_eq!(client.get_deposit_total(&vault_id), 1_000i128);
}
