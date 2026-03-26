#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, token, Address, Env,
    String, Vec,
};

mod types;
use types::{
    BeneficiaryEntry, DataKey, ReleaseEvent, ReleaseStatus, Vault, EXPIRY_WARNING_THRESHOLD,
    PING_EXPIRY_TOPIC, RELEASE_TOPIC, VAULT_CREATED_TOPIC,
};

#[cfg(test)]
mod test;

pub const VAULT_TTL_THRESHOLD: u32 = 1000;
pub const VAULT_TTL_LEDGERS: u32 = 200_000;
pub const INSTANCE_TTL_THRESHOLD: u32 = 1000;
pub const INSTANCE_TTL_LEDGERS: u32 = 200_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialized = 1,
    InvalidInterval = 2,
    VaultNotFound = 3,
    EmptyVault = 4,
    InvalidAmount = 5,
    NotOwner = 6,
    AlreadyReleased = 7,
    InsufficientBalance = 8,
    NotAdmin = 9,
    Paused = 10,
    InvalidBps = 11,
    NotExpiringSoon = 12,
}

#[contract]
pub struct TtlVaultContract;

#[contractimpl]
impl TtlVaultContract {
    // --- admin/config ---

    pub fn initialize(env: Env, xlm_token: Address, admin: Address) {
        if env.storage().instance().has(&DataKey::TokenAddress)
            || env.storage().instance().has(&DataKey::Admin)
        {
            panic_with_error!(&env, ContractError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::TokenAddress, &xlm_token);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    pub fn pause(env: Env) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Paused, &true);
    }

    pub fn unpause(env: Env) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    pub fn is_paused(env: Env) -> bool {
        Self::load_paused(&env)
    }

    // --- vault lifecycle ---

