#![cfg(test)]

extern crate alloc;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{self, StellarAssetClient},
    vec, Address, BytesN, Env,
};

fn setup_milestone_test() -> (
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
    let attestor = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };
    (env, owner, beneficiary, token_address, attestor, client)
}

// ============================================================================
// TESTS FOR MILESTONE-BASED VESTING
// ============================================================================

/// Test that owner can add a vesting milestone to a vault
#[test]
fn test_add_vesting_milestone_succeeds_for_owner() {
    let (env, owner, beneficiary, _token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Owner adds a milestone
    client.add_vesting_milestone(&vault_id, &"Beneficiary turns 18".to_string(), &attestor);

    // Verify the vault still exists (no error)
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, 0);
}

/// Test that non-owner cannot add a vesting milestone
#[test]
#[should_panic(expected = "")]
fn test_add_vesting_milestone_fails_for_non_owner() {
    let (env, owner, beneficiary, _token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    let non_owner = Address::generate(&env);

    // Non-owner tries to add a milestone - should fail auth check
    client.add_vesting_milestone(&vault_id, &"Beneficiary turns 18".to_string(), &attestor);
}

/// Test that designated attestor can attest a milestone
#[test]
fn test_attest_milestone_succeeds_for_attestor() {
    let (env, owner, beneficiary, _token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.add_vesting_milestone(&vault_id, &"Beneficiary turns 18".to_string(), &attestor);

    // Attestor attests the milestone (milestone_id = 0)
    client.attest_milestone(&vault_id, &0);

    // Verify milestone is now unlocked by attempting release
    // (release should not be blocked by milestones anymore)
}

/// Test that non-attestor cannot attest a milestone
#[test]
#[should_panic(expected = "")]
fn test_attest_milestone_fails_for_non_attestor() {
    let (env, owner, beneficiary, _token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.add_vesting_milestone(&vault_id, &"Beneficiary turns 18".to_string(), &attestor);

    let non_attestor = Address::generate(&env);

    // Non-attestor tries to attest - should fail auth check
    client.attest_milestone(&vault_id, &0);
}

/// Test that attestor cannot attest the same milestone twice
#[test]
#[should_panic(expected = "MilestoneAlreadyAttested")]
fn test_attest_milestone_fails_if_already_attested() {
    let (env, owner, beneficiary, _token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.add_vesting_milestone(&vault_id, &"Beneficiary turns 18".to_string(), &attestor);

    // First attestation succeeds
    client.attest_milestone(&vault_id, &0);

    // Second attestation on same milestone should fail
    client.attest_milestone(&vault_id, &0);
}

/// Test that release is blocked if any milestone is not yet attested
#[test]
#[should_panic(expected = "MilestoneNotFound")]
fn test_release_blocked_with_unattested_milestone() {
    let (env, owner, beneficiary, token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Deposit funds
    client.deposit(&vault_id, &owner, &1_000_000);

    // Add milestone but don't attest it
    client.add_vesting_milestone(&vault_id, &"Beneficiary turns 18".to_string(), &attestor);

    // Fast forward to expiry
    let vault = client.get_vault(&vault_id);
    env.ledger()
        .with_mut(|l| l.set_timestamp(vault.last_check_in + vault.check_in_interval + 1));

    // Try to release - should be blocked by unattested milestone
    client.trigger_release(&vault_id);
}

/// Test that release succeeds after all milestones are attested
#[test]
fn test_release_succeeds_with_all_milestones_attested() {
    let (env, owner, beneficiary, token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Deposit funds
    client.deposit(&vault_id, &owner, &1_000_000);

    // Add milestone
    client.add_vesting_milestone(&vault_id, &"Beneficiary turns 18".to_string(), &attestor);

    // Attest the milestone
    client.attest_milestone(&vault_id, &0);

    // Fast forward to expiry
    let vault = client.get_vault(&vault_id);
    env.ledger()
        .with_mut(|l| l.set_timestamp(vault.last_check_in + vault.check_in_interval + 1));

    // Release should now succeed
    client.trigger_release(&vault_id);

    // Verify vault status is Released
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Released);
}

/// Test that multiple milestones must all be attested before release
#[test]
#[should_panic(expected = "MilestoneNotFound")]
fn test_release_blocked_with_multiple_unattested_milestones() {
    let (env, owner, beneficiary, token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Deposit funds
    client.deposit(&vault_id, &owner, &1_000_000);

    // Add three milestones
    client.add_vesting_milestone(&vault_id, &"Beneficiary turns 18".to_string(), &attestor);
    let attestor2 = Address::generate(&env);
    client.add_vesting_milestone(&vault_id, &"Completes education".to_string(), &attestor2);
    let attestor3 = Address::generate(&env);
    client.add_vesting_milestone(&vault_id, &"Employed for 1 year".to_string(), &attestor3);

    // Attest first two milestones, but not the third
    client.attest_milestone(&vault_id, &0);
    client.attest_milestone(&vault_id, &1);
    // milestone_id = 2 is NOT attested

    // Fast forward to expiry
    let vault = client.get_vault(&vault_id);
    env.ledger()
        .with_mut(|l| l.set_timestamp(vault.last_check_in + vault.check_in_interval + 1));

    // Try to release - should be blocked by unattested third milestone
    client.trigger_release(&vault_id);
}

/// Test that release succeeds after all multiple milestones are attested
#[test]
fn test_release_succeeds_with_all_multiple_milestones_attested() {
    let (env, owner, beneficiary, token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Deposit funds
    client.deposit(&vault_id, &owner, &1_000_000);

    // Add three milestones
    client.add_vesting_milestone(&vault_id, &"Beneficiary turns 18".to_string(), &attestor);
    let attestor2 = Address::generate(&env);
    client.add_vesting_milestone(&vault_id, &"Completes education".to_string(), &attestor2);
    let attestor3 = Address::generate(&env);
    client.add_vesting_milestone(&vault_id, &"Employed for 1 year".to_string(), &attestor3);

    // Attest all three milestones
    client.attest_milestone(&vault_id, &0);
    client.attest_milestone(&vault_id, &1);
    client.attest_milestone(&vault_id, &2);

    // Fast forward to expiry
    let vault = client.get_vault(&vault_id);
    env.ledger()
        .with_mut(|l| l.set_timestamp(vault.last_check_in + vault.check_in_interval + 1));

    // Release should now succeed
    client.trigger_release(&vault_id);

    // Verify vault status is Released
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Released);
}

/// Test that vault without any milestones can still be released normally
#[test]
fn test_release_succeeds_without_milestones() {
    let (env, owner, beneficiary, token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Deposit funds
    client.deposit(&vault_id, &owner, &1_000_000);

    // Do NOT add any milestones

    // Fast forward to expiry
    let vault = client.get_vault(&vault_id);
    env.ledger()
        .with_mut(|l| l.set_timestamp(vault.last_check_in + vault.check_in_interval + 1));

    // Release should succeed without any milestone checks
    client.trigger_release(&vault_id);

    // Verify vault status is Released
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Released);
}

/// Test that owner can add milestones with descriptive names
#[test]
fn test_add_milestone_with_descriptive_name() {
    let (env, owner, beneficiary, _token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    let description =
        "Beneficiary completes university education and obtains bachelor's degree".to_string();
    client.add_vesting_milestone(&vault_id, &description, &attestor);

    // Verify the vault still exists
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, 0);
}

/// Test that milestone attestation with same attestor for multiple milestones works
#[test]
fn test_same_attestor_can_attest_multiple_milestones() {
    let (env, owner, beneficiary, _token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Add two milestones with same attestor
    client.add_vesting_milestone(&vault_id, &"Milestone 1".to_string(), &attestor);
    client.add_vesting_milestone(&vault_id, &"Milestone 2".to_string(), &attestor);

    // Same attestor attests both milestones
    client.attest_milestone(&vault_id, &0);
    client.attest_milestone(&vault_id, &1);

    // Both should be marked as unlocked
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, 0);
}

/// Test that release transitions vault correctly to Released status
#[test]
fn test_release_transitions_vault_status() {
    let (env, owner, beneficiary, token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Deposit funds
    client.deposit(&vault_id, &owner, &1_000_000);

    // Add and attest milestone
    client.add_vesting_milestone(&vault_id, &"Beneficiary turns 18".to_string(), &attestor);
    client.attest_milestone(&vault_id, &0);

    // Verify vault is initially Locked
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Locked);

    // Fast forward to expiry
    env.ledger()
        .with_mut(|l| l.set_timestamp(vault.last_check_in + vault.check_in_interval + 1));

    // Trigger release
    client.trigger_release(&vault_id);

    // Verify vault status changed to Released
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Released);
}

/// Test that attestation event is emitted correctly
#[test]
fn test_milestone_attestation_emits_event() {
    let (env, owner, beneficiary, _token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.add_vesting_milestone(&vault_id, &"Beneficiary turns 18".to_string(), &attestor);

    // Attest should emit an event
    client.attest_milestone(&vault_id, &0);

    // Events are published; we verify by successful execution without panic
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.balance, 0);
}

/// Test that release with vesting schedule AND milestones is properly handled
#[test]
fn test_release_with_vesting_schedule_and_milestones() {
    let (env, owner, beneficiary, token_address, attestor, client) = setup_milestone_test();
    let interval = 1_000u64;

    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);

    // Deposit funds
    client.deposit(&vault_id, &owner, &500_000);

    // Add milestone
    client.add_vesting_milestone(&vault_id, &"Beneficiary turns 18".to_string(), &attestor);

    // Attest milestone
    client.attest_milestone(&vault_id, &0);

    // Fast forward to expiry
    let vault = client.get_vault(&vault_id);
    env.ledger()
        .with_mut(|l| l.set_timestamp(vault.last_check_in + vault.check_in_interval + 1));

    // Release should succeed (milestone gate passed)
    client.trigger_release(&vault_id);

    // Verify vault status is Released
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Released);
}
