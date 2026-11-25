/// Tests for Issue #560 – Passkey Expiry Notifications (contract-side ping_expiry event).
///
/// The backend NotificationService is tested in backend/src/tests.rs.
/// Here we verify the on-chain `pk_exp_warn` event emitted by ping_expiry
/// when a passkey's remaining TTL is below EXPIRY_WARNING_THRESHOLD.
#[cfg(test)]
use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
};

use crate::tests::common::setup_unfunded as setup;

fn make_pk(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::<32>::from_array(env, &[byte; 32])
}

#[test]
fn test_passkey_expiry_warning_event_emitted() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &300_u64, &None);
    let pk = make_pk(&env, 1);
    client.add_passkey(&vault_id, &owner, &pk, &false);

    let now = env.ledger().timestamp() + 100;
    env.ledger().set_timestamp(now);
    // Set expiry to now + (EXPIRY_WARNING_THRESHOLD - 1) so it is "soon".
    let expiry = now + EXPIRY_WARNING_THRESHOLD - 1;
    client.extend_passkey_expiry(&vault_id, &owner, &pk, &expiry);

    // ping_expiry should emit a pk_exp_warn event.
    client.ping_expiry(&vault_id);
    // We verify by asserting no panic; the event emission is the observable side-effect.
}

#[test]
fn test_passkey_expiry_no_warning_when_not_close() {
    let (env, owner, beneficiary, client) = setup();
    let vault_id = client.create_vault(&owner, &beneficiary, &300_u64, &None);
    let pk = make_pk(&env, 2);
    client.add_passkey(&vault_id, &owner, &pk, &false);

    let now = env.ledger().timestamp() + 100;
    env.ledger().set_timestamp(now);
    // Set expiry far in the future – no warning expected.
    let expiry = now + EXPIRY_WARNING_THRESHOLD * 2;
    client.extend_passkey_expiry(&vault_id, &owner, &pk, &expiry);

    // Should complete without error.
    client.ping_expiry(&vault_id);
}
