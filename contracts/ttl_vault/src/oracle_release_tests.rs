#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl, testutils::Address as _, vec, Address, Env,
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

use crate::tests::common::setup_with_roles as setup;

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

// ---------------------------------------------------------------------------
// Test: missing oracle
// A vault whose release condition points to an address that has no deployed
// contract (i.e. the oracle is "missing") must:
//   1. Not panic — the graceful-fallback in oracle::query() returns Ok(false).
//   2. Keep the vault Locked with funds intact.
//   3. Emit ConditionsNotApproved (error 33) to the caller.
// ---------------------------------------------------------------------------
#[test]
fn test_missing_oracle_returns_conditions_not_approved_and_vault_stays_locked() {
    let (env, owner, beneficiary, _, _, client) = setup();

    // A freshly-generated address has no contract deployed behind it — it is
    // the canonical "missing oracle" scenario.
    let missing_oracle = Address::generate(&env);

    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.deposit(&vault_id, &owner, &25_000);
    client.set_release_condition(
        &vault_id,
        &owner,
        &ReleaseCondition::Oracle(missing_oracle.clone()),
    );

    // Attempting to release while the oracle address resolves to nothing must
    // surface ConditionsNotApproved (error 33), not an unhandled panic.
    let err = client
        .try_trigger_release(&vault_id)
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from_contract_error(33),
        "expected ConditionsNotApproved when oracle contract is missing"
    );

    // Vault must remain Locked with full balance untouched.
    let vault = client.get_vault(&vault_id);
    assert_eq!(
        vault.status,
        ReleaseStatus::Locked,
        "vault must stay Locked after missing-oracle call"
    );
    assert_eq!(
        vault.balance, 25_000,
        "vault balance must be unchanged after missing-oracle call"
    );

    // Confirm it is also still not expired — this is a pure oracle failure,
    // not a TTL issue.
    assert!(
        !client.is_expired(&vault_id),
        "vault must not appear expired after missing-oracle call"
    );
}

// ---------------------------------------------------------------------------
// Test: stale oracle
// A stale oracle is one that has been deployed and responds, but has not been
// updated — it keeps returning `false` even as ledger time advances well past
// the vault's check-in interval.  The system must:
//   1. Never release funds based on the passage of time alone when the
//      release condition is Oracle-only (not TTLExpiry).
//   2. Continue returning ConditionsNotApproved on every attempt.
//   3. Release correctly the moment the oracle is finally updated to `true`.
// ---------------------------------------------------------------------------
#[test]
fn test_stale_oracle_blocks_release_regardless_of_time_passing() {
    let (env, owner, beneficiary, _, token_address, client) = setup();

    let oracle_address = env.register_contract(None, MockOracle);
    let oracle_client = MockOracleClient::new(&env, &oracle_address);

    // Oracle starts stale — deployed but perpetually returning false.
    oracle_client.set_release(&false);

    // Short check-in interval so advancing time is cheap in the test.
    let interval = 100u64;
    let vault_id = client.create_vault(&owner, &beneficiary, &interval, &None);
    client.deposit(&vault_id, &owner, &40_000);

    // Pure oracle condition — NOT combined with TTLExpiry, so time alone
    // must never trigger a release.
    client.set_release_condition(
        &vault_id,
        &owner,
        &ReleaseCondition::Oracle(oracle_address.clone()),
    );

    // Advance ledger well past the TTL — stale oracle should still block.
    env.ledger().with_mut(|li| {
        li.timestamp += interval * 10; // 10× the check-in interval
    });

    // Even though the vault would be "expired" by TTL, an Oracle-only
    // condition must not be satisfied by time.
    let err = client
        .try_trigger_release(&vault_id)
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from_contract_error(33),
        "stale oracle must block release even after TTL would have expired"
    );

    // Sanity: vault is still locked and fully funded.
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Locked);
    assert_eq!(vault.balance, 40_000);

    // --- Now the oracle is finally updated (the real-world event occurred) ---
    oracle_client.set_release(&true);

    client.trigger_release(&vault_id);

    let vault = client.get_vault(&vault_id);
    assert_eq!(
        vault.status,
        ReleaseStatus::Released,
        "vault must release once the stale oracle is updated to true"
    );
    assert_eq!(vault.balance, 0);

    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(
        token_client.balance(&beneficiary),
        40_000,
        "beneficiary must receive full balance after oracle update"
    );
}

// ---------------------------------------------------------------------------
// Test: unauthorized oracle update
// Only the vault owner may call set_release_condition or
// set_release_conditions.  Any other caller — including a beneficiary or a
// random third party — must be rejected with NotOwner (error 6).
// This ensures a malicious actor cannot silently swap the oracle address to
// one they control and force an unauthorized release.
// ---------------------------------------------------------------------------
#[test]
fn test_unauthorized_oracle_update_is_rejected() {
    let (env, owner, beneficiary, _, _, client) = setup();

    let attacker = Address::generate(&env);

    // Attacker deploys their own oracle that always returns true.
    let malicious_oracle = env.register_contract(None, MockOracle);
    let malicious_oracle_client = MockOracleClient::new(&env, &malicious_oracle);
    malicious_oracle_client.set_release(&true);

    // Legitimate oracle that returns false (vault should NOT be releasable).
    let legitimate_oracle = env.register_contract(None, MockOracle);
    let legitimate_oracle_client = MockOracleClient::new(&env, &legitimate_oracle);
    legitimate_oracle_client.set_release(&false);

    let vault_id = client.create_vault(&owner, &beneficiary, &3600u64, &None);
    client.deposit(&vault_id, &owner, &75_000);

    // Owner correctly sets the legitimate oracle.
    client.set_release_condition(
        &vault_id,
        &owner,
        &ReleaseCondition::Oracle(legitimate_oracle.clone()),
    );

    // Attacker tries to overwrite the oracle condition with their own address
    // via set_release_condition — must be rejected with NotOwner (6).
    let err_single = client
        .try_set_release_condition(
            &vault_id,
            &attacker,
            &ReleaseCondition::Oracle(malicious_oracle.clone()),
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err_single,
        soroban_sdk::Error::from_contract_error(6),
        "set_release_condition must reject non-owner caller with NotOwner"
    );

    // Attacker also tries the multi-condition path.
    let malicious_conditions = vec![
        &env,
        ReleaseCondition::Oracle(malicious_oracle.clone()),
    ];
    let err_multi = client
        .try_set_release_conditions(&vault_id, &attacker, &malicious_conditions)
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err_multi,
        soroban_sdk::Error::from_contract_error(6),
        "set_release_conditions must reject non-owner caller with NotOwner"
    );

    // Beneficiary is also not the owner — same rejection expected.
    let err_beneficiary = client
        .try_set_release_condition(
            &vault_id,
            &beneficiary,
            &ReleaseCondition::Oracle(malicious_oracle.clone()),
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err_beneficiary,
        soroban_sdk::Error::from_contract_error(6),
        "beneficiary must not be able to overwrite oracle condition"
    );

    // Verify the vault condition was never changed — the legitimate (false)
    // oracle is still in effect, so trigger_release must still be blocked.
    let err_release = client
        .try_trigger_release(&vault_id)
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err_release,
        soroban_sdk::Error::from_contract_error(33),
        "vault must still be blocked by the original legitimate oracle"
    );

    // And vault remains fully intact.
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status, ReleaseStatus::Locked);
    assert_eq!(vault.balance, 75_000);
}
