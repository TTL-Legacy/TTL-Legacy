#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl, testutils::Address as _, token::StellarAssetClient, vec, Address, Env,
};

/// Mock Oracle Contract for testing oracle-gated release conditions.
#[contract]
pub struct MockOracle;

#[contracttype]
#[derive(Clone)]
pub enum MockOracleKey {
    ReleaseStatus,
}

#[contractimpl]
impl MockOracle {
    /// Configures the return value of `query_release`.
    pub fn set_release(env: Env, status: bool) {
        env.storage()
            .instance()
            .set(&MockOracleKey::ReleaseStatus, &status);
    }

    /// Implements `OracleInterface::query_release`.
    pub fn query_release(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&MockOracleKey::ReleaseStatus)
            .unwrap_or(false)
    }
}

/// Helper client for MockOracle
pub struct MockOracleClient<'a> {
    pub env: &'a Env,
    pub address: &'a Address,
}

impl<'a> MockOracleClient<'a> {
    pub fn new(env: &'a Env, address: &'a Address) -> Self {
        Self { env, address }
    }

    pub fn set_release(&self, status: &bool) {
        self.env
            .invoke_contract::<()>(self.address, &soroban_sdk::Symbol::new(self.env, "set_release"), soroban_sdk::vec![self.env, status.into_val(self.env)]);
    }

    pub fn query_release(&self) -> bool {
        self.env
            .invoke_contract::<bool>(self.address, &soroban_sdk::Symbol::new(self.env, "query_release"), soroban_sdk::vec![self.env])
    }
}

fn setup() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
    TtlVaultContractClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let admin = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, beneficiary, admin, token_address, client)
}

#[test]
fn test_oracle_gated_release_success_when_oracle_true() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let oracle_address = env.register_contract(None, MockOracle);
    let oracle_client = MockOracleClient::new(&env, &oracle_address);
    oracle_client.set_release(&true);

    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);
    client.deposit(&vault_id, &owner, &10_000);

    // Set single release condition to Oracle
    client.set_release_condition(&vault_id, &owner, &ReleaseCondition::Oracle(oracle_address));

    // Vault is NOT expired yet (timestamp has not advanced past 1000s)
    assert!(!client.is_expired(&vault_id));

    // Release should succeed immediately because oracle returns true
    client.trigger_release(&vault_id);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Released);
    assert_eq!(vault.balance, 0);

    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&beneficiary), 10_000);
}

#[test]
fn test_oracle_gated_release_blocked_when_oracle_false() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let oracle_address = env.register_contract(None, MockOracle);
    let oracle_client = MockOracleClient::new(&env, &oracle_address);
    oracle_client.set_release(&false);

    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);
    client.deposit(&vault_id, &owner, &10_000);

    client.set_release_condition(&vault_id, &owner, &ReleaseCondition::Oracle(oracle_address));

    // Trigger release should fail with ConditionsNotApproved (error code 33)
    let err = client.try_trigger_release(&vault_id).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(33));

    // Vault remains locked and funds intact
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Locked);
    assert_eq!(vault.balance, 10_000);
}

#[test]
fn test_oracle_gated_release_state_transition() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let oracle_address = env.register_contract(None, MockOracle);
    let oracle_client = MockOracleClient::new(&env, &oracle_address);

    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);
    client.deposit(&vault_id, &owner, &50_000);
    client.set_release_condition(&vault_id, &owner, &ReleaseCondition::Oracle(oracle_address));

    // 1. Oracle says false -> release blocked
    oracle_client.set_release(&false);
    assert!(client.try_trigger_release(&vault_id).is_err());

    // 2. Oracle reports true (event occurred) -> release succeeds
    oracle_client.set_release(&true);
    client.trigger_release(&vault_id);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Released);
    assert_eq!(vault.balance, 0);

    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&beneficiary), 50_000);
}

#[test]
fn test_multi_condition_release_oracle_or_ttl() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let oracle_address = env.register_contract(None, MockOracle);
    let oracle_client = MockOracleClient::new(&env, &oracle_address);
    oracle_client.set_release(&false);

    let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
    client.deposit(&vault_id, &owner, &20_000);

    // Set multiple conditions: TTL expiry OR Oracle trigger
    let conditions = vec![
        &env,
        ReleaseCondition::TTLExpiry,
        ReleaseCondition::Oracle(oracle_address.clone()),
    ];
    client.set_release_conditions(&vault_id, &owner, &conditions);

    // Neither condition met yet -> fails with ConditionsNotApproved
    let err = client.try_trigger_release(&vault_id).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(33));

    // Now oracle triggers true -> release succeeds without waiting for TTL expiry
    oracle_client.set_release(&true);
    client.trigger_release(&vault_id);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Released);

    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&beneficiary), 20_000);
}

#[test]
fn test_multi_condition_release_via_ttl_when_oracle_false() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let oracle_address = env.register_contract(None, MockOracle);
    let oracle_client = MockOracleClient::new(&env, &oracle_address);
    oracle_client.set_release(&false);

    let interval = 100u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&vault_id, &owner, &20_000);

    let conditions = vec![
        &env,
        ReleaseCondition::TTLExpiry,
        ReleaseCondition::Oracle(oracle_address),
    ];
    client.set_release_conditions(&vault_id, &owner, &conditions);

    // Oracle is false, but TTL expires
    env.ledger().with_mut(|li| {
        li.timestamp += interval + 1;
    });

    // Release succeeds because TTL expiry condition is met
    client.trigger_release(&vault_id);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Released);

    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&beneficiary), 20_000);
}

#[test]
fn test_nonexistent_or_failing_oracle_handled_gracefully() {
    let (env, owner, beneficiary, _, _, client) = setup();

    // Random non-contract address as oracle
    let invalid_oracle = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);
    client.deposit(&vault_id, &owner, &10_000);

    client.set_release_condition(&vault_id, &owner, &ReleaseCondition::Oracle(invalid_oracle));

    // Should return ConditionsNotApproved error rather than panicking uncontrollably
    let err = client.try_trigger_release(&vault_id).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(33));
}

#[test]
fn test_multi_beneficiary_split_with_oracle_release() {
    let (env, owner, primary_ben, _, token_address, client) = setup();

    let ben1 = Address::generate(&env);
    let ben2 = Address::generate(&env);

    let oracle_address = env.register_contract(None, MockOracle);
    let oracle_client = MockOracleClient::new(&env, &oracle_address);
    oracle_client.set_release(&true);

    let vault_id = client.create_vault(&owner, &primary_ben, &1000u64, &None);
    client.deposit(&vault_id, &owner, &100_000);

    let beneficiaries = vec![
        &env,
        BeneficiaryEntry {
            address: ben1.clone(),
            bps: 6000,
            minimum_threshold: 0,
        },
        BeneficiaryEntry {
            address: ben2.clone(),
            bps: 4000,
            minimum_threshold: 0,
        },
    ];
    client.set_beneficiaries(&vault_id, &owner, &beneficiaries);
    client.set_release_condition(&vault_id, &owner, &ReleaseCondition::Oracle(oracle_address));

    client.trigger_release(&vault_id);

    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&ben1), 60_000);
    assert_eq!(token_client.balance(&ben2), 40_000);
}

#[test]
fn test_only_owner_can_set_release_condition() {
    let (env, owner, beneficiary, _, _, client) = setup();
    let stranger = Address::generate(&env);
    let oracle_address = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);

    let err = client
        .try_set_release_condition(&vault_id, &stranger, &ReleaseCondition::Oracle(oracle_address))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(6)); // NotOwner
}
