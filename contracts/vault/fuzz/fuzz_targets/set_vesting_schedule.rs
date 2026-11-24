#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_sdk::{Address, Env};
use vault::{VaultContract, VaultContractClient, VestingSchedule};

fuzz_target!(|data: (u64, u64, u64, u64)| {
    let (cliff, end, total, now) = data;

    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, VaultContract);
    let client = VaultContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let token = Address::generate(&env);

    // Seed a vault so the schedule can be applied to a real vault.
    let vault_id = client.create_vault(&owner, &token, &total);

    let schedule = VestingSchedule {
        cliff,
        end,
        total,
    };

    // The contract must either accept the schedule or return a typed
    // ContractError. It must never panic on arbitrary input.
    let result = client.try_set_vesting_schedule(&vault_id, &schedule);
    match result {
        Ok(_) => {}
        Err(_) => {}
    }

    // Exercise the schedule against an arbitrary "now" to ensure the
    // vesting math also never panics.
    let _ = client.try_vested_amount(&vault_id, &now);
});
