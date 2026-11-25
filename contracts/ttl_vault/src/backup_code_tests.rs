#![cfg(test)]
use crate::{TtlVaultContract, TtlVaultContractClient, ContractError};
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, BytesN};

#[test]
fn test_backup_code_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, TtlVaultContract);
    let client = TtlVaultContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    client.initialize(&token, &admin);

    let owner = Address::generate(&env);
    let vault_id = client.create_vault(&owner, &owner, &86400, &false, &soroban_sdk::String::from_str(&env, "Test"));

    let codes = client.generate_backup_codes(&vault_id, &owner);
    assert_eq!(codes.len(), 10);

    let code = codes.get(0).unwrap();
    
    // First time use should succeed
    let result = client.use_backup_code(&vault_id, &code);
    assert_eq!(result, true);

    // Second time use should fail
    let err = client.try_use_backup_code(&vault_id, &code).unwrap_err().unwrap();
    assert_eq!(err, ContractError::BackupCodeAlreadyUsed);

    // Invalid code should fail
    let err2 = client.try_use_backup_code(&vault_id, &123456789).unwrap_err().unwrap();
    assert_eq!(err2, ContractError::InvalidBackupCode);
}
