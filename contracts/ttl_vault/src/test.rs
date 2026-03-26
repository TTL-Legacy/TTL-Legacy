#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};
use types::VaultError;

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
}

#[test]
fn test_check_in_resets_timer() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);

    env.ledger().with_mut(|l| l.timestamp += 43200);
    client.check_in(&vault_id, &owner);

    let remaining = client.get_ttl_remaining(&vault_id);
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
fn test_update_beneficiary() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    let new_beneficiary = Address::generate(&env);
    client.update_beneficiary(&vault_id, &new_beneficiary);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.beneficiary, new_beneficiary);
}

#[test]
fn test_deposit_increases_balance() {
    let (_, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    client.deposit(&vault_id, &owner, &500_000);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, 500_000);
}

#[test]
fn test_withdraw_decreases_balance() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    client.deposit(&vault_id, &owner, &500_000);
    client.withdraw(&vault_id, &200_000);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, 300_000);
}

#[test]
fn test_trigger_release_transfers_to_beneficiary() {
    let (env, owner, beneficiary, token_address, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    client.deposit(&vault_id, &owner, &500_000);

    env.ledger().with_mut(|l| l.timestamp += 90000); // expire
    client.trigger_release(&vault_id);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, 0);
    assert_eq!(vault.status, ReleaseStatus::Released);

    let token = TokenClient::new(&env, &token_address);
    assert_eq!(token.balance(&beneficiary), 500_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_trigger_release_zero_balance() {
    let (env, owner, beneficiary, _, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &86400u64);
    // no deposit — balance stays 0
    env.ledger().with_mut(|l| l.timestamp += 90000);
    client.trigger_release(&vault_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_create_vault_zero_interval() {
    let (_, owner, beneficiary, _, client) = setup();
    client.create_vault(&owner, &beneficiary, &0u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_load_nonexistent_vault() {
    let (_, _, _, _, client) = setup();
    client.get_vault(&999u64);
}
