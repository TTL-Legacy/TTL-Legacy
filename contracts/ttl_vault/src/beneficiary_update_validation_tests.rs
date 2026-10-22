//! Tests for beneficiary address validation on updates (Issue #1268)

#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
};

use crate::tests::common::setup_funded as setup;

#[test]
#[should_panic]
fn test_update_beneficiary_rejects_zero_address() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    let zero = Address::from_contract_id(&env, &BytesN::<32>::zero(&env));
    client.update_beneficiary(&vault_id, &owner, &zero, &None, &None, &None);
}

#[test]
fn test_update_beneficiary_rejects_owner() {
    let (_env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);

    let result = client.try_update_beneficiary(&vault_id, &owner, &owner, &None, &None, &None);
    assert_eq!(result, Err(Ok(ContractError::InvalidBeneficiary)));
}

#[test]
fn test_update_beneficiary_accepts_valid_address() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &MIN_CHECK_IN_INTERVAL, &None);
    let new_beneficiary = Address::generate(&env);

    client.update_beneficiary(&vault_id, &owner, &new_beneficiary, &None, &None, &None);

    env.ledger().set_timestamp(env.ledger().timestamp() + 86_400);
    client.apply_beneficiary_update(&vault_id, &owner);

    assert_eq!(client.get_beneficiary(&vault_id), new_beneficiary);
}
