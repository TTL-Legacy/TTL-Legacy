//! Shared Soroban fixtures for the vault contract test modules (#1751).
//!
//! Every test module used to declare its own `setup()` function with the same
//! boilerplate: create an `Env`, mock auths, generate owner/beneficiary/admin
//! addresses, register a Stellar asset contract and mint to the owner, then
//! register and initialize `TtlVaultContract` and transmute the generated
//! client to `'static`.
//!
//! The helpers below are the single source of truth for that flow. Test
//! modules import the fixture they need under the local name `setup`, so test
//! bodies are unchanged:
//!
//! ```ignore
//! use crate::tests::common::setup;
//! ```

#![allow(dead_code)]

use soroban_sdk::{testutils::Address as _, token::StellarAssetClient, Address, Env};

use crate::{TtlVaultContract, TtlVaultContractClient};

/// `(env, owner, beneficiary, client)` with the owner funded with 10M units.
pub type VaultFixture = (Env, Address, Address, TtlVaultContractClient<'static>);

/// `(env, owner, beneficiary, token, client)` with the owner funded with 10M units.
pub type VaultTokenFixture = (
    Env,
    Address,
    Address,
    Address,
    TtlVaultContractClient<'static>,
);

/// `(env, owner, beneficiary, admin, token, client)` with the requested owner balance.
pub type VaultRolesFixture = (
    Env,
    Address,
    Address,
    Address,
    Address,
    TtlVaultContractClient<'static>,
);

/// Default owner balance used by the generated test fixtures.
pub const DEFAULT_OWNER_BALANCE: i128 = 10_000_000;

/// The most common fixture's owner balance.
pub const FUNDED_OWNER_BALANCE: i128 = 1_000_000;

/// Standard fixture: funded owner (10M), initialized contract.
pub fn setup() -> VaultFixture {
    setup_with_balance(DEFAULT_OWNER_BALANCE)
}

/// Fixture for modules that use the smaller 1M owner balance.
pub fn setup_funded() -> VaultFixture {
    setup_with_balance(FUNDED_OWNER_BALANCE)
}

/// Funded fixture with an explicit owner balance.
pub fn setup_with_balance(owner_balance: i128) -> VaultFixture {
    let (env, owner, beneficiary, _admin, _token, client) =
        setup_with_roles_and_balance(owner_balance);
    (env, owner, beneficiary, client)
}

/// Fixture that also returns the token address and the admin address.
///
/// Used by modules that need to assert on token balances or initialize with a
/// specific admin.
pub fn setup_with_roles() -> VaultRolesFixture {
    setup_with_roles_and_balance(FUNDED_OWNER_BALANCE)
}

/// `setup_with_roles` with an explicit owner balance.
pub fn setup_with_roles_and_balance(owner_balance: i128) -> VaultRolesFixture {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let admin = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &owner_balance);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);
    client.initialize(&token_address, &admin);

    // The generated client borrows the `Env`. The fixture owns that `Env` for
    // the whole test, so widening the client's lifetime to `'static` is sound
    // in this test-only context. This is the pattern the test modules used
    // locally before the fixtures were consolidated.
    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    (env, owner, beneficiary, admin, token_address, client)
}

/// Fixture that returns the token address (10M owner balance).
pub fn setup_with_token() -> VaultTokenFixture {
    let (env, owner, beneficiary, _admin, token_address, client) =
        setup_with_roles_and_balance(DEFAULT_OWNER_BALANCE);
    (env, owner, beneficiary, token_address, client)
}

/// Fixture with a plain (non-Stellar-asset) token address and no funding.
///
/// Used by tests that never move tokens and only need an initialized contract.
pub fn setup_unfunded() -> VaultFixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    let contract_address = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_address);

    let token_address = Address::generate(&env);
    client.initialize(&token_address, &admin);

    // See `setup_with_roles_and_balance` for why this transmute is sound.
    let client: TtlVaultContractClient<'static> = unsafe { core::mem::transmute(client) };

    (env, owner, beneficiary, client)
}
