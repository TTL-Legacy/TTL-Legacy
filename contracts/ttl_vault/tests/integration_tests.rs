#![cfg(test)]

extern crate alloc;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{self, StellarAssetClient},
    Address, BytesN, Env,
};

use ttl_vault::{ReleaseStatus, TtlVaultContract, TtlVaultContractClient};

/// Integration test configuration (kept for live testnet use)
pub struct TestnetConfig {
    pub rpc_url: &'static str,
    pub network_passphrase: &'static str,
    pub contract_id: Option<&'static str>,
}

impl TestnetConfig {
    pub fn testnet() -> Self {
        Self {
            rpc_url: "https://soroban-testnet.stellar.org",
            network_passphrase: "Test SDF Network ; September 2015",
            contract_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Issue #1178: Full vault lifecycle in-process test
// Sequence: create_vault → deposit → check_in → advance ledger past TTL
//           → trigger_release → assert beneficiary balance + vault status
// ---------------------------------------------------------------------------

/// Sets up a fresh Soroban environment with a deployed TtlVaultContract and a
/// funded token account for the owner.
fn setup_integration() -> (
    Env,
    Address, // owner
    Address, // beneficiary
    Address, // token address
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
    // Mint enough for deposits
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    // SAFETY: lifetime transmute mirrors the pattern used across the test suite
    // (`lifecycle_tests.rs`, `test.rs`). The environment outlives every call made
    // through the client within the enclosing test function.
    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    (env, owner, beneficiary, token_address, client)
}

/// Full vault lifecycle:
/// create_vault → deposit → check_in → advance ledger past TTL → trigger_release
///
/// Asserts:
/// - beneficiary balance increases by exactly the deposited amount
/// - vault balance is 0 after release
/// - vault status is `Released` after trigger
#[test]
fn lifecycle_full_flow() {
    let (env, owner, beneficiary, token_address, client) = setup_integration();

    let check_in_interval = 1_000u64;
    let deposit_amount = 500_000i128;

    // 1. Create vault
    let vault_id = client.create_vault(&owner, &beneficiary, &check_in_interval, &None);
    assert_eq!(
        client.get_vault(&vault_id).balance,
        0,
        "vault balance must be 0 after creation"
    );

    // 2. Deposit funds
    client.deposit(&vault_id, &owner, &deposit_amount);
    assert_eq!(
        client.get_vault(&vault_id).balance,
        deposit_amount,
        "vault balance must equal deposited amount"
    );

    // 3. Check-in to prove liveness (resets the TTL countdown)
    env.ledger()
        .with_mut(|l| l.timestamp = check_in_interval / 2);
    client.check_in(
        &vault_id,
        &owner,
        &BytesN::from_array(&env, &[1u8; 32]),
        &0u64,
    );
    assert!(
        !client.is_expired(&vault_id),
        "vault must not be expired after check-in"
    );

    // 4. Advance ledger past the check-in interval — vault now expires
    env.ledger()
        .with_mut(|l| l.timestamp += check_in_interval + 1);
    assert!(
        client.is_expired(&vault_id),
        "vault must be expired after missing check-in deadline"
    );

    // 5. Trigger release and verify beneficiary receives the full deposit
    let token_client = token::Client::new(&env, &token_address);
    let balance_before = token_client.balance(&beneficiary);

    client.trigger_release(&vault_id);

    let balance_after = token_client.balance(&beneficiary);
    assert_eq!(
        balance_after - balance_before,
        deposit_amount,
        "beneficiary must receive exactly the deposited amount"
    );

    // 6. Vault balance zeroed out
    assert_eq!(
        client.get_vault(&vault_id).balance,
        0,
        "vault balance must be 0 after release"
    );

    // 7. Vault status is Released
    assert_eq!(
        client.get_vault(&vault_id).status,
        ReleaseStatus::Released,
        "vault status must be Released after trigger_release"
    );
}

// ---------------------------------------------------------------------------
// Live testnet integration tests (kept as stubs — run with --ignored flag)
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn integration_full_vault_lifecycle() {
    let config = TestnetConfig::testnet();
    println!("Integration test: Full vault lifecycle");
    println!("RPC: {}", config.rpc_url);
    println!("Network: {}", config.network_passphrase);
}

#[test]
#[ignore]
fn integration_vault_creation_and_deposit() {
    println!("Integration test: Vault creation and deposit");
}

#[test]
#[ignore]
fn integration_checkin_extends_ttl() {
    println!("Integration test: Check-in extends TTL");
}

#[test]
#[ignore]
fn integration_passkey_authentication() {
    println!("Integration test: Passkey authentication");
}

#[test]
#[ignore]
fn integration_fee_calculation_and_transfers() {
    println!("Integration test: Fee calculation and transfers");
}

#[test]
#[ignore]
fn integration_beneficiary_payout_on_expiry() {
    println!("Integration test: Beneficiary payout on expiry");
}

#[test]
#[ignore]
fn integration_multiple_vaults_isolation() {
    println!("Integration test: Multiple vaults isolation");
}

#[test]
#[ignore]
fn integration_error_handling() {
    println!("Integration test: Error handling");
}

#[test]
#[ignore]
fn integration_state_persistence() {
    println!("Integration test: State persistence");
}

#[test]
#[ignore]
fn integration_network_latency_handling() {
    println!("Integration test: Network latency handling");
}
