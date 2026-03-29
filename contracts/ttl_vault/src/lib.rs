#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, token, Address,
    BytesN, Env, String, Vec,
};

mod types;
use types::{
    BeneficiaryEntry, DataKey, ReleaseEvent, ReleaseStatus, Vault, EXPIRY_WARNING_THRESHOLD,
    CANCEL_TOPIC, CHECK_IN_TOPIC, DEPOSIT_TOPIC, OWNERSHIP_TOPIC, PING_EXPIRY_TOPIC,
    RELEASE_TOPIC, VAULT_CREATED_TOPIC, WITHDRAW_TOPIC,
};

#[cfg(test)]
mod test;

pub const VAULT_TTL_THRESHOLD: u32 = 1000;
pub const VAULT_TTL_LEDGERS: u32 = 200_000;
pub const INSTANCE_TTL_THRESHOLD: u32 = 1000;
pub const INSTANCE_TTL_LEDGERS: u32 = 200_000;

/// Approximate ledger close time in seconds (Stellar mainnet ~5s).
const LEDGER_SECOND: u32 = 5;
/// Soroban maximum persistent entry TTL in ledgers (~180 days at 5s/ledger).
const MAX_PERSISTENT_TTL: u32 = 3_110_400;

/// Compute a persistent storage TTL (in ledgers) for a vault with the given
/// check-in interval. Applies a 2× safety buffer so storage outlives the
/// interval, capped at the Soroban maximum.
fn vault_ttl_ledgers(check_in_interval: u64) -> u32 {
    let ledgers = (check_in_interval as u32)
        .saturating_mul(2)
        .saturating_div(LEDGER_SECOND);
    ledgers.max(VAULT_TTL_LEDGERS).min(MAX_PERSISTENT_TTL)
}

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
    NoPendingAdmin = 11,
    InvalidBps = 12,
    NotExpiringSoon = 13,
    IntervalTooLow = 14,
    IntervalTooHigh = 15,
    NotExpired = 16,
    InvalidBeneficiary = 17,
    BalanceOverflow = 18,
    VaultExpired = 19,
}

#[contract]
pub struct TtlVaultContract;

#[contractimpl]
impl TtlVaultContract {
    // --- admin/config ---