    pub fn create_vault(
        env: Env,
        owner: Address,
        beneficiary: Address,
        check_in_interval: u64,
    ) -> u64 {
        owner.require_auth();
        if check_in_interval == 0 {
            panic_with_error!(&env, ContractError::InvalidInterval);
        }
        let vault_id = Self::vault_count(env.clone()) + 1;
        let vault = Vault {
            owner: owner.clone(),
            beneficiary: beneficiary.clone(),
            balance: 0,
            check_in_interval,
            last_check_in: env.ledger().timestamp(),
            status: ReleaseStatus::Locked,
            beneficiaries: Vec::new(&env),
            metadata: String::from_str(&env, ""),
        };
        Self::save_vault(&env, vault_id, &vault);
        Self::add_owner_vault_id(&env, &owner, vault_id);
        env.storage().instance().set(&DataKey::VaultCount, &vault_id);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (VAULT_CREATED_TOPIC,),
            (vault_id, owner, beneficiary, check_in_interval),
        );
        vault_id
    }

    pub fn check_in(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        vault.last_check_in = env.ledger().timestamp();
        Self::save_vault(&env, vault_id, &vault);
        env.events().publish((symbol_short!("check_in"), vault_id), vault.last_check_in);
        Ok(())
    }

    pub fn deposit(env: Env, vault_id: u64, from: Address, amount: i128) {
        Self::assert_not_paused(&env);
        if amount <= 0 {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }
        from.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if vault.status != ReleaseStatus::Locked {
            panic_with_error!(&env, ContractError::AlreadyReleased);
        }
        let xlm = token::Client::new(&env, &Self::load_token(&env));
        xlm.transfer(&from, &env.current_contract_address(), &amount);
        vault.balance += amount;
        Self::save_vault(&env, vault_id, &vault);
    }

    pub fn withdraw(env: Env, vault_id: u64, amount: i128) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        let mut vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        if vault.balance < amount {
            return Err(ContractError::InsufficientBalance);
        }
        let xlm = token::Client::new(&env, &Self::load_token(&env));
        xlm.transfer(&env.current_contract_address(), &vault.owner, &amount);
        vault.balance -= amount;
        Self::save_vault(&env, vault_id, &vault);
        Ok(())
    }

    /// Anyone can call once TTL has lapsed. Splits funds to beneficiaries.
    pub fn trigger_release(env: Env, vault_id: u64) {
        Self::assert_not_paused(&env);
        let mut vault = Self::load_vault(&env, vault_id);
        if vault.status != ReleaseStatus::Locked {
            panic_with_error!(&env, ContractError::AlreadyReleased);
        }
        if !Self::is_expired(env.clone(), vault_id) {
            panic!("vault not yet expired");
        }
        if vault.balance == 0 {
            panic_with_error!(&env, ContractError::EmptyVault);
        }
        let total = vault.balance;
        let xlm = token::Client::new(&env, &Self::load_token(&env));

        if vault.beneficiaries.is_empty() {
            xlm.transfer(&env.current_contract_address(), &vault.beneficiary, &total);
            env.events().publish(
                (RELEASE_TOPIC,),
                ReleaseEvent { vault_id, beneficiary: vault.beneficiary.clone(), amount: total },
            );
        } else {
            let mut distributed: i128 = 0;
            let last_idx = vault.beneficiaries.len() - 1;
            for (i, entry) in vault.beneficiaries.iter().enumerate() {
                let share = if i as u32 == last_idx {
                    total - distributed
                } else {
                    total * (entry.bps as i128) / 10_000
                };
                xlm.transfer(&env.current_contract_address(), &entry.address, &share);
                distributed += share;
                env.events().publish(
                    (RELEASE_TOPIC,),
                    ReleaseEvent { vault_id, beneficiary: entry.address.clone(), amount: share },
                );
            }
        }

        vault.balance = 0;
        vault.status = ReleaseStatus::Released;
        Self::save_vault(&env, vault_id, &vault);
    }

    // --- Task 1: ping_expiry ---

    /// Callable by anyone. Emits a warning event if TTL remaining < EXPIRY_WARNING_THRESHOLD.
    pub fn ping_expiry(env: Env, vault_id: u64) -> u64 {
        let ttl = Self::get_ttl_remaining(env.clone(), vault_id).unwrap_or(0);
        if ttl < EXPIRY_WARNING_THRESHOLD {
            env.events().publish((PING_EXPIRY_TOPIC, vault_id), ttl);
        }
        ttl
    }

    // --- Task 2: partial_release ---

    /// Owner-only. Transfers `amount` to the beneficiary without changing vault status.
    pub fn partial_release(env: Env, vault_id: u64, amount: i128) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        let mut vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        if vault.balance < amount {
            return Err(ContractError::InsufficientBalance);
        }
        let xlm = token::Client::new(&env, &Self::load_token(&env));
        xlm.transfer(&env.current_contract_address(), &vault.beneficiary, &amount);
        vault.balance -= amount;
        Self::save_vault(&env, vault_id, &vault);
        env.events().publish(
            (symbol_short!("partial"), vault_id),
            (vault.beneficiary, amount),
        );
        Ok(())
    }

    // --- Task 3: set_beneficiaries ---

    /// Owner-only. Set multiple beneficiaries with BPS allocations summing to 10_000.
    pub fn set_beneficiaries(
        env: Env,
        vault_id: u64,
        beneficiaries: Vec<BeneficiaryEntry>,
    ) -> Result<(), ContractError> {
        let mut vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        let total_bps: u32 = beneficiaries.iter().map(|e| e.bps).sum();
        if total_bps != 10_000 {
            return Err(ContractError::InvalidBps);
        }
        vault.beneficiaries = beneficiaries;
        Self::save_vault(&env, vault_id, &vault);
        Ok(())
    }

    // --- Task 4: update_metadata ---

    /// Owner-only. Attach or update a short metadata string on the vault.
    pub fn update_metadata(env: Env, vault_id: u64, metadata: String) -> Result<(), ContractError> {
        let mut vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        vault.metadata = metadata;
        Self::save_vault(&env, vault_id, &vault);
        Ok(())
    }

    // --- views ---

    pub fn is_expired(env: Env, vault_id: u64) -> bool {
        let vault = Self::load_vault(&env, vault_id);
        let now = env.ledger().timestamp();
        now >= vault.last_check_in + vault.check_in_interval
    }

    pub fn get_vault(env: Env, vault_id: u64) -> Vault {
        Self::load_vault(&env, vault_id)
    }

    pub fn get_vaults_by_owner(env: Env, owner: Address) -> Vec<u64> {
        Self::load_owner_vault_ids(&env, &owner)
    }

    pub fn get_ttl_remaining(env: Env, vault_id: u64) -> Option<u64> {
        let vault: Vault = env.storage().persistent().get(&DataKey::Vault(vault_id))?;
        let deadline = vault.last_check_in + vault.check_in_interval;
        let now = env.ledger().timestamp();
        if now >= deadline { Some(0) } else { Some(deadline - now) }
    }

    pub fn get_release_status(env: Env, vault_id: u64) -> ReleaseStatus {
        Self::load_vault(&env, vault_id).status
    }

    pub fn vault_count(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::VaultCount).unwrap_or(0u64)
    }

    pub fn update_beneficiary(env: Env, vault_id: u64, new_beneficiary: Address) {
        let mut vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        if vault.status != ReleaseStatus::Locked {
            panic_with_error!(&env, ContractError::AlreadyReleased);
        }
        vault.beneficiary = new_beneficiary;
        Self::save_vault(&env, vault_id, &vault);
    }

    pub fn update_check_in_interval(
        env: Env,
        vault_id: u64,
        new_interval: u64,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        if new_interval == 0 {
            return Err(ContractError::InvalidInterval);
        }
        let mut vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        vault.check_in_interval = new_interval;
        Self::save_vault(&env, vault_id, &vault);
        Ok(())
    }

    pub fn cancel_vault(env: Env, vault_id: u64) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        let mut vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        let refund_amount = vault.balance;
        if refund_amount > 0 {
            let xlm = token::Client::new(&env, &Self::load_token(&env));
            xlm.transfer(&env.current_contract_address(), &vault.owner, &refund_amount);
        }
        vault.balance = 0;
        vault.status = ReleaseStatus::Cancelled;
        Self::save_vault(&env, vault_id, &vault);
        Ok(())
    }

    pub fn transfer_ownership(
        env: Env,
        vault_id: u64,
        new_owner: Address,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        let mut vault = Self::load_vault(&env, vault_id);
        let old_owner = vault.owner.clone();
        old_owner.require_auth();
        new_owner.require_auth();
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        if old_owner != new_owner {
            Self::remove_owner_vault_id(&env, &old_owner, vault_id);
            Self::add_owner_vault_id(&env, &new_owner, vault_id);
        }
        vault.owner = new_owner;
        Self::save_vault(&env, vault_id, &vault);
        Ok(())
    }

    // --- helpers ---

    fn assert_not_paused(env: &Env) {
        if Self::load_paused(env) {
            panic_with_error!(env, ContractError::Paused);
        }
    }

    fn load_paused(env: &Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    fn require_admin(env: &Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
    }

    fn load_token(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::TokenAddress).expect("not initialized")
    }

    fn load_vault(env: &Env, vault_id: u64) -> Vault {
        env.storage()
            .persistent()
            .get(&DataKey::Vault(vault_id))
            .unwrap_or_else(|| panic_with_error!(env, ContractError::VaultNotFound))
    }

    fn load_owner_vault_ids(env: &Env, owner: &Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerVaults(owner.clone()))
            .unwrap_or(Vec::new(env))
    }

    fn save_owner_vault_ids(env: &Env, owner: &Address, vault_ids: &Vec<u64>) {
        let key = DataKey::OwnerVaults(owner.clone());
        env.storage().persistent().set(&key, vault_ids);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
    }

    fn add_owner_vault_id(env: &Env, owner: &Address, vault_id: u64) {
        let mut vault_ids = Self::load_owner_vault_ids(env, owner);
        vault_ids.push_back(vault_id);
        Self::save_owner_vault_ids(env, owner, &vault_ids);
    }

    fn remove_owner_vault_id(env: &Env, owner: &Address, vault_id: u64) {
        let vault_ids = Self::load_owner_vault_ids(env, owner);
        let mut next_ids = Vec::new(env);
        for id in vault_ids.iter() {
            if id != vault_id {
                next_ids.push_back(id);
            }
        }
        Self::save_owner_vault_ids(env, owner, &next_ids);
    }

    fn save_vault(env: &Env, vault_id: u64, vault: &Vault) {
        let key = DataKey::Vault(vault_id);
        env.storage().persistent().set(&key, vault);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
    }
}
