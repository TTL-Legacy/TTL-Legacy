use soroban_sdk::{contracttype, Address, Symbol, Env};

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

pub struct VaultCreatedEvent {
    pub vault_id: u64,
    pub owner: Address,
    pub beneficiary: Address,
    pub check_in_interval: u64,
}

impl VaultCreatedEvent {
    pub fn new(vault_id: u64, owner: Address, beneficiary: Address, check_in_interval: u64) -> Self {
        Self {
            vault_id,
            owner,
            beneficiary,
            check_in_interval,
        }
    }

    pub fn topic(&self) -> Symbol {
        Symbol::new(&Env::default(), "v_created")
    }

    pub fn to_tuple(&self) -> (u64, Address, Address, u64) {
        (self.vault_id, self.owner.clone(), self.beneficiary.clone(), self.check_in_interval)
    }
}