    /// Initializes the contract with the XLM token address and admin address.
    ///
    /// This function must be called once before any other contract operations.
    /// It sets up the initial configuration and stores the admin address.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `xlm_token` - The address of the XLM token contract
    /// * `admin` - The address of the contract administrator
    ///
    /// # Panics
    /// Panics if the contract has already been initialized
    pub fn initialize(env: Env, xlm_token: Address, admin: Address) {
        if env.storage().instance().has(&DataKey::TokenAddress)
            || env.storage().instance().has(&DataKey::Admin)
        {
            panic_with_error!(&env, ContractError::AlreadyInitialized);
        }
        if xlm_token == admin {
            panic_with_error!(&env, ContractError::InvalidAdmin);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::TokenAddress, &xlm_token);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    /// Pauses the contract, blocking all state-changing operations.
    ///
    /// Only the admin can call this function. When paused, operations like
    /// deposit, withdraw, check_in, and trigger_release will fail.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Panics
    /// Panics if the caller is not the admin
    pub fn pause(env: Env) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Paused, &true);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    /// Unpauses the contract, allowing all operations to resume.
    ///
    /// Only the admin can call this function.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Panics
    /// Panics if the caller is not the admin
    pub fn unpause(env: Env) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    /// Sets the minimum allowed check-in interval for vaults.
    ///
    /// This constraint applies to both new vaults and interval updates.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `min_interval` - Minimum interval in seconds (must be > 0)
    ///
    /// # Panics
    /// * Panics if the caller is not the admin
    /// * Panics if `min_interval` is 0
    pub fn set_min_check_in_interval(env: Env, min_interval: u64) {
        Self::require_admin(&env);
        if min_interval == 0 {
            panic_with_error!(&env, ContractError::InvalidInterval);
        }
        env.storage().instance().set(&DataKey::MinCheckInInterval, &min_interval);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    /// Sets the maximum allowed check-in interval for vaults.
    ///
    /// This constraint applies to both new vaults and interval updates.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `max_interval` - Maximum interval in seconds (must be > 0)
    ///
    /// # Panics
    /// * Panics if the caller is not the admin
    /// * Panics if `max_interval` is 0
    pub fn set_max_check_in_interval(env: Env, max_interval: u64) {
        Self::require_admin(&env);
        if max_interval == 0 {
            panic_with_error!(&env, ContractError::InvalidInterval);
        }
        env.storage().instance().set(&DataKey::MaxCheckInInterval, &max_interval);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    /// Returns the minimum check-in interval if set.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// `Some(seconds)` with the minimum interval, or `None` if not set
    pub fn get_min_check_in_interval(env: Env) -> Option<u64> {
        env.storage().instance().get(&DataKey::MinCheckInInterval)
    }

    /// Returns the maximum check-in interval if set.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// `Some(seconds)` with the maximum interval, or `None` if not set
    pub fn get_max_check_in_interval(env: Env) -> Option<u64> {
        env.storage().instance().get(&DataKey::MaxCheckInInterval)
    }

    /// Admin-only. Upgrades the contract to a new WASM hash.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        Self::require_admin(&env);
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    /// Returns whether the contract is currently paused.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// `true` if the contract is paused, `false` otherwise
    pub fn is_paused(env: Env) -> bool {
        Self::load_paused(&env)
    }

    /// Returns the current admin address.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// The admin address
    ///
    /// # Panics
    /// Panics if the contract is not initialized
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::VaultNotFound))
    }

    /// Proposes a new admin. The proposed admin must call `accept_admin` to complete the transfer.
    pub fn propose_admin(env: Env, new_admin: Address) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::PendingAdmin, &new_admin);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    /// Returns the pending admin address, if any.
    pub fn get_pending_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::PendingAdmin)
    }

    /// Accepts a pending admin transfer. Must be called by the pending admin.
    pub fn accept_admin(env: Env) {
        let pending: Address = env
            .storage()
            .instance()
            .get(&DataKey::PendingAdmin)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::NoPendingAdmin));
        pending.require_auth();
        env.storage().instance().set(&DataKey::Admin, &pending);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    // --- vault lifecycle ---

    /// Creates a new time-locked vault.
    ///
    /// The vault starts with a zero balance and must be funded via `deposit`
    /// or `batch_deposit` before it can hold assets.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `owner` - The address of the vault owner (must authorize)
    /// * `beneficiary` - The address that will receive funds when the vault expires
    /// * `check_in_interval` - Time interval in seconds between required check-ins
    ///
    /// # Returns
    /// The unique vault ID
    ///
    /// # Panics
    /// * Panics if `check_in_interval` is 0
    /// * Panics if `check_in_interval` is outside the configured min/max bounds
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

        Self::assert_interval_in_bounds(&env, check_in_interval);

        if owner == beneficiary {
            panic_with_error!(&env, ContractError::InvalidBeneficiary);
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
        Self::add_beneficiary_vault_id(&env, &beneficiary, vault_id);
        // VaultCount is updated only after all vault data is written. If any
        // prior storage call panics, the count is not advanced, keeping it
        // consistent with the number of successfully persisted vaults.
        env.storage().instance().set(&DataKey::VaultCount, &vault_id);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (VAULT_CREATED_TOPIC,),
            (vault_id, owner, beneficiary, check_in_interval),
        );
        vault_id
    }

    /// Records a check-in to reset the vault's expiry timer.
    ///
    /// The caller must be the vault owner. This function resets the `last_check_in`
    /// timestamp, extending the vault's TTL.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `caller` - The address of the caller (must be the vault owner)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
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
        env.events().publish((CHECK_IN_TOPIC, vault_id), vault.last_check_in);
        Ok(())
    }

    /// Deposits funds into a vault.
    ///
    /// Transfers tokens from the caller to the contract and increases the vault's balance.
    /// The vault must be in Locked status.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `from` - The address depositing funds (must authorize)
    /// * `amount` - Amount to deposit in stroops (1 XLM = 10,000,000 stroops)
    ///
    /// # Panics
    /// * Panics if the contract is paused
    /// * Panics if `amount` is not positive
    /// * Panics if the vault is not in Locked status
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

        let now = env.ledger().timestamp();
        if now >= vault.last_check_in + vault.check_in_interval {
            panic_with_error!(&env, ContractError::VaultExpired);
        }

        let xlm = token::Client::new(&env, &Self::load_token(&env));
        xlm.transfer(&from, &env.current_contract_address(), &amount);
        vault.balance = vault.balance
            .checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::BalanceOverflow));
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (DEPOSIT_TOPIC, vault_id),
            (amount, vault.balance),
        );
    }

    /// Deposits funds into multiple vaults in a single transfer.
    ///
    /// This is more efficient than calling `deposit` multiple times as it only
    /// requires one token transfer. All vaults must be in Locked status.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `from` - The address depositing funds (must authorize)
    /// * `deposit` - Vector of (vault_id, amount) pairs where amount is in stroops (1 XLM = 10,000,000 stroops)
    ///
    /// # Panics
    /// * Panics if the contract is paused
    /// * Panics if any amount is not positive
    /// * Panics if any vault is not in Locked status
    /// * Panics if the total amount overflows
    pub fn batch_deposit(env: Env, from: Address, deposits: Vec<(u64, i128)>) {
        Self::assert_not_paused(&env);
        from.require_auth();

        let mut validated = Vec::new(&env);
        let mut total_amount = 0i128;

        for deposit in deposits.iter() {
            let (vault_id, amount) = deposit;
            if amount <= 0 {
                panic_with_error!(&env, ContractError::InvalidAmount);
            }

            let vault = Self::load_vault(&env, vault_id);
            if vault.status != ReleaseStatus::Locked {
                panic_with_error!(&env, ContractError::AlreadyReleased);
            }

            total_amount = total_amount
                .checked_add(amount)
                .unwrap_or_else(|| panic_with_error!(&env, ContractError::InvalidAmount));
            validated.push_back((vault_id, vault, amount));
        }

        if total_amount == 0 {
            return;
        }

        let xlm = token::Client::new(&env, &Self::load_token(&env));
        xlm.transfer(&from, &env.current_contract_address(), &total_amount);

        for validated_deposit in validated.iter() {
            let (vault_id, mut vault, amount) = validated_deposit;
            vault.balance = vault.balance
                .checked_add(amount)
                .unwrap_or_else(|| panic_with_error!(&env, ContractError::BalanceOverflow));
            Self::save_vault(&env, vault_id, &vault);
        }
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    /// Owner withdraws from the vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `amount` - Amount to withdraw in stroops (1 XLM = 10,000,000 stroops)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::InvalidAmount` - If amount is not positive
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    /// * `ContractError::InsufficientBalance` - If vault balance is less than amount
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
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (WITHDRAW_TOPIC, vault_id),
            (amount, vault.balance),
        );
        Ok(())
    }

    /// Triggers the release of funds to beneficiaries after the vault expires.
    ///
    /// Anyone can call this function once the vault's TTL has lapsed. The funds
    /// are distributed to the primary beneficiary or split among multiple beneficiaries
    /// based on their BPS allocations.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Panics
    /// * Panics if the contract is paused
    /// * Panics if the vault is not in Locked status
    /// * Panics if the vault has not expired yet
    /// * Panics if the vault balance is zero
    pub fn trigger_release(env: Env, vault_id: u64) {
        Self::assert_not_paused(&env);
        let mut vault = Self::load_vault(&env, vault_id);
        if vault.status != ReleaseStatus::Locked {
            panic_with_error!(&env, ContractError::AlreadyReleased);
        }
        if !Self::is_expired(env.clone(), vault_id) {
            panic_with_error!(&env, ContractError::NotExpired);
        }
        let total = vault.balance;
        let xlm = token::Client::new(&env, &Self::load_token(&env));

        if vault.beneficiaries.is_empty() {
            if total > 0 {
                xlm.transfer(&env.current_contract_address(), &vault.beneficiary, &total);
            }
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
                if share > 0 {
                    xlm.transfer(&env.current_contract_address(), &entry.address, &share);
                }
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
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    // --- Task 1: ping_expiry ---

    /// Checks the remaining TTL and emits a warning event if near expiry.
    ///
    /// This function can be called by anyone to monitor vault expiry status.
    /// If the remaining TTL is less than `EXPIRY_WARNING_THRESHOLD` (24 hours),
    /// a warning event is emitted.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// The remaining TTL in seconds (0 if expired)
    pub fn ping_expiry(env: Env, vault_id: u64) -> u64 {
        if Self::try_load_vault(&env, vault_id).is_none() {
            panic_with_error!(&env, ContractError::VaultNotFound);
        }
        let ttl = Self::get_ttl_remaining(env.clone(), vault_id).unwrap_or(0);
        if ttl < EXPIRY_WARNING_THRESHOLD {
            env.events().publish((PING_EXPIRY_TOPIC, vault_id), ttl);
        }
        ttl
    }

    // --- Task 2: partial_release ---

    /// Transfers a partial amount to the beneficiary (or beneficiaries) without releasing the vault.
    ///
    /// This allows the owner to distribute funds gradually while keeping the vault
    /// in Locked status. The vault can still be checked in and released later.
    ///
    /// When a multi-beneficiary split has been configured via `set_beneficiaries`, the
    /// `amount` is distributed proportionally according to each entry's BPS allocation,
    /// using the same rounding logic as `trigger_release` (last entry absorbs dust).
    /// When no split is configured, the full `amount` goes to the primary beneficiary.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `amount` - Amount to transfer in stroops (1 XLM = 10,000,000 stroops)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::InvalidAmount` - If amount is not positive
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    /// * `ContractError::InsufficientBalance` - If vault balance is less than amount
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

        if vault.beneficiaries.is_empty() {
            // Single-beneficiary path: send full amount to primary beneficiary.
            xlm.transfer(&env.current_contract_address(), &vault.beneficiary, &amount);
            env.events().publish(
                (symbol_short!("partial"), vault_id),
                (vault.beneficiary.clone(), amount),
            );
        } else {
            // Multi-beneficiary path: split `amount` by BPS, last entry absorbs dust.
            let mut distributed: i128 = 0;
            let last_idx = vault.beneficiaries.len() - 1;
            for (i, entry) in vault.beneficiaries.iter().enumerate() {
                let share = if i as u32 == last_idx {
                    amount - distributed
                } else {
                    amount * (entry.bps as i128) / 10_000
                };
                if share > 0 {
                    xlm.transfer(&env.current_contract_address(), &entry.address, &share);
                }
                distributed += share;
                env.events().publish(
                    (symbol_short!("partial"), vault_id),
                    (entry.address.clone(), share),
                );
            }
        }

        vault.balance -= amount;
        Self::save_vault(&env, vault_id, &vault);
        Ok(())
    }

    // --- Task 3: set_beneficiaries ---

    /// Sets multiple beneficiaries with basis point (BPS) allocations.
    ///
    /// The sum of all BPS values must equal 10,000 (100%). When the vault is
    /// released, funds are split according to these allocations.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `beneficiaries` - Vector of BeneficiaryEntry structs with addresses and BPS values
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    /// * `ContractError::InvalidBps` - If BPS sum is not 10,000
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
        for entry in beneficiaries.iter() {
            if entry.address == vault.owner {
                return Err(ContractError::InvalidBeneficiary);
            }
        }
        vault.beneficiaries = beneficiaries;
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(())
    }

    // --- Task 4: update_metadata ---

    /// Updates the metadata string associated with a vault.
    ///
    /// This can be used to store a label, IPFS hash, or other reference data.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `metadata` - The metadata string to attach
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn update_metadata(env: Env, vault_id: u64, metadata: String) -> Result<(), ContractError> {
        let mut vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        vault.metadata = metadata;
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(())
    }

    // --- views ---

    /// Checks if a vault has expired based on the check-in interval.
    ///
    /// A vault is considered expired when the current timestamp is greater than
    /// or equal to the deadline (last_check_in + check_in_interval).
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// `true` if the vault has expired, `false` otherwise
    ///
    /// # Panics
    /// Panics if the vault does not exist
    pub fn is_expired(env: Env, vault_id: u64) -> bool {
        let vault = Self::load_vault(&env, vault_id);
        let now = env.ledger().timestamp();
        now >= vault.last_check_in + vault.check_in_interval
    }

    /// Retrieves a vault by its unique identifier.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// The `Vault` struct containing all vault data
    ///
    /// # Panics
    /// Panics if the vault does not exist (use `vault_exists` to check first)
    pub fn get_vault(env: Env, vault_id: u64) -> Vault {
        Self::load_vault(&env, vault_id)
    }

    /// Checks if a vault exists.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// `true` if the vault exists, `false` otherwise
    pub fn vault_exists(env: Env, vault_id: u64) -> bool {
        Self::try_load_vault(&env, vault_id).is_some()
    }

    /// Returns a paginated slice of vault IDs owned by a specific address.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `owner` - The owner address
    /// * `page` - Zero-based page index
    /// * `page_size` - Number of items per page
    ///
    /// # Returns
    /// A vector of vault IDs for the requested page
    pub fn get_vaults_by_owner(env: Env, owner: Address, page: u32, page_size: u32) -> Vec<u64> {
        let all = Self::load_owner_vault_ids(&env, &owner);
        Self::paginate(&env, all, page, page_size)
    }

    /// Returns a paginated slice of vault IDs where a specific address is the beneficiary.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `beneficiary` - The beneficiary address
    /// * `page` - Zero-based page index
    /// * `page_size` - Number of items per page
    ///
    /// # Returns
    /// A vector of vault IDs for the requested page
    pub fn get_vaults_by_beneficiary(env: Env, beneficiary: Address, page: u32, page_size: u32) -> Vec<u64> {
        let all = Self::load_beneficiary_vault_ids(&env, &beneficiary);
        Self::paginate(&env, all, page, page_size)
    }

    /// Returns the remaining time-to-live (TTL) for a vault in seconds.
    ///
    /// The TTL is calculated as the time remaining until the vault expires
    /// based on the last check-in time and the check-in interval.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// `Some(seconds)` with the remaining time in seconds if the vault exists and has not expired,
    /// `None` if the vault does not exist or the TTL has already lapsed.
    pub fn get_ttl_remaining(env: Env, vault_id: u64) -> Option<u64> {
        let vault = Self::try_load_vault(&env, vault_id)?;
        let deadline = vault.last_check_in + vault.check_in_interval;
        let now = env.ledger().timestamp();
        if now >= deadline { None } else { Some(deadline - now) }
    }

    /// Returns the current release status of a vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// The `ReleaseStatus` enum value (Locked, Released, or Cancelled)
    ///
    /// # Panics
    /// Panics if the vault does not exist
    pub fn get_release_status(env: Env, vault_id: u64) -> ReleaseStatus {
        Self::load_vault(&env, vault_id).status
    }

    /// Returns the total number of vaults created.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// The total vault count
    pub fn vault_count(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::VaultCount).unwrap_or(0u64)
    }

    /// Returns the address of the XLM token used by this contract.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// The token contract address
    pub fn get_contract_token(env: Env) -> Address {
        Self::load_token(&env)
    }

    /// Updates the primary beneficiary of a vault.
    ///
    /// This function allows the vault owner to change the beneficiary who will
    /// receive the funds when the vault expires. The vault must still be in
    /// Locked status (not yet released).
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `new_beneficiary` - The new beneficiary address
    ///
    /// # Panics
    /// * Panics if the caller is not the vault owner
    /// * Panics if the vault is not in Locked status (already released or cancelled)
    pub fn update_beneficiary(env: Env, vault_id: u64, new_beneficiary: Address) {
        let mut vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        if vault.status != ReleaseStatus::Locked {
            panic_with_error!(&env, ContractError::AlreadyReleased);
        }

        if vault.owner == new_beneficiary {
            panic_with_error!(&env, ContractError::InvalidBeneficiary);
        }

        vault.beneficiary = new_beneficiary;
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    /// Updates the check-in interval for a vault.
    ///
    /// The new interval must be within the configured min/max bounds.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `new_interval` - New interval in seconds (must be > 0)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::InvalidInterval` - If new_interval is 0
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    /// * `ContractError::IntervalTooLow` - If new_interval is below minimum
    /// * `ContractError::IntervalTooHigh` - If new_interval exceeds maximum
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
        Self::assert_interval_in_bounds(&env, new_interval);
        let mut vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        vault.check_in_interval = new_interval;
        Self::save_vault(&env, vault_id, &vault);
        // Explicitly re-extend the vault's persistent TTL using the new (potentially
        // longer) interval so storage outlives the updated check-in deadline.
        let new_ttl = vault_ttl_ledgers(new_interval);
        env.storage().persistent().extend_ttl(
            &DataKey::Vault(vault_id),
            VAULT_TTL_THRESHOLD,
            new_ttl,
        );
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(())
    }

    /// Cancels a vault and refunds the balance to the owner.
    ///
    /// This permanently marks the vault as Cancelled and transfers any
    /// remaining balance back to the owner.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
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
        env.events().publish((CANCEL_TOPIC, vault_id), (vault.owner, refund_amount));
        Ok(())
    }

    /// Transfers ownership of a vault to a new address.
    ///
    /// Both the current owner and new owner must authorize this operation.
    /// The vault must still be in Locked status.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `new_owner` - The address of the new owner (must authorize)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
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
        // Invariant: owner and beneficiary must always be distinct addresses.
        // BeneficiaryVaults index does not need updating here because the vault's
        // beneficiary field is not changed by an ownership transfer.
        if new_owner == vault.beneficiary {
            return Err(ContractError::InvalidBeneficiary);
        }
        if old_owner != new_owner {
            Self::remove_owner_vault_id(&env, &old_owner, vault_id);
            Self::add_owner_vault_id(&env, &new_owner, vault_id);
        }
        vault.owner = new_owner.clone();
        Self::save_vault(&env, vault_id, &vault);
        env.events().publish((OWNERSHIP_TOPIC, vault_id), (old_owner, new_owner));
        Ok(())
    }

    // --- helpers ---

    fn paginate(env: &Env, all: Vec<u64>, page: u32, page_size: u32) -> Vec<u64> {
        if page_size == 0 {
            return Vec::new(env);
        }
        let start = (page as u64).saturating_mul(page_size as u64);
        let len = all.len() as u64;
        let mut result = Vec::new(env);
        let mut i = start;
        while i < len && i < start + page_size as u64 {
            result.push_back(all.get(i as u32).unwrap());
            i += 1;
        }
        result
    }

    fn assert_not_paused(env: &Env) {
        if Self::load_paused(env) {
            panic_with_error!(env, ContractError::Paused);
        }
    }

    fn load_paused(env: &Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    fn require_admin(env: &Env) {
        let admin = Self::load_admin(env);
        admin.require_auth();
    }

    fn load_admin(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, ContractError::VaultNotFound))
    }

    fn load_token(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::TokenAddress).unwrap_or_else(|| panic_with_error!(env, ContractError::VaultNotFound))
    }

    fn load_vault(env: &Env, vault_id: u64) -> Vault {
        env.storage()
            .persistent()
            .get(&DataKey::Vault(vault_id))
            .unwrap_or_else(|| panic_with_error!(env, ContractError::VaultNotFound))
    }

    /// Tries to load a vault, returning None if it doesn't exist.
    ///
    /// This is a safe alternative to `load_vault` for use in view functions
    /// that should not panic when a vault is not found.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// `Some(Vault)` if the vault exists, `None` otherwise
    fn try_load_vault(env: &Env, vault_id: u64) -> Option<Vault> {
        env.storage()
            .persistent()
            .get(&DataKey::Vault(vault_id))
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
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, vault);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
    }

    fn load_beneficiary_vault_ids(env: &Env, beneficiary: &Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::BeneficiaryVaults(beneficiary.clone()))
            .unwrap_or(Vec::new(env))
    }

    fn save_beneficiary_vault_ids(env: &Env, beneficiary: &Address, vault_ids: &Vec<u64>) {
        let key = DataKey::BeneficiaryVaults(beneficiary.clone());
        env.storage().persistent().set(&key, vault_ids);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
    }

    fn add_beneficiary_vault_id(env: &Env, beneficiary: &Address, vault_id: u64) {
        let mut vault_ids = Self::load_beneficiary_vault_ids(env, beneficiary);
        vault_ids.push_back(vault_id);
        Self::save_beneficiary_vault_ids(env, beneficiary, &vault_ids);
    }

    fn assert_interval_in_bounds(env: &Env, interval: u64) {
        if let Some(min) = env.storage().instance().get::<DataKey, u64>(&DataKey::MinCheckInInterval) {
            if interval < min {
                panic_with_error!(env, ContractError::IntervalTooLow);
            }
        }
        if let Some(max) = env.storage().instance().get::<DataKey, u64>(&DataKey::MaxCheckInInterval) {
            if interval > max {
                panic_with_error!(env, ContractError::IntervalTooHigh);
            }
        }
    }
}
