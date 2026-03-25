use soroban_sdk::{contracterror, contracttype, Address};

/// Contract-level errors with explicit, human-readable codes.
#[contracterror]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaultError {
    /// Caller is not the vault owner. Only the owner may perform this action.
    NotOwner = 1,
    VaultNotFound = 2,
    AlreadyReleased = 3,
    NotExpired = 4,
    InsufficientBalance = 5,
    /// Amount must be greater than zero.
    InvalidAmount = 6,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Vault(u64),
    VaultCount,
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
