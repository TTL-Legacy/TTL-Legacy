use soroban_sdk::{contracttype, symbol_short, Address, Symbol};

pub const RELEASE_TOPIC: Symbol = symbol_short!("release");
pub const VAULT_CREATED_TOPIC: Symbol = symbol_short!("v_created");

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Vault(u64),
    OwnerVaults(Address),
    VaultCount,
    TokenAddress,
    Admin,
    Paused,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ReleaseStatus {
    Locked,
    Released,
    Cancelled,
}

#[contracttype]
#[derive(Clone)]
pub struct ReleaseEvent {
    pub vault_id: u64,
    pub beneficiary: Address,
    pub amount: i128,
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
