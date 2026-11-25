#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

fn setup() -> (
    Env,
    Address,
    Vec<Address>,
    u64,
    u32,
    TtlVaultContractClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiaries = vec![
        &env,
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &4_000_000);

    let contract = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract);
    client.initialize(&token_address, &admin);
    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    let vault_id = client.create_vault(&owner, &beneficiaries.get(0).unwrap(), &3600u64, &None);
    client.deposit(&vault_id, &owner, &4_000_000);
    client.set_beneficiaries(
        &vault_id,
        &owner,
        &vec![
            &env,
            BeneficiaryEntry {
                address: beneficiaries.get(0).unwrap(),
                bps: 3334,
                minimum_threshold: 0,
            },
            BeneficiaryEntry {
                address: beneficiaries.get(1).unwrap(),
                bps: 3333,
                minimum_threshold: 0,
            },
            BeneficiaryEntry {
                address: beneficiaries.get(2).unwrap(),
                bps: 3333,
                minimum_threshold: 0,
            },
        ],
    );

    let start_time = 3800u64;
    let schedule_id = client.set_vesting_schedule(
        &vault_id,
        &owner,
        &start_time,
        &100u64,
        &4u32,
        &4_000_000,
        &0,
    );
    client.set_release_conditions(&vault_id, &owner, &vec![&env, ReleaseCondition::TTLExpiry]);
    env.ledger().set_timestamp(3601);
    client.trigger_release(&vault_id);
    env.ledger().set_timestamp(start_time + 1);
    client.claim_vested(&vault_id, &schedule_id, &beneficiaries.get(0).unwrap());

    (env, owner, beneficiaries, vault_id, schedule_id, client)
}

fn proposed_schedule(env: &Env, claimed_installments: u32) -> VestingSchedule {
    VestingSchedule {
        start_time: env.ledger().timestamp(),
        interval: 50,
        num_installments: 4,
        claimed_installments,
        total_amount: 4_000_000,
        cliff_period: 20,
        cliff_grace_period_seconds: 10,
    }
}

#[test]
fn beneficiary_majority_approves_amendment_and_preserves_claims() {
    let (env, owner, beneficiaries, vault_id, schedule_id, client) = setup();
    let schedule = proposed_schedule(&env, 0);
    let proposal_id = client.propose_vesting_amendment(&vault_id, &owner, &schedule_id, &schedule);
    let proposal = client
        .get_vesting_amendment_proposal(&vault_id, &proposal_id)
        .unwrap();

    assert_eq!(proposal.required_approvals, 2);
    assert_eq!(proposal.approvals.len(), 0);
    assert_eq!(proposal.schedule.claimed_installments, 1);
    assert_eq!(
        client
            .get_vesting_schedule_by_id(&vault_id, &schedule_id)
            .unwrap()
            .interval,
        100
    );

    let stranger = Address::generate(&env);
    let result = client.try_approve_vesting_amendment(&vault_id, &proposal_id, &stranger);
    assert_eq!(result, Err(Ok(ContractError::NotBeneficiary)));

    client.approve_vesting_amendment(&vault_id, &proposal_id, &beneficiaries.get(0).unwrap());
    assert_eq!(
        client
            .get_vesting_amendment_proposal(&vault_id, &proposal_id)
            .unwrap()
            .status,
        ProposalStatus::Pending
    );
    client.approve_vesting_amendment(&vault_id, &proposal_id, &beneficiaries.get(1).unwrap());

    let proposal = client
        .get_vesting_amendment_proposal(&vault_id, &proposal_id)
        .unwrap();
    assert_eq!(proposal.status, ProposalStatus::Executed);
    let updated = client
        .get_vesting_schedule_by_id(&vault_id, &schedule_id)
        .unwrap();
    assert_eq!(updated.interval, 50);
    assert_eq!(updated.num_installments, 4);
    assert_eq!(updated.total_amount, 4_000_000);
    assert_eq!(updated.cliff_period, 20);
    assert_eq!(updated.cliff_grace_period_seconds, 10);
    assert_eq!(updated.claimed_installments, 1);
    env.ledger()
        .set_timestamp(updated.start_time + updated.cliff_period);
    assert_eq!(
        client.get_vesting_claimable(&vault_id, &schedule_id),
        0,
        "the first installment must not become claimable a second time"
    );
    env.ledger()
        .set_timestamp(updated.start_time + updated.interval);
    assert_eq!(
        client.get_vesting_claimable(&vault_id, &schedule_id),
        1_000_000
    );
}

#[test]
fn amendment_cannot_reduce_the_value_of_claimed_installments() {
    let (env, owner, _, vault_id, schedule_id, client) = setup();
    let mut schedule = proposed_schedule(&env, 0);
    schedule.total_amount = 3_999_999;

    let result = client.try_propose_vesting_amendment(&vault_id, &owner, &schedule_id, &schedule);
    assert_eq!(result, Err(Ok(ContractError::InvalidVestingSchedule)));
}

#[test]
fn expired_amendment_does_not_change_the_schedule() {
    let (env, owner, beneficiaries, vault_id, schedule_id, client) = setup();
    let schedule = proposed_schedule(&env, 0);
    let proposal_id = client.propose_vesting_amendment(&vault_id, &owner, &schedule_id, &schedule);
    let proposal = client
        .get_vesting_amendment_proposal(&vault_id, &proposal_id)
        .unwrap();

    env.ledger().set_timestamp(proposal.expires_at);
    client.approve_vesting_amendment(&vault_id, &proposal_id, &beneficiaries.get(0).unwrap());

    assert_eq!(
        client
            .get_vesting_amendment_proposal(&vault_id, &proposal_id)
            .unwrap()
            .status,
        ProposalStatus::Expired
    );
    assert_eq!(
        client
            .get_vesting_schedule_by_id(&vault_id, &schedule_id)
            .unwrap()
            .interval,
        100
    );
}
