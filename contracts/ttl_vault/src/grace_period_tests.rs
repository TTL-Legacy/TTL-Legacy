#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env,
};

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
fn test_grace_period_active_blocks_release() {
    let (env, owner, beneficiary, admin, token_address, client) = setup();

    // Set grace period to 100 seconds
    let mut config = client.get_protocol_config();
    config.release_grace_period_seconds = 100;
    client.propose_protocol_config(&config);

    // fast forward 24 hours (86400 seconds) to apply protocol config
    env.ledger().set_timestamp(env.ledger().timestamp() + 86400);
    client.apply_protocol_config();

    // Create vault with check-in interval of 500 seconds
    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    // Initial check in to reset the timestamp
    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None);

    // Fast forward to expiry (now + 500)
    now += 500;
    env.ledger().set_timestamp(now);

    // It is expired now, but grace period is 100 seconds
    // So attempting to trigger release should fail with GracePeriodActive
    let err = client.try_trigger_release(&vault_id).unwrap_err().unwrap();
    assert_eq!(err, soroban_sdk::Error::from_contract_error(83)); // GracePeriodActive

    // Fast forward past the grace period
    now += 100;
    env.ledger().set_timestamp(now);

    // Release should now succeed
    client.trigger_release(&vault_id);
    assert_eq!(client.get_vault(&vault_id).status, ReleaseStatus::Released);
}

#[test]
fn test_grace_period_default_allows_immediate_release() {
    let (env, owner, beneficiary, _admin, _token_address, client) = setup();

    // Create vault with check-in interval of 500 seconds
    let interval = 500u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let deposit_amount = 100_000i128;
    client.deposit(&owner, &vault_id, &deposit_amount);

    // Initial check in to reset the timestamp
    let mut now = env.ledger().timestamp() + 10;
    env.ledger().set_timestamp(now);
    client.check_in(&vault_id, &owner, &None);

    // Fast forward to expiry (now + 500)
    now += 500;
    env.ledger().set_timestamp(now);

    // Default grace period is 0, so release should succeed immediately
    client.trigger_release(&vault_id);
    assert_eq!(client.get_vault(&vault_id).status, ReleaseStatus::Released);
}
