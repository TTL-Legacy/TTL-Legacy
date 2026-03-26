#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    vec, Address, Env, IntoVal,
};
use types::{ReleaseEvent, RELEASE_TOPIC};

fn setup() -> (Env, Address, Address, Address, TtlVaultContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    // Deploy a native-style token (stellar asset contract)
    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract_v2(token_admin.clone()).address();

    // Mint some tokens to owner so deposits work
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address);

    // Leak env lifetime for convenience — safe in tests
    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, beneficiary, token_address, client)
}

#[test]
fn test_create_vault_extends_vault_ttl() {
    let (env, owner, beneficiary) = setup();
    let contract_id = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);

    let ttl = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get_ttl(&DataKey::Vault(vault_id))
    });
    assert!(
        ttl >= VAULT_TTL_THRESHOLD,
        "vault TTL {ttl} is below threshold {VAULT_TTL_THRESHOLD}"
    );
}

#[test]
fn test_check_in_extends_vault_ttl() {
    let (env, owner, beneficiary) = setup();
    let contract_id = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    env.ledger().with_mut(|l| l.sequence_number += 1000);
    client.check_in(&vault_id, &owner);

    let ttl = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get_ttl(&DataKey::Vault(vault_id))
    });
    assert!(
        ttl >= VAULT_TTL_THRESHOLD,
        "vault TTL {ttl} is below threshold after check_in"
    );
}

#[test]
fn test_create_vault_extends_instance_ttl() {
    let (env, owner, beneficiary) = setup();
    let contract_id = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_id);

    client.create_vault(&owner, &beneficiary, &86400u64);

    // Instance TTL must be at least the threshold away from expiry.
    let ttl = env.as_contract(&contract_id, || {
        env.storage().instance().get_ttl()
    });
    assert!(
        ttl >= INSTANCE_TTL_THRESHOLD,
        "instance TTL {ttl} is below threshold {INSTANCE_TTL_THRESHOLD}"
    );
}

#[test]
fn test_create_vault() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    assert_eq!(vault_id, 1);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.owner, owner);
    assert_eq!(vault.beneficiary, beneficiary);
    assert_eq!(vault.balance, 0);

    // Assert that vault creation event was emitted
    let events = env.events().all();
    assert_eq!(events.len(), 1);
    
    let event = events.first().unwrap();
    
    // Check the topics (event.1 is a Vec<Val>)
    let topics = &event.1;
    assert_eq!(topics.len(), 1);
    let topic_symbol = Symbol::from_val(&env, &topics.get_unchecked(0));
    assert_eq!(topic_symbol, Symbol::new(&env, "v_created"));
    
    // Check the data (event.2 is a Val containing our tuple)
    let data_tuple = <(u64, Address, Address, u64)>::from_val(&env, &event.2);
    assert_eq!(data_tuple, (vault_id, owner, beneficiary, 86400u64));
}

#[test]
fn test_check_in_resets_timer() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);

    env.ledger().with_mut(|l| l.timestamp += 43200);
    client.check_in(&vault_id, &owner);

    // TTL remaining should be close to full interval again
    let remaining = client.get_ttl_remaining(&vault_id).unwrap();
    assert!(remaining > 43000 && remaining <= 86400);
}

#[test]
fn test_non_owner_cannot_check_in() {
    let (env, owner, beneficiary) = setup();
    let client = TtlVaultContractClient::new(&env, &env.register_contract(None, TtlVaultContract));

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    let stranger = Address::generate(&env);

    let result = client.try_check_in(&vault_id, &stranger);
    assert_eq!(
        result,
        Err(Ok(VaultError::NotOwner)),
        "non-owner must receive NotOwner error"
    );
}

#[test]
fn test_is_not_expired_before_interval() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    env.ledger().with_mut(|l| l.timestamp += 43200);
    assert!(!client.is_expired(&vault_id));
}

#[test]
fn test_is_expired_after_interval() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    env.ledger().with_mut(|l| l.timestamp += 90000);
    assert!(client.is_expired(&vault_id));
}

#[test]
fn test_withdraw_zero_amount_rejected() {
    let (env, owner, beneficiary) = setup();
    let client = TtlVaultContractClient::new(&env, &env.register_contract(None, TtlVaultContract));

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    let result = client.try_withdraw(&vault_id, &0i128);
    assert_eq!(result, Err(Ok(VaultError::InvalidAmount)));
}

#[test]
fn test_withdraw_negative_amount_rejected() {
    let (env, owner, beneficiary) = setup();
    let client = TtlVaultContractClient::new(&env, &env.register_contract(None, TtlVaultContract));

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    let result = client.try_withdraw(&vault_id, &-1i128);
    assert_eq!(result, Err(Ok(VaultError::InvalidAmount)));
}

#[test]
fn test_check_in_emits_event() {
    let (env, owner, beneficiary) = setup();
    let contract_id = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    let ts = env.ledger().timestamp();
    client.check_in(&vault_id);

    let events = env.events().all();
    assert_eq!(events.len(), 1);

    let (_, topics, data) = events.get(0).unwrap();
    // topic[0] = symbol "check_in", topic[1] = vault_id
    let topic0: soroban_sdk::Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic1: u64 = topics.get(1).unwrap().try_into_val(&env).unwrap();
    assert_eq!(topic0, symbol_short!("check_in"));
    assert_eq!(topic1, vault_id);
    // data = last_check_in timestamp
    let emitted_ts: u64 = data.try_into_val(&env).unwrap();
    assert_eq!(emitted_ts, ts);
}

#[test]
fn test_update_beneficiary() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    let new_beneficiary = Address::generate(&env);
    client.update_beneficiary(&vault_id, &new_beneficiary);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.beneficiary, new_beneficiary);
}

#[test]
#[should_panic(expected = "vault already released")]
fn test_update_beneficiary_after_release_fails() {
    let (env, owner, beneficiary) = setup();
    let client = TtlVaultContractClient::new(&env, &env.register_contract(None, TtlVaultContract));

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);

    // Advance time past the check-in interval to expire the vault
    env.ledger().with_mut(|l| l.timestamp += 90000);
    client.trigger_release(&vault_id);

    // Attempt to update beneficiary on a Released vault — must panic
    let new_beneficiary = Address::generate(&env);
    client.update_beneficiary(&vault_id, &new_beneficiary);
}

#[test]
fn test_update_beneficiary_while_locked_near_expiry() {
    let (env, owner, beneficiary) = setup();
    let client = TtlVaultContractClient::new(&env, &env.register_contract(None, TtlVaultContract));

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);

    // Advance time to just before expiry — vault is still Locked
    env.ledger().with_mut(|l| l.timestamp += 86399);
    assert!(!client.is_expired(&vault_id));

    let new_beneficiary = Address::generate(&env);
    client.update_beneficiary(&vault_id, &new_beneficiary);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.beneficiary, new_beneficiary);
}

#[test]
fn test_trigger_release_emits_event() {
    let (env, owner, beneficiary) = setup();
    let contract_id = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);

    // Advance past the check-in interval
    env.ledger().with_mut(|l| l.timestamp += 90000);

    client.trigger_release(&vault_id);

    let events = env.events().all();
    assert_eq!(events.len(), 1);

    let (emitted_contract, topics, data) = events.get(0).unwrap();
    assert_eq!(emitted_contract, contract_id);
    assert_eq!(topics, vec![&env, RELEASE_TOPIC.into_val(&env)]);

    let event: ReleaseEvent = data.into_val(&env);
    assert_eq!(event.vault_id, vault_id);
    assert_eq!(event.beneficiary, beneficiary);
    assert_eq!(event.amount, 0); // no deposit was made
}
