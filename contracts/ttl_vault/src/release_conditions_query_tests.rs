#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::tests::common::setup_funded as setup;

#[test]
fn test_get_release_conditions_empty_by_default() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    let conditions = client.get_release_conditions(&vault_id);
    assert_eq!(conditions, vec![&env]);
}

#[test]
fn test_get_release_conditions_single() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    let expected = vec![&env, ReleaseCondition::TTLExpiry];
    client.set_release_conditions(&vault_id, &owner, &expected);

    assert_eq!(client.get_release_conditions(&vault_id), expected);
}

#[test]
fn test_get_release_conditions_multiple() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let oracle = Address::generate(&env);

    let expected = vec![
        &env,
        ReleaseCondition::TTLExpiry,
        ReleaseCondition::OwnerInitiated,
        ReleaseCondition::Oracle(oracle),
    ];
    client.set_release_conditions(&vault_id, &owner, &expected);

    assert_eq!(client.get_release_conditions(&vault_id), expected);
}
