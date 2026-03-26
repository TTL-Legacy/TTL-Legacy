use soroban_sdk::{contracterror, contracttype, Address};

#[contracterror]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ContractError {
    VaultNotFound = 1,
    InvalidInterval = 2,
    EmptyVault = 3,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Vault(u64),
    VaultCount,
    TokenAddress,
}

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum ReleaseStatus {
    Locked,
    Released,
}

#[contracttype]
#[derive(Clone)]
pub struct Vault {
    pub owner: Address,
    pub beneficiary: Address,
    pub balance: i128,
    pub check_in_interval: u64, // seconds
    pub last_check_in: u64,     // ledger timestamp
    pub status: ReleaseStatus,
}
