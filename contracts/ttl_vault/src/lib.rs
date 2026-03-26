#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, token, Address, Env,
    Vec,
};

mod types;
use types::{DataKey, ReleaseEvent, ReleaseStatus, Vault, RELEASE_TOPIC, VAULT_CREATED_TOPIC};

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
}

#[contract]
pub struct TtlVaultContract;

#[contractimpl]
impl TtlVaultContract {
    // --- admin/config ---

    /// One-time initializer for token/admin configuration.
    pub fn initialize(env: Env, xlm_token: Address, admin: Address) {
        if env.storage().instance().has(&DataKey::TokenAddress)
            || env.storage().instance().has(&DataKey::Admin)
        {
            panic_with_error!(&env, ContractError::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::TokenAddress, &xlm_token);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
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

    /// Create a new vault. Returns the vault ID.
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
        };

        Self::save_vault(&env, vault_id, &vault);
        Self::add_owner_vault_id(&env, &owner, vault_id);
        env.storage()
            .instance()
            .set(&DataKey::VaultCount, &vault_id);
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish(
            (VAULT_CREATED_TOPIC,),
            (vault_id, owner, beneficiary, check_in_interval),
        );

        vault_id
    }

    /// Owner checks in, resetting the TTL countdown.
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

        env.events()
            .publish((symbol_short!("check_in"), vault_id), vault.last_check_in);

        Ok(())
    }

    /// Deposit configured token into the vault.
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

    /// Owner withdraws from the vault.
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

    /// Anyone can call this once the TTL has lapsed to release funds to beneficiary.
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

        let released_amount = vault.balance;
        let xlm = token::Client::new(&env, &Self::load_token(&env));
        xlm.transfer(
            &env.current_contract_address(),
            &vault.beneficiary,
            &released_amount,
        );

        vault.balance = 0;
        vault.status = ReleaseStatus::Released;
        Self::save_vault(&env, vault_id, &vault);

        env.events().publish(
            (RELEASE_TOPIC,),
            ReleaseEvent {
                vault_id,
                beneficiary: vault.beneficiary,
                amount: released_amount,
            },
        );
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

        if now >= deadline {
            Some(0)
        } else {
            Some(deadline - now)
        }
    }

    pub fn get_release_status(env: Env, vault_id: u64) -> ReleaseStatus {
        let vault = Self::load_vault(&env, vault_id);
        vault.status
    }

    pub fn vault_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::VaultCount)
            .unwrap_or(0u64)
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
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
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
        env.storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .expect("not initialized")
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
        env.storage()
            .persistent()
            .extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
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

    /// Persist a vault and extend its TTL so it is never silently archived.
    fn save_vault(env: &Env, vault_id: u64, vault: &Vault) {
        let key = DataKey::Vault(vault_id);
        env.storage().persistent().set(&key, vault);
        env.storage()
            .persistent()
            .extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
    }
}
