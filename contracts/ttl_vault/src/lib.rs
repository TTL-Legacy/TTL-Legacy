#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, token, Address,
    BytesN, Bytes, Env, String, Vec,
};

mod types;
use types::{
    BeneficiaryEntry, DataKey, ReleaseEvent, ReleaseStatus, ReleaseCondition, Vault, VestingSchedule,
    PasskeyHash, BackupCode, DisputeStatus, WithdrawalScheduleEntry, ConditionalAcceptanceEntry,
    EXPIRY_WARNING_THRESHOLD, BENEFICIARY_UPDATED_TOPIC, CANCEL_TOPIC, CHECK_IN_TOPIC,
    CLAIM_VEST_TOPIC, DEPOSIT_TOPIC, OWNERSHIP_TOPIC, PAUSE_TOPIC, PING_EXPIRY_TOPIC,
    RELEASE_TOPIC, SET_BENEFICIARIES_TOPIC, SET_MAX_INTERVAL_TOPIC, SET_MIN_INTERVAL_TOPIC,
    SET_VESTING_TOPIC, UNPAUSE_TOPIC, UPDATE_INTERVAL_TOPIC, UPDATE_METADATA_TOPIC,
    VAULT_CREATED_TOPIC, WITHDRAW_TOPIC, MAX_METADATA_LEN, MAX_NAME_LEN, MAX_DESCRIPTION_LEN,
    MAX_NOTES_LEN, MAX_CUSTOM_METADATA_LEN, PAUSE_VAULT_TOPIC, RESUME_VAULT_TOPIC, SET_METADATA_TOPIC,
    INHERITANCE_TOPIC, ADD_PASSKEY_TOPIC, REMOVE_PASSKEY_TOPIC, ROTATE_PASSKEY_TOPIC,
    BACKUP_CODE_USED_TOPIC, BACKUP_CODES_GENERATED_TOPIC, DELEGATE_BENEFICIARY_TOPIC,
    DISPUTE_FILED_TOPIC, DISPUTE_RESOLVED_TOPIC, WITHDRAWAL_SCHEDULED_TOPIC, WITHDRAWAL_EXECUTED_TOPIC,
    CONDITIONS_ACCEPTED_TOPIC,
};

#[cfg(test)]
mod test;

#[cfg(test)]
mod regression_tests;

/// Minimum TTL (in ledgers) before a persistent entry is eligible for extension.
/// At ~5 s/ledger this is ~83 minutes.
pub const VAULT_TTL_THRESHOLD: u32 = 1000;

/// Default persistent storage TTL for vault entries, in ledgers.
/// 200_000 ledgers × 5 s/ledger ≈ 11.6 days.
/// Used as the floor in `vault_ttl_ledgers`; long-interval vaults get a larger value.
pub const VAULT_TTL_LEDGERS: u32 = 200_000;

/// Minimum TTL (in ledgers) before instance storage is eligible for extension.
/// At ~5 s/ledger this is ~83 minutes.
pub const INSTANCE_TTL_THRESHOLD: u32 = 1000;

/// TTL for instance storage entries, in ledgers.
/// 200_000 ledgers × 5 s/ledger ≈ 11.6 days.
/// Extended on every state-mutating call to keep contract instance alive.
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
    ledgers.clamp(VAULT_TTL_LEDGERS, MAX_PERSISTENT_TTL)
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
    InvalidAdmin = 20,
    NotInitialized = 21,
    VestingNotFound = 22,
    NothingToClaimYet = 23,
    VestingAlreadyComplete = 24,
    MaxTtlExceeded = 25,
    InvalidPasskey = 26,
    PasskeyNotFound = 27,
    InvalidBackupCode = 28,
    BackupCodeAlreadyUsed = 29,
    NotBeneficiary = 30,
    DisputeFiled = 31,
    NoScheduledWithdrawals = 32,
    ConditionsNotApproved = 33,
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
        env.storage().instance().set(&DataKey::Version, &String::from_str(&env, "1.0.0"));
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
        env.events().publish((PAUSE_TOPIC,), true);
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
        env.events().publish((UNPAUSE_TOPIC,), false);
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
        if let Some(max) = env.storage().instance().get::<DataKey, u64>(&DataKey::MaxCheckInInterval) {
            if min_interval > max {
                panic_with_error!(&env, ContractError::InvalidInterval);
            }
        }
        env.storage().instance().set(&DataKey::MinCheckInInterval, &min_interval);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((SET_MIN_INTERVAL_TOPIC,), min_interval);
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
        if let Some(min) = env.storage().instance().get::<DataKey, u64>(&DataKey::MinCheckInInterval) {
            if max_interval < min {
                panic_with_error!(&env, ContractError::InvalidInterval);
            }
        }
        env.storage().instance().set(&DataKey::MaxCheckInInterval, &max_interval);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((SET_MAX_INTERVAL_TOPIC,), max_interval);
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

    /// Sets the maximum TTL (time-to-live) for vaults in seconds.
    ///
    /// This prevents vaults from becoming permanent by capping the maximum
    /// TTL that can be set during check-in. Default is 10 years (315,360,000 seconds).
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `max_ttl` - Maximum TTL in seconds (must be > 0)
    ///
    /// # Panics
    /// * Panics if the caller is not the admin
    /// * Panics if `max_ttl` is 0
    pub fn set_max_ttl_seconds(env: Env, max_ttl: u64) {
        Self::require_admin(&env);
        if max_ttl == 0 {
            panic_with_error!(&env, ContractError::InvalidInterval);
        }
        env.storage().instance().set(&DataKey::MaxTtlSeconds, &max_ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((SET_MAX_TTL_TOPIC,), max_ttl);
    }

    /// Returns the maximum TTL in seconds.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// The maximum TTL in seconds, or 10 years if not set
    pub fn get_max_ttl_seconds(env: Env) -> u64 {
        // Default: 10 years in seconds
        env.storage().instance().get(&DataKey::MaxTtlSeconds).unwrap_or(315_360_000)
    }

    /// Sets the TTL decay rate as a percentage per month.
    ///
    /// If check-ins become infrequent (no check-in for 30 days), the TTL is reduced
    /// by this rate. For example, 100 = 1% per month.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `decay_rate` - Decay rate in basis points (1-10000, where 100 = 1%)
    ///
    /// # Panics
    /// * Panics if the caller is not the admin
    /// * Panics if `decay_rate` is 0 or > 10000
    pub fn set_ttl_decay_rate(env: Env, decay_rate: u32) {
        Self::require_admin(&env);
        if decay_rate == 0 || decay_rate > 10_000 {
            panic_with_error!(&env, ContractError::InvalidBps);
        }
        env.storage().instance().set(&DataKey::TtlDecayRate, &decay_rate);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((SET_DECAY_RATE_TOPIC,), decay_rate);
    }

    /// Returns the TTL decay rate in basis points.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// The decay rate in basis points (0 if not set, meaning no decay)
    pub fn get_ttl_decay_rate(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::TtlDecayRate).unwrap_or(0)
    }

    /// Adds a token to the whitelist, allowing it to be used in vaults.
    ///
    /// # Arguments
    /// * `token_address` - The token contract address to whitelist
    ///
    /// # Panics
    /// * Panics if the caller is not the admin
    pub fn whitelist_token(env: Env, token_address: Address) {
        Self::require_admin(&env);
        let key = DataKey::TokenWhitelist(token_address.clone());
        env.storage().persistent().set(&key, &true);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    /// Removes a token from the whitelist.
    ///
    /// # Arguments
    /// * `token_address` - The token contract address to remove
    ///
    /// # Panics
    /// * Panics if the caller is not the admin
    pub fn remove_token_whitelist(env: Env, token_address: Address) {
        Self::require_admin(&env);
        let key = DataKey::TokenWhitelist(token_address);
        env.storage().persistent().remove(&key);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    /// Checks if a token is whitelisted.
    ///
    /// # Arguments
    /// * `token_address` - The token contract address to check
    ///
    /// # Returns
    /// `true` if the token is whitelisted or is the default XLM token, `false` otherwise
    pub fn is_token_whitelisted(env: Env, token_address: Address) -> bool {
        // Default XLM token is always whitelisted
        let default_token = Self::load_token(&env);
        if token_address == default_token {
            return true;
        }
        
        let key = DataKey::TokenWhitelist(token_address);
        env.storage().persistent().get(&key).unwrap_or(false)
    }

    // --- Cross-Chain Bridge Support (Issue #366) ---

    /// Registers a bridge for cross-chain support.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `chain_id` - The target chain ID
    /// * `bridge_address` - The bridge contract address
    ///
    /// # Panics
    /// * Panics if the caller is not the admin
    pub fn register_bridge(env: Env, chain_id: u32, bridge_address: Address) {
        Self::require_admin(&env);
        let config = BridgeConfig {
            chain_id,
            bridge_address: bridge_address.clone(),
            is_active: true,
        };
        let key = DataKey::BridgeConfig(chain_id);
        env.storage().persistent().set(&key, &config);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((symbol_short!("br_reg"),), (chain_id, bridge_address));
    }

    /// Deactivates a bridge.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `chain_id` - The chain ID to deactivate
    ///
    /// # Panics
    /// * Panics if the caller is not the admin
    pub fn deactivate_bridge(env: Env, chain_id: u32) {
        Self::require_admin(&env);
        let key = DataKey::BridgeConfig(chain_id);
        if let Some(mut config) = env.storage().persistent().get::<DataKey, BridgeConfig>(&key) {
            config.is_active = false;
            env.storage().persistent().set(&key, &config);
            env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        }
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((symbol_short!("br_deact"),), chain_id);
    }

    /// Gets the bridge configuration for a chain.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `chain_id` - The chain ID
    ///
    /// # Returns
    /// The bridge configuration if it exists
    pub fn get_bridge_config(env: Env, chain_id: u32) -> Option<BridgeConfig> {
        let key = DataKey::BridgeConfig(chain_id);
        env.storage().persistent().get(&key)
    }

    /// Checks if a bridge is active.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `chain_id` - The chain ID
    ///
    /// # Returns
    /// `true` if the bridge is active, `false` otherwise
    pub fn is_bridge_active(env: Env, chain_id: u32) -> bool {
        if let Some(config) = Self::get_bridge_config(env, chain_id) {
            config.is_active
        } else {
            false
        }
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
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::NotInitialized))
    }

    /// Returns the contract version string set during initialization.
    pub fn get_version(env: Env) -> String {
        env.storage()
            .instance()
            .get(&DataKey::Version)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::NotInitialized))
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
            token_address: Option<Address>,
        ) -> u64 {
            owner.require_auth();
            Self::require_initialized(&env);
            if check_in_interval == 0 {
                panic_with_error!(&env, ContractError::InvalidInterval);
            }

            Self::assert_interval_in_bounds(&env, check_in_interval);

            if owner == beneficiary {
                panic_with_error!(&env, ContractError::InvalidBeneficiary);
            }

            // Use provided token or default to contract's XLM token
            let vault_token = match token_address {
                Some(addr) => {
                    // Validate token is whitelisted
                    Self::assert_token_whitelisted(&env, &addr);
                    addr
                }
                None => Self::load_token(&env),
            };

            let vault_id = Self::vault_count(env.clone()) + 1;
            let timestamp = env.ledger().timestamp();
            let metadata = String::from_str(&env, "");
            Self::assert_metadata_len(&env, &metadata);
            let vault = Vault {
                owner: owner.clone(),
                beneficiary: beneficiary.clone(),
                balance: 0,
                check_in_interval,
                last_check_in: timestamp,
                created_at: timestamp,
                status: ReleaseStatus::Locked,
                beneficiaries: Vec::new(&env),
                metadata,
                token_address: vault_token,
                custom_metadata: Bytes::new(&env),
                is_paused: false,
                release_condition: ReleaseCondition::OnExpiry,
                parent_vault_id: None,
                passkey_hash: None,
                max_deposit_amount: None,
                withdrawal_approval_threshold: None,
            };
            Self::save_vault(&env, vault_id, &vault);
            Self::add_owner_vault_id(&env, &owner, vault_id, check_in_interval);
            Self::add_beneficiary_vault_id(&env, &beneficiary, vault_id, check_in_interval);
            // Initialize empty passkeys and backup codes
            let empty_passkeys: Vec<PasskeyHash> = Vec::new(&env);
            let empty_codes: Vec<BackupCode> = Vec::new(&env);
            env.storage().persistent().set(&DataKey::VaultPasskeys(vault_id), &empty_passkeys);
            env.storage().persistent().set(&DataKey::BackupCodes(vault_id), &empty_codes);
            let ttl = vault_ttl_ledgers(check_in_interval);
            env.storage().persistent().extend_ttl(&DataKey::VaultPasskeys(vault_id), VAULT_TTL_THRESHOLD, ttl);
            env.storage().persistent().extend_ttl(&DataKey::BackupCodes(vault_id), VAULT_TTL_THRESHOLD, ttl);
            // VaultCount is an incrementing generation ID and must be updated
            // only after the vault and its owner/beneficiary indexes are persisted.
            //
            // Ordering guarantee:
            //  1) Compute next ID from current vault count
            //  2) Persist the vault and owner/beneficiary indexes
            //  3) Persist VaultCount only after the vault is fully saved
            // If any prior call (save_vault/add_owner_vault_id/add_beneficiary_vault_id)
            // panics, VaultCount remains unchanged and consumers cannot observe
            // a hole in the sequence.
            let key = DataKey::VaultCount;
            env.storage().persistent().set(&key, &vault_id);
            env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
            env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
            
            env.events().publish(
                (VAULT_CREATED_TOPIC,),
                (vault_id, owner, beneficiary, check_in_interval, timestamp),
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
    pub fn check_in(env: Env, vault_id: u64, caller: Address, passkey_hash: BytesN<32>) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if vault.is_paused {
            return Err(ContractError::Paused);
        }
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        
        // Save original state for rollback on failure - Issue #391
        let original_last_check_in = vault.last_check_in;
        
        let now = env.ledger().timestamp();
        if let Some(expiry) = Self::get_passkey_expiry(env.clone(), vault_id, passkey_hash.clone()) {
            if now > expiry {
                return Err(ContractError::InvalidAmount); // Reusing error for expired passkey
            }
        }
        
        vault.last_check_in = now;
        
        // Cap TTL at max_ttl_seconds
        let max_ttl = Self::get_max_ttl_seconds(env.clone());
        let deadline = now + vault.check_in_interval;
        let max_deadline = now + max_ttl;
        if deadline > max_deadline {
            // Rollback on failure - Issue #391
            vault.last_check_in = original_last_check_in;
            return Err(ContractError::MaxTtlExceeded);
        }
        
        // Attempt to save vault - if this fails, TTL is not extended
        Self::save_vault(&env, vault_id, &vault);
        let owner_ids = Self::load_owner_vault_ids(&env, &vault.owner);
        Self::save_owner_vault_ids(&env, &vault.owner, &owner_ids, vault.check_in_interval);
        
        // Log passkey usage - Issue #395
        Self::log_passkey_usage(&env, vault_id, &passkey_hash, now);
        
        Self::log_audit_entry(&env, vault_id, "check_in", &caller, "");
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
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
        Self::require_initialized(&env);
        if amount <= 0 {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }
        from.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if vault.is_paused {
            panic_with_error!(&env, ContractError::Paused);
        }
        if vault.status != ReleaseStatus::Locked {
            panic_with_error!(&env, ContractError::AlreadyReleased);
        }

        let now = env.ledger().timestamp();
        if now >= vault.last_check_in + vault.check_in_interval {
            panic_with_error!(&env, ContractError::VaultExpired);
        }

        // Check deposit limit - Issue #403
        if let Some(max_deposit) = vault.max_deposit_amount {
            let new_balance = vault.balance.checked_add(amount)
                .unwrap_or_else(|| panic_with_error!(&env, ContractError::BalanceOverflow));
            if new_balance > max_deposit {
                panic_with_error!(&env, ContractError::DepositLimitExceeded);
            }
        }

        // Use vault's token instead of default XLM
        let token_client = token::Client::new(&env, &vault.token_address);
        token_client.transfer(&from, &env.current_contract_address(), &amount);
        vault.balance = vault.balance
            .checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::BalanceOverflow));
        Self::save_vault(&env, vault_id, &vault);
        Self::log_audit_entry(&env, vault_id, "deposit", &from, "");
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (DEPOSIT_TOPIC, vault_id),
            (amount, vault.balance),
        );
    }

    /// Deposits funds into multiple vaults in a single transfer.
    ///
    /// This is more efficient than calling `deposit` multiple times as it only
    /// requires one token transfer. All vaults must be in Locked status and not expired.
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
    /// * Panics if any vault has expired
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

            let now = env.ledger().timestamp();
            if now >= vault.last_check_in + vault.check_in_interval {
                panic_with_error!(&env, ContractError::VaultExpired);
            }

            total_amount = total_amount
                .checked_add(amount)
                .unwrap_or_else(|| panic_with_error!(&env, ContractError::InvalidAmount));
            validated.push_back((vault_id, vault, amount));
        }

        if total_amount == 0 {
            env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
            return;
        }

        // Note: batch_deposit now requires all vaults to use the same token (default XLM)
        // For multi-token support, use individual deposit calls
        let default_token = Self::load_token(&env);
        let token_client = token::Client::new(&env, &default_token);
        token_client.transfer(&from, &env.current_contract_address(), &total_amount);

        for validated_deposit in validated.iter() {
            let (vault_id, mut vault, amount) = validated_deposit;
            // Verify vault uses default token
            if vault.token_address != default_token {
                panic_with_error!(&env, ContractError::InvalidAmount);
            }
            vault.balance = vault.balance
                .checked_add(amount)
                .unwrap_or_else(|| panic_with_error!(&env, ContractError::BalanceOverflow));
            Self::save_vault(&env, vault_id, &vault);
        }
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    /// Owner withdraws from the vault.
    ///
    /// This function is owner-only and is unaffected by any multi-beneficiary
    /// split configured via `set_beneficiaries`. Beneficiary splits only apply
    /// during `trigger_release` and `partial_release`; `withdraw` always sends
    /// funds directly back to the vault owner regardless of the beneficiaries list.
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
    pub fn withdraw(env: Env, vault_id: u64, caller: Address, amount: i128) -> Result<(), ContractError> {
            if Self::load_paused(&env) {
                return Err(ContractError::Paused);
            }
            if amount <= 0 {
                return Err(ContractError::InvalidAmount);
            }
            caller.require_auth();
            let mut vault = Self::load_vault(&env, vault_id);
            if vault.is_paused {
                return Err(ContractError::Paused);
            }
            if caller != vault.owner {
                return Err(ContractError::NotOwner);
            }
            if vault.status != ReleaseStatus::Locked {
                return Err(ContractError::AlreadyReleased);
            }
            if vault.balance < amount {
                return Err(ContractError::InsufficientBalance);
            }

            // Check withdrawal approval threshold - Issue #404
            if let Some(threshold) = vault.withdrawal_approval_threshold {
                if amount > threshold {
                    return Err(ContractError::WithdrawalNotApproved);
                }
            }

            let token_client = token::Client::new(&env, &vault.token_address);
            token_client.transfer(&env.current_contract_address(), &vault.owner, &amount);
            vault.balance -= amount;
            Self::save_vault(&env, vault_id, &vault);
            Self::log_audit_entry(&env, vault_id, "withdraw", &caller, "");
            env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
            env.events().publish(
                (WITHDRAW_TOPIC, vault_id),
                (amount, vault.balance),
            );
            Ok(())
        }

    // --- Issue #318: batch_withdraw ---

    /// Withdraws from multiple vaults owned by the same caller in a single transaction.
    ///
    /// This is more efficient than calling `withdraw` multiple times as it reduces
    /// transaction overhead. All vault_ids and amounts are validated before any
    /// state mutation occurs.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_ids` - Vector of vault IDs to withdraw from
    /// * `amounts` - Vector of amounts (in stroops) to withdraw from each vault
    /// * `caller` - The address of the caller (must be the owner of all vaults)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::InvalidAmount` - If vault_ids.len() != amounts.len() or any amount is not positive
    /// * `ContractError::VaultNotFound` - If any vault does not exist
    /// * `ContractError::NotOwner` - If caller is not the owner of any vault
    /// * `ContractError::AlreadyReleased` - If any vault is not in Locked status
    /// * `ContractError::InsufficientBalance` - If any vault balance is less than the requested amount
    pub fn batch_withdraw(
        env: Env,
        vault_ids: Vec<u64>,
        amounts: Vec<i128>,
        caller: Address,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        if vault_ids.len() != amounts.len() {
            return Err(ContractError::InvalidAmount);
        }
        caller.require_auth();

        // Validate all entries before mutating state
        for (vault_id, amount) in vault_ids.iter().zip(amounts.iter()) {
            if amount <= 0 {
                return Err(ContractError::InvalidAmount);
            }
            let vault = Self::try_load_vault(&env, vault_id)
                .ok_or(ContractError::VaultNotFound)?;
            if caller != vault.owner {
                return Err(ContractError::NotOwner);
            }
            if vault.status != ReleaseStatus::Locked {
                return Err(ContractError::AlreadyReleased);
            }
            if vault.balance < amount {
                return Err(ContractError::InsufficientBalance);
            }
        }

        // All validations passed — apply withdrawals
        // Note: batch_withdraw requires all vaults to use the same token (default XLM)
        let default_token = Self::load_token(&env);
        let token_client = token::Client::new(&env, &default_token);
        for (vault_id, amount) in vault_ids.iter().zip(amounts.iter()) {
            let mut vault = Self::load_vault(&env, vault_id);
            if vault.token_address != default_token {
                return Err(ContractError::InvalidAmount);
            }
            token_client.transfer(&env.current_contract_address(), &vault.owner, &amount);
            vault.balance -= amount;
            let remaining = vault.balance;
            Self::save_vault(&env, vault_id, &vault);
            env.events().publish(
                (WITHDRAW_TOPIC, vault_id),
                (amount, remaining),
            );
        }
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(())
    }

    // --- Issue #319: batch_check_in ---

    /// Records check-ins for multiple vaults owned by the same caller in a single transaction.
    ///
    /// This is more efficient than calling `check_in` multiple times as it reduces
    /// transaction overhead. All vaults are validated before any state mutation occurs.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_ids` - Vector of vault IDs to check in
    /// * `caller` - The address of the caller (must be the owner of all vaults)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::VaultNotFound` - If any vault does not exist
    /// * `ContractError::NotOwner` - If caller is not the owner of any vault
    /// * `ContractError::AlreadyReleased` - If any vault is not in Locked status
    pub fn batch_check_in(
        env: Env,
        vault_ids: Vec<u64>,
        caller: Address,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        caller.require_auth();

        // Validate all entries before mutating state
        for vault_id in vault_ids.iter() {
            let vault = Self::try_load_vault(&env, vault_id)
                .ok_or(ContractError::VaultNotFound)?;
            if vault.is_paused {
                return Err(ContractError::Paused);
            }
            if caller != vault.owner {
                return Err(ContractError::NotOwner);
            }
            if vault.status != ReleaseStatus::Locked {
                return Err(ContractError::AlreadyReleased);
            }
        }

        // All validations passed — apply check-ins
        let now = env.ledger().timestamp();
        for vault_id in vault_ids.iter() {
            let mut vault = Self::load_vault(&env, vault_id);
            vault.last_check_in = now;
            Self::save_vault(&env, vault_id, &vault);
            env.events().publish((CHECK_IN_TOPIC, vault_id), now);
        }
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(())
    }

    /// Triggers the release of funds to beneficiaries after the vault expires.
    ///
    /// Anyone can call this function once the vault's TTL has lapsed. If a vesting
    /// schedule is attached, the vault is marked as Released but funds remain locked
    /// until claimed via `claim_vested_installment`. Otherwise, funds are distributed
    /// immediately to the primary beneficiary or split among multiple beneficiaries
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
        if total == 0 {
            panic_with_error!(&env, ContractError::EmptyVault);
        }

        // Check beneficiary acceptance status - Issue #397
        let beneficiary_status = Self::get_beneficiary_status(env.clone(), vault_id);
        if beneficiary_status == BeneficiaryStatus::Declined {
            panic_with_error!(&env, ContractError::InvalidBeneficiary);
        }

        // Check if a vesting schedule is attached
        let has_vesting = env
            .storage()
            .persistent()
            .has(&DataKey::VestingSchedule(vault_id));

        if has_vesting {
            // Vesting schedule exists: mark as Released but keep balance intact
            vault.status = ReleaseStatus::Released;
            Self::save_vault(&env, vault_id, &vault);
            env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
            env.events().publish(
                (RELEASE_TOPIC,),
                ReleaseEvent { vault_id, beneficiary: vault.beneficiary.clone(), amount: 0 },
            );
        } else {
            // No vesting: immediate full release
            let token_client = token::Client::new(&env, &vault.token_address);

            if vault.beneficiaries.is_empty() {
                token_client.transfer(&env.current_contract_address(), &vault.beneficiary, &total);
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
                        token_client.transfer(&env.current_contract_address(), &entry.address, &share);
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
    }

    /// Applies TTL decay to a vault if no check-in for 30 days.
    ///
    /// Anyone can call this function. If the vault hasn't been checked in for 30 days,
    /// the TTL is reduced by the configured decay rate. This encourages regular engagement.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// `Ok(new_ttl_remaining)` with the remaining TTL after decay, or `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::VaultNotFound` - If vault does not exist
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn apply_ttl_decay(env: Env, vault_id: u64) -> Result<u64, ContractError> {
        let mut vault = Self::try_load_vault(&env, vault_id)
            .ok_or(ContractError::VaultNotFound)?;
        
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        
        let decay_rate = Self::get_ttl_decay_rate(env.clone());
        if decay_rate == 0 {
            // No decay configured
            return Ok(Self::get_ttl_remaining(env, vault_id).unwrap_or(0));
        }
        
        let now = env.ledger().timestamp();
        let last_check_in = vault.last_check_in;
        let thirty_days = 30 * 24 * 60 * 60; // 2,592,000 seconds
        
        // Only apply decay if no check-in for 30 days
        if now < last_check_in + thirty_days {
            return Ok(Self::get_ttl_remaining(env, vault_id).unwrap_or(0));
        }
        
        // Calculate new TTL with decay applied
        let current_deadline = last_check_in + vault.check_in_interval;
        let remaining = if now >= current_deadline {
            0u64
        } else {
            current_deadline - now
        };
        
        // Apply decay: new_ttl = remaining * (1 - decay_rate / 10000)
        let decayed_ttl = remaining * (10_000 - decay_rate as u64) / 10_000;
        let new_deadline = now + decayed_ttl;
        
        // Update last_check_in to reflect the decay application
        vault.last_check_in = new_deadline - vault.check_in_interval;
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((TTL_DECAY_TOPIC, vault_id), (remaining, decayed_ttl));
        
        Ok(decayed_ttl)
    }

    /// Synchronizes TTL across multiple vaults owned by the caller.
    ///
    /// Extends TTL for all specified vaults in a single transaction. This is more
    /// efficient than calling `check_in` multiple times. All vaults must be owned
    /// by the caller and in Locked status.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_ids` - Vector of vault IDs to synchronize
    /// * `caller` - The address of the caller (must be the owner of all vaults)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::VaultNotFound` - If any vault does not exist
    /// * `ContractError::NotOwner` - If caller is not the owner of any vault
    /// * `ContractError::AlreadyReleased` - If any vault is not in Locked status
    /// * `ContractError::MaxTtlExceeded` - If any vault would exceed max TTL
    pub fn sync_vault_ttls(
        env: Env,
        vault_ids: Vec<u64>,
        caller: Address,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        caller.require_auth();

        // Validate all vaults before mutating state
        for vault_id in vault_ids.iter() {
            let vault = Self::try_load_vault(&env, vault_id)
                .ok_or(ContractError::VaultNotFound)?;
            if caller != vault.owner {
                return Err(ContractError::NotOwner);
            }
            if vault.status != ReleaseStatus::Locked {
                return Err(ContractError::AlreadyReleased);
            }
            
            // Check max TTL constraint
            let max_ttl = Self::get_max_ttl_seconds(env.clone());
            let now = env.ledger().timestamp();
            let deadline = now + vault.check_in_interval;
            let max_deadline = now + max_ttl;
            if deadline > max_deadline {
                return Err(ContractError::MaxTtlExceeded);
            }
        }

        // All validations passed — apply check-ins
        let now = env.ledger().timestamp();
        for vault_id in vault_ids.iter() {
            let mut vault = Self::load_vault(&env, vault_id);
            vault.last_check_in = now;
            Self::save_vault(&env, vault_id, &vault);
            env.events().publish((CHECK_IN_TOPIC, vault_id), now);
        }
        
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((SYNC_TTL_TOPIC,), vault_ids.len());
        Ok(())
    }

    /// Forecasts the expected expiry time of a vault based on check-in frequency.
    ///
    /// Calculates when the vault will expire if check-ins continue at the specified
    /// frequency. This helps owners plan ahead and ensure timely check-ins.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `check_in_frequency_days` - Expected check-in frequency in days
    ///
    /// # Returns
    /// Unix timestamp of the expected expiry, or `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::VaultNotFound` - If vault does not exist
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn forecast_expiry(env: Env, vault_id: u64, check_in_frequency_days: u64) -> Result<u64, ContractError> {
        let vault = Self::try_load_vault(&env, vault_id)
            .ok_or(ContractError::VaultNotFound)?;
        
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        
        if check_in_frequency_days == 0 {
            return Err(ContractError::InvalidInterval);
        }
        
        let now = env.ledger().timestamp();
        let check_in_frequency_seconds = check_in_frequency_days * 24 * 60 * 60;
        
        // Current deadline
        let current_deadline = vault.last_check_in + vault.check_in_interval;
        
        // If already expired, return current time
        if now >= current_deadline {
            return Ok(now);
        }
        
        // Calculate how many check-ins until expiry at the given frequency
        let remaining_until_expiry = current_deadline - now;
        let num_check_ins = (remaining_until_expiry + check_in_frequency_seconds - 1) / check_in_frequency_seconds;
        
        // Each check-in extends TTL by vault.check_in_interval
        let total_extension = num_check_ins * vault.check_in_interval;
        let forecasted_expiry = now + total_extension;
        
        Ok(forecasted_expiry)
    }

    // --- Task 1: ping_expiry ---

    /// Checks the remaining TTL and emits a warning event if near expiry.
    ///
    /// This function can be called by anyone to monitor vault expiry status.
    /// If the remaining TTL is less than `EXPIRY_WARNING_THRESHOLD` (24 hours),
    /// a warning event is emitted. No event is emitted for Released or Cancelled vaults.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// The remaining TTL in seconds (0 if expired)
    pub fn ping_expiry(env: Env, vault_id: u64) -> u64 {
        let vault = Self::try_load_vault(&env, vault_id)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::VaultNotFound));
        
        // Only emit events for Locked vaults
        if vault.status != ReleaseStatus::Locked {
            return 0;
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
    /// * `ContractError::VaultExpired` - If vault has expired
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
        if Self::is_expired(env.clone(), vault_id) {
            return Err(ContractError::VaultExpired);
        }
        if vault.balance < amount {
            return Err(ContractError::InsufficientBalance);
        }
        let token_client = token::Client::new(&env, &vault.token_address);

        if vault.beneficiaries.is_empty() {
            // Single-beneficiary path: send full amount to primary beneficiary.
            token_client.transfer(&env.current_contract_address(), &vault.beneficiary, &amount);
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
                    token_client.transfer(&env.current_contract_address(), &entry.address, &share);
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
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
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
            caller: Address,
            beneficiaries: Vec<BeneficiaryEntry>,
        ) -> Result<(), ContractError> {
            caller.require_auth();
            if beneficiaries.is_empty() {
                return Err(ContractError::InvalidBps);
            }
            let mut vault = Self::load_vault(&env, vault_id);
            if caller != vault.owner {
                return Err(ContractError::NotOwner);
            }
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
            vault.beneficiaries = beneficiaries.clone();
            Self::save_vault(&env, vault_id, &vault);
            env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
            env.events().publish((SET_BENEFICIARIES_TOPIC, vault_id), beneficiaries);
            Ok(())
        }

    /// Adds a single beneficiary to a vault's multi-beneficiary split.
    ///
    /// This function adds a new beneficiary with the specified BPS allocation.
    /// The total BPS across all beneficiaries must not exceed 10,000 (100%).
    /// If adding this beneficiary would exceed 10,000 BPS, the operation fails.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `caller` - The address of the caller (must be the vault owner)
    /// * `address` - The beneficiary address to add
    /// * `percentage` - The BPS allocation (0-10000)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    /// * `ContractError::InvalidBps` - If total BPS would exceed 10,000
    /// * `ContractError::InvalidBeneficiary` - If address is the vault owner
    pub fn add_beneficiary(
        env: Env,
        vault_id: u64,
        caller: Address,
        address: Address,
        percentage: u32,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        if address == vault.owner {
            return Err(ContractError::InvalidBeneficiary);
        }
        
        // Check if beneficiary already exists
        for entry in vault.beneficiaries.iter() {
            if entry.address == address {
                return Err(ContractError::InvalidBeneficiary);
            }
        }
        
        // Calculate total BPS after adding new beneficiary
        let current_total: u32 = vault.beneficiaries.iter().map(|e| e.bps).sum();
        if current_total + percentage > 10_000 {
            return Err(ContractError::InvalidBps);
        }
        
        vault.beneficiaries.push_back(BeneficiaryEntry {
            address: address.clone(),
            bps: percentage,
        });
        
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((BENEFICIARY_UPDATED_TOPIC, vault_id), (address, percentage));
        Ok(())
    }

    /// Removes a beneficiary from a vault's multi-beneficiary split.
    ///
    /// This function removes an existing beneficiary from the vault's beneficiaries list.
    /// If the beneficiary is not found, the operation fails.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `caller` - The address of the caller (must be the vault owner)
    /// * `address` - The beneficiary address to remove
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    /// * `ContractError::InvalidBeneficiary` - If beneficiary is not found
    pub fn remove_beneficiary(
        env: Env,
        vault_id: u64,
        caller: Address,
        address: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        
        let mut found = false;
        let mut new_beneficiaries = Vec::new(&env);
        for entry in vault.beneficiaries.iter() {
            if entry.address != address {
                new_beneficiaries.push_back(entry);
            } else {
                found = true;
            }
        }
        
        if !found {
            return Err(ContractError::InvalidBeneficiary);
        }
        
        vault.beneficiaries = new_beneficiaries;
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((BENEFICIARY_UPDATED_TOPIC, vault_id), address);
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
    pub fn update_metadata(env: Env, vault_id: u64, caller: Address, metadata: String) -> Result<(), ContractError> {
            caller.require_auth();
            if metadata.len() > MAX_METADATA_LEN {
                return Err(ContractError::InvalidAmount);
            }
            let mut vault = Self::load_vault(&env, vault_id);
            if caller != vault.owner {
                return Err(ContractError::NotOwner);
            }
            if vault.status != ReleaseStatus::Locked {
                return Err(ContractError::AlreadyReleased);
            }
            vault.metadata = metadata.clone();
            Self::save_vault(&env, vault_id, &vault);
            Self::log_audit_entry(&env, vault_id, "update_metadata", &caller, "");
            env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
            env.events().publish((UPDATE_METADATA_TOPIC, vault_id), metadata);
            Ok(())
        }

    /// Sets the vault name, description, and notes fields.
    /// 
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// * `caller` - The address calling this function (must be vault owner)
    /// * `name` - Vault name/title (max 64 chars)
    /// * `description` - Vault description (max 512 chars)
    /// * `notes` - Notes/instructions for beneficiary (max 1024 chars)
    /// 
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    /// * `ContractError::InvalidAmount` - If any field exceeds size limits
    pub fn set_vault_notes(
        env: Env,
        vault_id: u64,
        caller: Address,
        name: String,
        description: String,
        notes: String,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        
        // Validate field lengths
        if name.len() > MAX_NAME_LEN {
            return Err(ContractError::InvalidAmount);
        }
        if description.len() > MAX_DESCRIPTION_LEN {
            return Err(ContractError::InvalidAmount);
        }
        if notes.len() > MAX_NOTES_LEN {
            return Err(ContractError::InvalidAmount);
        }
        
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        
        vault.name = name;
        vault.description = description;
        vault.notes = notes;
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(())
    }

    /// Gets the vault metadata fields (name, description, notes).
    /// 
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// 
    /// # Returns
    /// A tuple of (name, description, notes)
    pub fn get_vault_notes(env: Env, vault_id: u64) -> (String, String, String) {
        let vault = Self::load_vault(&env, vault_id);
        (vault.name, vault.description, vault.notes)
    }

    // --- Task 5: vesting schedules ---

    /// Attaches a vesting schedule to a vault.
    ///
    /// Once set, the vault's balance is released to the beneficiary (or beneficiaries)
    /// in `num_installments` equal tranches. Each tranche becomes claimable every
    /// `interval` seconds starting from `start_time`.
    ///
    /// The vault must have been released (trigger_release called) before installments
    /// can be claimed. The schedule is set by the owner while the vault is still Locked.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault to attach the schedule to
    /// * `caller` - Must be the vault owner
    /// * `start_time` - Unix timestamp of the first claimable installment
    /// * `interval` - Seconds between installments (must be > 0)
    /// * `num_installments` - Number of tranches (must be > 0)
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not Locked
    /// * `ContractError::InvalidInterval` - If interval or num_installments is 0
    /// * `ContractError::EmptyVault` - If vault balance is 0
    pub fn set_vesting_schedule(
        env: Env,
        vault_id: u64,
        caller: Address,
        start_time: u64,
        interval: u64,
        num_installments: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        if interval == 0 || num_installments == 0 {
            return Err(ContractError::InvalidInterval);
        }
        if vault.balance == 0 {
            return Err(ContractError::EmptyVault);
        }
        let schedule = VestingSchedule {
            start_time,
            interval,
            num_installments,
            claimed_installments: 0,
            total_amount: vault.balance,
        };
        let key = DataKey::VestingSchedule(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &schedule);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (SET_VESTING_TOPIC, vault_id),
            (start_time, interval, num_installments, vault.balance),
        );
        Ok(())
    }

    /// Returns the vesting schedule for a vault, if one exists.
    pub fn get_vesting_schedule(env: Env, vault_id: u64) -> Option<VestingSchedule> {
        env.storage().persistent().get(&DataKey::VestingSchedule(vault_id))
    }

    /// Claims all vested installments that have become available since the last claim.
    ///
    /// The vault must have been released (trigger_release called) and a vesting schedule
    /// must be attached. The beneficiary (or any caller) can invoke this once the vault
    /// is Released and at least one installment window has elapsed since `start_time`.
    ///
    /// Funds are distributed to the primary beneficiary, or split among multi-beneficiaries
    /// using the same BPS logic as `trigger_release`.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault to claim from
    ///
    /// # Returns
    /// The total amount transferred in this call
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::VestingNotFound` - If no vesting schedule exists
    /// * `ContractError::NothingToClaimYet` - If no new installments are available
    /// * `ContractError::VestingAlreadyComplete` - If all installments have been claimed
    /// * `ContractError::InsufficientBalance` - If vault balance is insufficient
    pub fn claim_vested_installment(env: Env, vault_id: u64) -> Result<i128, ContractError> {
        Self::assert_not_paused(&env);
        let mut vault = Self::load_vault(&env, vault_id);

        // Vault must be Released for vesting claims
        if vault.status != ReleaseStatus::Released {
            return Err(ContractError::AlreadyReleased);
        }

        let mut schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::VestingSchedule(vault_id))
            .ok_or(ContractError::VestingNotFound)?;

        if schedule.claimed_installments >= schedule.num_installments {
            return Err(ContractError::VestingAlreadyComplete);
        }

        let now = env.ledger().timestamp();
        if now < schedule.start_time {
            return Err(ContractError::NothingToClaimYet);
        }

        // How many installments are unlocked so far?
        let elapsed = now - schedule.start_time;
        let unlocked = ((elapsed / schedule.interval) + 1).min(schedule.num_installments as u64) as u32;
        let claimable = unlocked.saturating_sub(schedule.claimed_installments);
        if claimable == 0 {
            return Err(ContractError::NothingToClaimYet);
        }

        // Calculate payout: each installment = total / num_installments,
        // last installment absorbs remainder.
        let per_installment = schedule.total_amount / schedule.num_installments as i128;
        let amount = if unlocked >= schedule.num_installments {
            // Final batch: pay out everything remaining in the vault
            vault.balance
        } else {
            per_installment * claimable as i128
        };

        if vault.balance < amount {
            return Err(ContractError::InsufficientBalance);
        }

        let token_client = token::Client::new(&env, &vault.token_address);

        if vault.beneficiaries.is_empty() {
            token_client.transfer(&env.current_contract_address(), &vault.beneficiary, &amount);
            env.events().publish(
                (CLAIM_VEST_TOPIC, vault_id),
                (vault.beneficiary.clone(), amount, unlocked),
            );
        } else {
            let mut distributed: i128 = 0;
            let last_idx = vault.beneficiaries.len() - 1;
            for (i, entry) in vault.beneficiaries.iter().enumerate() {
                let share = if i as u32 == last_idx {
                    amount - distributed
                } else {
                    amount * (entry.bps as i128) / 10_000
                };
                if share > 0 {
                    token_client.transfer(&env.current_contract_address(), &entry.address, &share);
                }
                distributed += share;
                env.events().publish(
                    (CLAIM_VEST_TOPIC, vault_id),
                    (entry.address.clone(), share, unlocked),
                );
            }
        }

        vault.balance -= amount;
        schedule.claimed_installments = unlocked;
        Self::save_vault(&env, vault_id, &vault);

        let sched_key = DataKey::VestingSchedule(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&sched_key, &schedule);
        env.storage().persistent().extend_ttl(&sched_key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(amount)
    }

    // --- views ---

    /// Checks if a vault has expired based on the check-in interval.
    ///
    /// A vault is considered expired when the current timestamp is greater than
    /// or equal to the deadline (last_check_in + check_in_interval).
    ///
    /// # Ledger time monotonicity assumption
    /// This function relies on `env.ledger().timestamp()` being monotonically
    /// non-decreasing across ledger closes. The Stellar consensus protocol
    /// guarantees this property: each ledger's close time must be strictly
    /// greater than the previous ledger's close time. Clock skew between
    /// individual validators does not affect this guarantee because the
    /// agreed-upon close time is determined by consensus, not by any single
    /// node's wall clock. Therefore the comparison `now >= deadline` is
    /// reliable and will never produce a false expiry due to time going
    /// backwards.
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
    /// This is a pure read-only function. It does **not** extend the vault's
    /// persistent storage TTL. Extending TTL on every read would introduce
    /// unintended side effects: callers (including off-chain indexers) would
    /// inadvertently pay storage fees and mutate ledger state. TTL extension
    /// is intentionally reserved for state-mutating operations such as
    /// `check_in`, `deposit`, and `withdraw`. If a vault is only ever read
    /// and never written to, its storage TTL will eventually lapse and the
    /// entry will be archived by the network.
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

    /// Returns the last check-in timestamp for a vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// The Unix timestamp of the last check-in
    pub fn get_vault_last_check_in(env: Env, vault_id: u64) -> u64 {
        Self::load_vault(&env, vault_id).last_check_in
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
    /// * `status_filter` - Optional status filter (None returns all vaults, Some(status) returns only vaults with that status)
    /// * `page` - Zero-based page index
    /// * `page_size` - Number of items per page
    ///
    /// # Returns
    /// A vector of vault IDs for the requested page
    pub fn get_vaults_by_owner(env: Env, owner: Address, status_filter: Option<ReleaseStatus>, page: u32, page_size: u32) -> Vec<u64> {
        let all = Self::load_owner_vault_ids(&env, &owner);
        let filtered = if let Some(status) = status_filter {
            let mut result = Vec::new(&env);
            for vault_id in all.iter() {
                if let Some(vault) = Self::try_load_vault(&env, vault_id) {
                    if vault.status == status {
                        result.push_back(vault_id);
                    }
                }
            }
            result
        } else {
            all
        };
        Self::paginate(&env, filtered, page, page_size)
    }

    /// Returns a paginated slice of vault IDs where a specific address is the beneficiary.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `beneficiary` - The beneficiary address
    /// * `status_filter` - Optional status filter (None returns all vaults, Some(status) returns only vaults with that status)
    /// * `page` - Zero-based page index
    /// * `page_size` - Number of items per page
    /// Returns all vault IDs associated with a beneficiary, including released and cancelled vaults.
    /// Use `get_active_vaults_by_beneficiary` to retrieve only locked (active) vaults.
    ///
    /// # Returns
    /// A vector of vault IDs for the requested page
    pub fn get_vaults_by_beneficiary(env: Env, beneficiary: Address, status_filter: Option<ReleaseStatus>, page: u32, page_size: u32) -> Vec<u64> {
        let all = Self::load_beneficiary_vault_ids(&env, &beneficiary);
        let filtered = if let Some(status) = status_filter {
            let mut result = Vec::new(&env);
            for vault_id in all.iter() {
                if let Some(vault) = Self::try_load_vault(&env, vault_id) {
                    if vault.status == status {
                        result.push_back(vault_id);
                    }
                }
            }
            result
        } else {
            all
        };
        Self::paginate(&env, filtered, page, page_size)
    }

    /// Returns only active (Locked) vault IDs for a beneficiary, excluding released and cancelled vaults.
    pub fn get_active_vaults_by_beneficiary(env: Env, beneficiary: Address, page: u32, page_size: u32) -> Vec<u64> {
        let all = Self::load_beneficiary_vault_ids(&env, &beneficiary);
        let mut active = Vec::new(&env);
        for id in all.iter() {
            if let Some(v) = Self::try_load_vault(&env, id) {
                if v.status == ReleaseStatus::Locked {
                    active.push_back(id);
                }
            }
        }
        Self::paginate(&env, active, page, page_size)
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
        env.storage().persistent().get(&DataKey::VaultCount).unwrap_or(0u64)
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

    /// Returns all vault IDs where the given address is a beneficiary - Issue #398
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `beneficiary` - The beneficiary address
    ///
    /// # Returns
    /// A vector of vault IDs where the address is a beneficiary
    pub fn get_vaults_as_beneficiary(env: Env, beneficiary: Address) -> Vec<u64> {
        Self::load_beneficiary_vault_ids(&env, &beneficiary)
    }

    /// Returns passkey usage history for a vault - Issue #395
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    ///
    /// # Returns
    /// A vector of PasskeyUsageEntry records
    pub fn get_passkey_usage(env: Env, vault_id: u64) -> Vec<PasskeyUsageEntry> {
        env.storage()
            .persistent()
            .get(&DataKey::PasskeyUsage(vault_id))
            .unwrap_or(Vec::new(&env))
    }

    /// Accepts the beneficiary role for a vault - Issue #397
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `caller` - The beneficiary address (must authorize)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    pub fn accept_beneficiary_role(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.beneficiary {
            return Err(ContractError::NotOwner);
        }
        env.storage().persistent().set(&DataKey::BeneficiaryStatus(vault_id), &BeneficiaryStatus::Accepted);
        env.storage().persistent().extend_ttl(&DataKey::BeneficiaryStatus(vault_id), VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.events().publish((BENEFICIARY_ACCEPTED_TOPIC, vault_id), caller);
        Ok(())
    }

    /// Declines the beneficiary role for a vault - Issue #397
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `caller` - The beneficiary address (must authorize)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    pub fn decline_beneficiary_role(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.beneficiary {
            return Err(ContractError::NotOwner);
        }
        env.storage().persistent().set(&DataKey::BeneficiaryStatus(vault_id), &BeneficiaryStatus::Declined);
        env.storage().persistent().extend_ttl(&DataKey::BeneficiaryStatus(vault_id), VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.events().publish((BENEFICIARY_DECLINED_TOPIC, vault_id), caller);
        Ok(())
    }

    /// Gets the beneficiary status for a vault - Issue #397
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    ///
    /// # Returns
    /// The BeneficiaryStatus (defaults to Pending if not set)
    pub fn get_beneficiary_status(env: Env, vault_id: u64) -> BeneficiaryStatus {
        env.storage()
            .persistent()
            .get(&DataKey::BeneficiaryStatus(vault_id))
            .unwrap_or(BeneficiaryStatus::Pending)
    }

    /// Extends passkey expiry - Issue #396
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `caller` - The vault owner (must authorize)
    /// * `passkey_hash` - The passkey hash to extend
    /// * `new_expiry` - New expiry timestamp
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    pub fn extend_passkey_expiry(env: Env, vault_id: u64, caller: Address, passkey_hash: BytesN<32>, new_expiry: u64) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        env.storage().persistent().set(&DataKey::PasskeyExpiry(vault_id, passkey_hash.clone()), &new_expiry);
        env.storage().persistent().extend_ttl(&DataKey::PasskeyExpiry(vault_id, passkey_hash.clone()), VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.events().publish((PASSKEY_EXPIRY_EXTENDED_TOPIC, vault_id), (passkey_hash, new_expiry));
        Ok(())
    }

    /// Gets passkey expiry timestamp - Issue #396
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `passkey_hash` - The passkey hash
    ///
    /// # Returns
    /// The expiry timestamp, or None if not set
    pub fn get_passkey_expiry(env: Env, vault_id: u64, passkey_hash: BytesN<32>) -> Option<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::PasskeyExpiry(vault_id, passkey_hash))
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
    /// * `caller` - The address of the caller (must be the vault owner)
    /// * `new_beneficiary` - The new beneficiary address
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn update_beneficiary(env: Env, vault_id: u64, caller: Address, new_beneficiary: Address) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }

        if vault.owner == new_beneficiary {
            return Err(ContractError::InvalidBeneficiary);
        }

        let old_beneficiary = vault.beneficiary.clone();
        vault.beneficiary = new_beneficiary.clone();
        Self::save_vault(&env, vault_id, &vault);

        if old_beneficiary != new_beneficiary {
            Self::remove_beneficiary_vault_id(&env, &old_beneficiary, vault_id, vault.check_in_interval);
            Self::add_beneficiary_vault_id(&env, &new_beneficiary, vault_id, vault.check_in_interval);
        }
        Self::log_audit_entry(&env, vault_id, "update_beneficiary", &caller, "");
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((BENEFICIARY_UPDATED_TOPIC, vault_id), (old_beneficiary, new_beneficiary));
        Ok(())
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
        let old_interval = vault.check_in_interval;
        vault.check_in_interval = new_interval;
        vault.last_check_in = env.ledger().timestamp();
        Self::save_vault(&env, vault_id, &vault);
        // Explicitly re-extend the vault's persistent TTL using the new (potentially
        // longer) interval so storage outlives the updated check-in deadline.
        let new_ttl = vault_ttl_ledgers(new_interval);
        env.storage().persistent().extend_ttl(
            &DataKey::Vault(vault_id),
            VAULT_TTL_THRESHOLD,
            new_ttl,
        );
        Self::log_audit_entry(&env, vault_id, "update_check_in_interval", &vault.owner, "");
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((UPDATE_INTERVAL_TOPIC, vault_id), (old_interval, new_interval));
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
    /// * `caller` - The address of the caller (must be the vault owner)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn cancel_vault(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
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
        let refund_amount = vault.balance;
        if refund_amount > 0 {
            let token_client = token::Client::new(&env, &vault.token_address);
            token_client.transfer(&env.current_contract_address(), &vault.owner, &refund_amount);
        }
        vault.balance = 0;
        vault.status = ReleaseStatus::Cancelled;
        Self::save_vault(&env, vault_id, &vault);
        Self::remove_owner_vault_id(&env, &vault.owner, vault_id, vault.check_in_interval);
        Self::remove_beneficiary_vault_id(&env, &vault.beneficiary, vault_id, vault.check_in_interval);
        Self::log_audit_entry(&env, vault_id, "cancel_vault", &caller, "");
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
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
            caller: Address,
            new_owner: Address,
        ) -> Result<(), ContractError> {
            if Self::load_paused(&env) {
                return Err(ContractError::Paused);
            }
            caller.require_auth();
            let mut vault = Self::load_vault(&env, vault_id);
            let old_owner = vault.owner.clone();
            if caller != old_owner {
                return Err(ContractError::NotOwner);
            }
            if vault.status != ReleaseStatus::Locked {
                return Err(ContractError::AlreadyReleased);
            }
            // Invariant: owner and beneficiary must always be distinct addresses.
            // BeneficiaryVaults is keyed by beneficiary address. Because ownership
            // transfer never changes vault.beneficiary, the index requires no update:
            // the existing entry (beneficiary → vault_id) remains valid. Any other
            // vaults where new_owner appears as a beneficiary are unrelated entries
            // in the index and are also unaffected.
            if new_owner == vault.beneficiary {
                return Err(ContractError::InvalidBeneficiary);
            }
            if old_owner != new_owner {
                Self::remove_owner_vault_id(&env, &old_owner, vault_id, vault.check_in_interval);
                Self::add_owner_vault_id(&env, &new_owner, vault_id, vault.check_in_interval);
            }
            vault.owner = new_owner.clone();
            Self::save_vault(&env, vault_id, &vault);
            Self::log_audit_entry(&env, vault_id, "transfer_ownership", &caller, "");
            env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
            env.events().publish((OWNERSHIP_TOPIC, vault_id), (old_owner, new_owner));
            Ok(())
        }

    // --- Issue #378: Vault Metadata ---

    /// Sets custom metadata for a vault (max 2KB).
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `caller` - The address of the caller (must be the vault owner)
    /// * `metadata` - Custom metadata as bytes (max 2KB)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    /// * `ContractError::InvalidAmount` - If metadata exceeds 2KB
    pub fn set_vault_metadata(
        env: Env,
        vault_id: u64,
        caller: Address,
        metadata: Bytes,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        if metadata.len() > MAX_CUSTOM_METADATA_LEN {
            return Err(ContractError::InvalidAmount);
        }
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        vault.custom_metadata = metadata.clone();
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((SET_METADATA_TOPIC, vault_id), metadata);
        Ok(())
    }

    /// Gets custom metadata for a vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// The custom metadata bytes
    pub fn get_vault_metadata(env: Env, vault_id: u64) -> Bytes {
        let vault = Self::load_vault(&env, vault_id);
        vault.custom_metadata
    }

    // --- Issue #380: Vault Pause/Freeze ---

    /// Pauses a vault, preventing all operations until resumed.
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
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn pause_vault(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        vault.is_paused = true;
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((PAUSE_VAULT_TOPIC, vault_id), true);
        Ok(())
    }

    /// Resumes a paused vault, allowing operations to resume.
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
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn resume_vault(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        vault.is_paused = false;
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((RESUME_VAULT_TOPIC, vault_id), false);
        Ok(())
    }

    /// Checks if a vault is paused.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// `true` if the vault is paused, `false` otherwise
    pub fn is_vault_paused(env: Env, vault_id: u64) -> bool {
        if let Some(vault) = Self::try_load_vault(&env, vault_id) {
            vault.is_paused
        } else {
            false
        }
    }

    // --- Issue #379: Conditional Release Logic ---

    /// Sets the release condition for a vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `caller` - The address of the caller (must be the vault owner)
    /// * `condition` - The release condition
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn set_release_condition(
        env: Env,
        vault_id: u64,
        caller: Address,
        condition: ReleaseCondition,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        vault.release_condition = condition;
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(())
    }

    /// Gets the release condition for a vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// The release condition
    pub fn get_release_condition(env: Env, vault_id: u64) -> ReleaseCondition {
        let vault = Self::load_vault(&env, vault_id);
        vault.release_condition
    }

    // --- Issue #381: Vault Inheritance Chain ---

    /// Creates a new vault from inherited funds (beneficiary-only).
    ///
    /// The beneficiary of a released vault can create a new vault with the inherited funds,
    /// establishing an inheritance chain for multi-generational wealth transfer.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `parent_vault_id` - The ID of the parent vault (must be released)
    /// * `caller` - The address of the caller (must be the beneficiary of parent vault)
    /// * `new_beneficiary` - The beneficiary for the new vault
    /// * `check_in_interval` - Check-in interval for the new vault
    /// * `token_address` - Optional token address for the new vault
    ///
    /// # Returns
    /// The ID of the newly created vault
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the beneficiary of parent vault
    /// * `ContractError::NotExpired` - If parent vault is not released
    /// * `ContractError::InvalidInterval` - If check_in_interval is invalid
    pub fn create_vault_from_inheritance(
        env: Env,
        parent_vault_id: u64,
        caller: Address,
        new_beneficiary: Address,
        check_in_interval: u64,
        token_address: Option<Address>,
    ) -> u64 {
        caller.require_auth();
        Self::require_initialized(&env);
        
        let parent_vault = Self::load_vault(&env, parent_vault_id);
        if caller != parent_vault.beneficiary {
            panic_with_error!(&env, ContractError::NotOwner);
        }
        if parent_vault.status != ReleaseStatus::Released {
            panic_with_error!(&env, ContractError::NotExpired);
        }
        if check_in_interval == 0 {
            panic_with_error!(&env, ContractError::InvalidInterval);
        }
        Self::assert_interval_in_bounds(&env, check_in_interval);
        if caller == new_beneficiary {
            panic_with_error!(&env, ContractError::InvalidBeneficiary);
        }

        let vault_token = match token_address {
            Some(addr) => {
                Self::assert_token_whitelisted(&env, &addr);
                addr
            }
            None => Self::load_token(&env),
        };

        let vault_id = Self::vault_count(env.clone()) + 1;
        let timestamp = env.ledger().timestamp();
        let metadata = String::from_str(&env, "");
        let new_vault = Vault {
            owner: caller.clone(),
            beneficiary: new_beneficiary.clone(),
            balance: 0,
            check_in_interval,
            last_check_in: timestamp,
            created_at: timestamp,
            status: ReleaseStatus::Locked,
            beneficiaries: Vec::new(&env),
            metadata,
            token_address: vault_token,
            custom_metadata: Bytes::new(&env),
            is_paused: false,
            release_condition: ReleaseCondition::OnExpiry,
            parent_vault_id: Some(parent_vault_id),
            passkey_hash: None,
            max_deposit_amount: None,
            withdrawal_approval_threshold: None,
        };
        
        Self::save_vault(&env, vault_id, &new_vault);
        Self::add_owner_vault_id(&env, &caller, vault_id, check_in_interval);
        Self::add_beneficiary_vault_id(&env, &new_beneficiary, vault_id, check_in_interval);
        
        let key = DataKey::VaultCount;
        env.storage().persistent().set(&key, &vault_id);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        
        env.events().publish(
            (INHERITANCE_TOPIC,),
            (parent_vault_id, vault_id, caller, new_beneficiary, check_in_interval),
        );
        vault_id
    }

    /// Gets the parent vault ID for an inherited vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// `Some(parent_vault_id)` if this is an inherited vault, `None` otherwise
    pub fn get_parent_vault(env: Env, vault_id: u64) -> Option<u64> {
        if let Some(vault) = Self::try_load_vault(&env, vault_id) {
            vault.parent_vault_id
        } else {
            None
        }
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
            .unwrap_or_else(|| panic_with_error!(env, ContractError::NotInitialized))
    }

    fn require_initialized(env: &Env) {
        if env.storage().instance().get::<DataKey, Address>(&DataKey::Admin).is_none() {
            panic_with_error!(env, ContractError::NotInitialized);
        }
    }

    fn load_token(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::TokenAddress).unwrap_or_else(|| panic_with_error!(env, ContractError::NotInitialized))
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

    fn save_owner_vault_ids(env: &Env, owner: &Address, vault_ids: &Vec<u64>, check_in_interval: u64) {
        let key = DataKey::OwnerVaults(owner.clone());
        let ttl = vault_ttl_ledgers(check_in_interval);
        env.storage().persistent().set(&key, vault_ids);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
    }

    fn add_owner_vault_id(env: &Env, owner: &Address, vault_id: u64, check_in_interval: u64) {
        let mut vault_ids = Self::load_owner_vault_ids(env, owner);
        vault_ids.push_back(vault_id);
        Self::save_owner_vault_ids(env, owner, &vault_ids, check_in_interval);
    }

    fn remove_owner_vault_id(env: &Env, owner: &Address, vault_id: u64, check_in_interval: u64) {
        let vault_ids = Self::load_owner_vault_ids(env, owner);
        let mut next_ids = Vec::new(env);
        for id in vault_ids.iter() {
            if id != vault_id {
                next_ids.push_back(id);
            }
        }
        // Save updated list or delete key if empty to save storage rent
        if next_ids.is_empty() {
            let key = DataKey::OwnerVaults(owner.clone());
            env.storage().persistent().remove(&key);
        } else {
            Self::save_owner_vault_ids(env, owner, &next_ids, check_in_interval);
        }
    }

    /// Persists a vault to storage with TTL derived from its check_in_interval.
    /// This ensures that when update_check_in_interval modifies the interval,
    /// the persistent storage TTL is automatically updated (issue #297).
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

    fn save_beneficiary_vault_ids(env: &Env, beneficiary: &Address, vault_ids: &Vec<u64>, check_in_interval: u64) {
        let key = DataKey::BeneficiaryVaults(beneficiary.clone());
        let ttl = vault_ttl_ledgers(check_in_interval);
        env.storage().persistent().set(&key, vault_ids);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
    }

    fn add_beneficiary_vault_id(env: &Env, beneficiary: &Address, vault_id: u64, check_in_interval: u64) {
        let mut vault_ids = Self::load_beneficiary_vault_ids(env, beneficiary);
        vault_ids.push_back(vault_id);
        Self::save_beneficiary_vault_ids(env, beneficiary, &vault_ids, check_in_interval);
    }

    fn remove_beneficiary_vault_id(env: &Env, beneficiary: &Address, vault_id: u64, check_in_interval: u64) {
        let vault_ids = Self::load_beneficiary_vault_ids(env, beneficiary);
        let mut next_ids = Vec::new(env);
        for id in vault_ids.iter() {
            if id != vault_id {
                next_ids.push_back(id);
            }
        }
        // Save updated list or delete key if empty to save storage rent
        if next_ids.is_empty() {
            let key = DataKey::BeneficiaryVaults(beneficiary.clone());
            env.storage().persistent().remove(&key);
        } else {
            Self::save_beneficiary_vault_ids(env, beneficiary, &next_ids, check_in_interval);
        }
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

    fn assert_metadata_len(env: &Env, metadata: &String) {
        if metadata.len() > MAX_METADATA_LEN {
            panic_with_error!(env, ContractError::InvalidAmount);
        }
    }

    fn assert_token_whitelisted(env: &Env, token_address: &Address) {
        let default_token = Self::load_token(env);
        if token_address == &default_token {
            return;
        }
        
        let key = DataKey::TokenWhitelist(token_address.clone());
        let is_whitelisted: bool = env.storage().persistent().get(&key).unwrap_or(false);
        if !is_whitelisted {
            panic_with_error!(env, ContractError::NotOwner); // Reusing error code for simplicity
        }
    }

    // --- Issue #395: Passkey Usage Analytics ---

    /// Logs a passkey usage entry for a vault check-in
    fn log_passkey_usage(env: &Env, vault_id: u64, passkey_hash: &BytesN<32>, timestamp: u64) {
        let mut usage: Vec<PasskeyUsageEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::PasskeyUsage(vault_id))
            .unwrap_or(Vec::new(env));
        
        usage.push_back(PasskeyUsageEntry {
            passkey_hash: passkey_hash.clone(),
            timestamp,
        });
        
        let key = DataKey::PasskeyUsage(vault_id);
        env.storage().persistent().set(&key, &usage);
        let ttl = vault_ttl_ledgers(Self::load_vault(env, vault_id).check_in_interval);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.events().publish((PASSKEY_USAGE_TOPIC, vault_id), (passkey_hash.clone(), timestamp));
    }

    // --- Issue #383: Vault Recovery Mode ---

    /// Sets a recovery contact who can extend the vault TTL if the owner loses access.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `caller` - The vault owner (must authorize)
    /// * `contact` - The recovery contact address
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn set_recovery_contact(
        env: Env,
        vault_id: u64,
        caller: Address,
        contact: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        vault.recovery_contact = Some(contact.clone());
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((SET_RECOVERY_TOPIC, vault_id), contact);
        Ok(())
    }

    /// Requests a recovery extension. Only the recovery contact can call this.
    /// Extends the vault TTL by 30 days.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `caller` - The recovery contact (must authorize)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::NotRecoveryContact` - If caller is not the recovery contact
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn request_recovery_extension(
        env: Env,
        vault_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        if let Some(contact) = &vault.recovery_contact {
            if caller != *contact {
                return Err(ContractError::NotRecoveryContact);
            }
        } else {
            return Err(ContractError::NotRecoveryContact);
        }
        vault.last_check_in = env.ledger().timestamp();
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((RECOVERY_EXTEND_TOPIC, vault_id), caller);
        Ok(())
    }

    // --- Issue #384: Vault Activity Audit Log ---

    /// Logs an audit entry for a vault operation.
    fn log_audit_entry(
        env: &Env,
        vault_id: u64,
        operation: &str,
        actor: &Address,
        details: &str,
    ) {
        let mut log: Vec<AuditEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::VaultAuditLog(vault_id))
            .unwrap_or(Vec::new(env));
        
        let entry = AuditEntry {
            timestamp: env.ledger().timestamp(),
            operation: String::from_str(env, operation),
            actor: actor.clone(),
            details: String::from_str(env, details),
        };
        log.push_back(entry);
        
        let key = DataKey::VaultAuditLog(vault_id);
        env.storage().persistent().set(&key, &log);
        let ttl = vault_ttl_ledgers(Self::load_vault(env, vault_id).check_in_interval);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
    }

    /// Retrieves the audit log for a vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    ///
    /// # Returns
    /// A vector of audit entries
    pub fn get_vault_audit_log(env: Env, vault_id: u64) -> Vec<AuditEntry> {
        env.storage()
            .persistent()
            .get(&DataKey::VaultAuditLog(vault_id))
            .unwrap_or(Vec::new(&env))
    }

    // --- Issue #385: Vault Cloning ---

    /// Clones a vault with a new beneficiary.
    ///
    /// Creates a new vault with the same settings as the original, but with a different beneficiary.
    /// Only the owner can clone their vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The original vault ID
    /// * `caller` - The vault owner (must authorize)
    /// * `new_beneficiary` - The beneficiary for the cloned vault
    ///
    /// # Returns
    /// The ID of the newly cloned vault
    ///
    /// # Errors (panics)
    /// * Panics if caller is not the vault owner
    /// * Panics if vault is not in Locked status
    /// * Panics if owner == new_beneficiary
    pub fn clone_vault(
        env: Env,
        vault_id: u64,
        caller: Address,
        new_beneficiary: Address,
    ) -> u64 {
        caller.require_auth();
        let original = Self::load_vault(&env, vault_id);
        if caller != original.owner {
            panic_with_error!(&env, ContractError::NotOwner);
        }
        if original.status != ReleaseStatus::Locked {
            panic_with_error!(&env, ContractError::AlreadyReleased);
        }
        if original.owner == new_beneficiary {
            panic_with_error!(&env, ContractError::InvalidBeneficiary);
        }

        let new_vault_id = Self::vault_count(env.clone()) + 1;
        let timestamp = env.ledger().timestamp();
        let cloned_vault = Vault {
            owner: original.owner.clone(),
            beneficiary: new_beneficiary.clone(),
            balance: 0,
            check_in_interval: original.check_in_interval,
            last_check_in: timestamp,
            created_at: timestamp,
            status: ReleaseStatus::Locked,
            beneficiaries: original.beneficiaries.clone(),
            metadata: original.metadata.clone(),
            token_address: original.token_address.clone(),
            recovery_contact: None,
        };
        Self::save_vault(&env, new_vault_id, &cloned_vault);
        Self::add_owner_vault_id(&env, &original.owner, new_vault_id, original.check_in_interval);
        Self::add_beneficiary_vault_id(&env, &new_beneficiary, new_vault_id, original.check_in_interval);
        
        let key = DataKey::VaultCount;
        env.storage().persistent().set(&key, &new_vault_id);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        
        env.events().publish((VAULT_CLONED_TOPIC,), (vault_id, new_vault_id, new_beneficiary));
        new_vault_id
    }

    // --- Issue #386: Vault Expiry Notification Events ---

    /// Emits expiry warning events for vaults with TTL < 7 days.
    ///
    /// Anyone can call this function to emit warnings for vaults approaching expiry.
    /// This enables off-chain reminder systems to monitor vault status.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_ids` - Vector of vault IDs to check
    pub fn emit_expiry_warnings(env: Env, vault_ids: Vec<u64>) {
        let warning_threshold: u64 = 604_800; // 7 days in seconds
        
        for vault_id in vault_ids.iter() {
            if let Some(vault) = Self::try_load_vault(&env, vault_id) {
                if vault.status != ReleaseStatus::Locked {
                    continue;
                }
                if let Some(ttl) = Self::get_ttl_remaining(env.clone(), vault_id) {
                    if ttl < warning_threshold {
                        env.events().publish((PING_EXPIRY_TOPIC, vault_id), ttl);
                    }
                }
            }
        }
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
    }

    // --- Issue #392: Passkey Rotation ---

    /// Rotates the primary passkey for a vault.
    ///
    /// Verifies the old passkey before accepting the new one. Only the vault owner can call this.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `caller` - The vault owner (must authorize)
    /// * `old_passkey_hash` - Hash of the old passkey (for verification)
    /// * `new_passkey_hash` - Hash of the new passkey
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::InvalidPasskey` - If old passkey doesn't match
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn rotate_passkey(
        env: Env,
        vault_id: u64,
        caller: Address,
        old_passkey_hash: BytesN<32>,
        new_passkey_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        
        // Verify old passkey matches
        if let Some(current) = &vault.passkey_hash {
            if current != &old_passkey_hash {
                return Err(ContractError::InvalidPasskey);
            }
        } else {
            return Err(ContractError::InvalidPasskey);
        }
        
        vault.passkey_hash = Some(new_passkey_hash.clone());
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((ROTATE_PASSKEY_TOPIC, vault_id), (old_passkey_hash, new_passkey_hash));
        Ok(())
    }

    // --- Issue #393: Passkey Backup Codes ---

    /// Generates 10 one-time backup codes for a vault.
    ///
    /// Only the vault owner can call this. Codes are generated deterministically from vault_id and timestamp.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `caller` - The vault owner (must authorize)
    ///
    /// # Returns
    /// Vector of 10 backup codes
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn generate_backup_codes(
        env: Env,
        vault_id: u64,
        caller: Address,
    ) -> Result<Vec<String>, ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        
        let mut codes: Vec<BackupCode> = Vec::new(&env);
        let mut result: Vec<String> = Vec::new(&env);
        let timestamp = env.ledger().timestamp();
        
        for i in 0..10 {
            let code_str = format!("{:016x}", vault_id.wrapping_mul(timestamp).wrapping_add(i as u64));
            let code = String::from_str(&env, &code_str);
            codes.push_back(BackupCode {
                code: code.clone(),
                used: false,
            });
            result.push_back(code);
        }
        
        let key = DataKey::BackupCodes(vault_id);
        env.storage().persistent().set(&key, &codes);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((BACKUP_CODES_GENERATED_TOPIC, vault_id), 10u32);
        Ok(result)
    }

    /// Uses a backup code to extend the vault TTL by 30 days.
    ///
    /// Anyone can call this with a valid backup code. The code is marked as used and cannot be reused.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `code` - The backup code to use
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::InvalidBackupCode` - If code is invalid or not found
    /// * `ContractError::BackupCodeAlreadyUsed` - If code has already been used
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn use_backup_code(
        env: Env,
        vault_id: u64,
        code: String,
    ) -> Result<(), ContractError> {
        let mut vault = Self::load_vault(&env, vault_id);
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        
        let key = DataKey::BackupCodes(vault_id);
        let mut codes: Vec<BackupCode> = env.storage().persistent().get(&key)
            .ok_or(ContractError::InvalidBackupCode)?;
        
        let mut found = false;
        for i in 0..codes.len() {
            if let Some(mut backup_code) = codes.get(i) {
                if backup_code.code == code {
                    if backup_code.used {
                        return Err(ContractError::BackupCodeAlreadyUsed);
                    }
                    backup_code.used = true;
                    codes.set(i, backup_code);
                    found = true;
                    break;
                }
            }
        }
        
        if !found {
            return Err(ContractError::InvalidBackupCode);
        }
        
        // Extend TTL by 30 days
        vault.last_check_in = env.ledger().timestamp();
        Self::save_vault(&env, vault_id, &vault);
        env.storage().persistent().set(&key, &codes);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((BACKUP_CODE_USED_TOPIC, vault_id), code);
        Ok(())
    }

    // --- Issue #394: Multi-Passkey Support ---

    /// Adds a new passkey to a vault.
    ///
    /// Only the vault owner can call this. Multiple passkeys allow different devices to authenticate.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `caller` - The vault owner (must authorize)
    /// * `passkey_hash` - Hash of the new passkey
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn add_passkey(
        env: Env,
        vault_id: u64,
        caller: Address,
        passkey_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        
        let key = DataKey::VaultPasskeys(vault_id);
        let mut passkeys: Vec<PasskeyHash> = env.storage().persistent().get(&key)
            .unwrap_or(Vec::new(&env));
        
        let timestamp = env.ledger().timestamp();
        passkeys.push_back(PasskeyHash {
            hash: passkey_hash.clone(),
            added_at: timestamp,
        });
        
        env.storage().persistent().set(&key, &passkeys);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((ADD_PASSKEY_TOPIC, vault_id), passkey_hash);
        Ok(())
    }

    /// Removes a passkey from a vault.
    ///
    /// Only the vault owner can call this. At least one passkey must remain.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `caller` - The vault owner (must authorize)
    /// * `passkey_hash` - Hash of the passkey to remove
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::PasskeyNotFound` - If passkey is not found
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn remove_passkey(
        env: Env,
        vault_id: u64,
        caller: Address,
        passkey_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        
        let key = DataKey::VaultPasskeys(vault_id);
        let passkeys: Vec<PasskeyHash> = env.storage().persistent().get(&key)
            .ok_or(ContractError::PasskeyNotFound)?;
        
        let mut new_passkeys: Vec<PasskeyHash> = Vec::new(&env);
        let mut found = false;
        
        for pk in passkeys.iter() {
            if pk.hash != passkey_hash {
                new_passkeys.push_back(pk);
            } else {
                found = true;
            }
        }
        
        if !found {
            return Err(ContractError::PasskeyNotFound);
        }
        
        env.storage().persistent().set(&key, &new_passkeys);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((REMOVE_PASSKEY_TOPIC, vault_id), passkey_hash);
        Ok(())
    }

    /// Gets all passkeys for a vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// Vector of passkey hashes
    pub fn get_vault_passkeys(env: Env, vault_id: u64) -> Vec<PasskeyHash> {
        let key = DataKey::VaultPasskeys(vault_id);
        env.storage().persistent().get(&key).unwrap_or(Vec::new(&env))
    }

    /// Checks if a passkey is valid for a vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `passkey_hash` - Hash of the passkey to check
    ///
    /// # Returns
    /// `true` if the passkey is valid, `false` otherwise
    pub fn is_valid_passkey(env: Env, vault_id: u64, passkey_hash: BytesN<32>) -> bool {
        let key = DataKey::VaultPasskeys(vault_id);
        if let Some(passkeys) = env.storage().persistent().get::<DataKey, Vec<PasskeyHash>>(&key) {
            for pk in passkeys.iter() {
                if pk.hash == passkey_hash {
                    return true;
                }
            }
        }
        false
    }

    // --- Issue #401: Beneficiary Delegation ---

    /// Delegates beneficiary role to another address.
    /// Only the current beneficiary can call this.
    pub fn delegate_beneficiary_role(env: Env, vault_id: u64, delegate_address: Address) {
        Self::assert_not_paused(&env);
        let vault = Self::load_vault(&env, vault_id);
        vault.beneficiary.require_auth();

        if delegate_address == vault.beneficiary {
            panic_with_error!(&env, ContractError::InvalidBeneficiary);
        }

        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryDelegate(vault_id), &delegate_address);

        env.events().publish(
            (DELEGATE_BENEFICIARY_TOPIC,),
            (vault_id, vault.beneficiary.clone(), delegate_address),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::BeneficiaryDelegate(vault_id),
            VAULT_TTL_THRESHOLD,
            vault_ttl_ledgers(vault.check_in_interval),
        );
    }

    /// Gets the delegated beneficiary for a vault, if any.
    pub fn get_delegated_beneficiary(env: Env, vault_id: u64) -> Option<Address> {
        env.storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::BeneficiaryDelegate(vault_id))
    }

    // --- Issue #402: Withdrawal Scheduling ---

    /// Sets a withdrawal schedule for the vault. Owner-only.
    pub fn set_withdrawal_schedule(
        env: Env,
        vault_id: u64,
        schedule: Vec<(u64, i128)>,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        let vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();

        let mut entries = Vec::new(&env);
        for (timestamp, amount) in schedule.iter() {
            if amount <= &0 {
                return Err(ContractError::InvalidAmount);
            }
            entries.push_back(WithdrawalScheduleEntry {
                timestamp,
                amount,
            });
        }

        env.storage()
            .persistent()
            .set(&DataKey::WithdrawalSchedule(vault_id), &entries);

        env.events().publish(
            (WITHDRAWAL_SCHEDULED_TOPIC,),
            (vault_id, entries.len()),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::WithdrawalSchedule(vault_id),
            VAULT_TTL_THRESHOLD,
            vault_ttl_ledgers(vault.check_in_interval),
        );
        Ok(())
    }

    /// Executes a scheduled withdrawal if conditions are met. Anyone can call.
    pub fn execute_scheduled_withdrawal(env: Env, vault_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        let mut vault = Self::load_vault(&env, vault_id);

        let key = DataKey::WithdrawalSchedule(vault_id);
        let mut schedule = env
            .storage()
            .persistent()
            .get::<DataKey, Vec<WithdrawalScheduleEntry>>(&key)
            .ok_or(ContractError::NoScheduledWithdrawals)?;

        let now = env.ledger().timestamp();
        let mut executed = false;

        for i in 0..schedule.len() {
            let entry = schedule.get(i).unwrap();
            if entry.timestamp <= now && entry.amount > 0 {
                if vault.balance < entry.amount {
                    return Err(ContractError::InsufficientBalance);
                }

                let token_client = token::Client::new(&env, &vault.token_address);
                let beneficiary = Self::get_delegated_beneficiary(env.clone(), vault_id)
                    .unwrap_or(vault.beneficiary.clone());

                token_client.transfer(
                    &env.current_contract_address(),
                    &beneficiary,
                    &entry.amount,
                );

                vault.balance -= entry.amount;
                schedule.set(
                    i,
                    WithdrawalScheduleEntry {
                        timestamp: entry.timestamp,
                        amount: 0,
                    },
                );
                executed = true;

                env.events().publish(
                    (WITHDRAWAL_EXECUTED_TOPIC,),
                    (vault_id, entry.amount),
                );
            }
        }

        if executed {
            Self::save_vault(&env, vault_id, &vault);
            env.storage()
                .persistent()
                .set(&DataKey::WithdrawalSchedule(vault_id), &schedule);
        }

        if executed {
            Ok(())
        } else {
            Err(ContractError::NoScheduledWithdrawals)
        }
    }

    // --- Issue #400: Conditional Acceptance ---

    /// Beneficiary accepts with conditions. Beneficiary-only.
    pub fn accept_with_conditions(
        env: Env,
        vault_id: u64,
        conditions: String,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        let vault = Self::load_vault(&env, vault_id);
        vault.beneficiary.require_auth();

        if conditions.len() == 0 {
            return Err(ContractError::InvalidAmount);
        }

        let entry = ConditionalAcceptanceEntry {
            conditions,
            approved_by_owner: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::ConditionalAcceptance(vault_id), &entry);

        env.events().publish(
            (CONDITIONS_ACCEPTED_TOPIC,),
            (vault_id, vault.beneficiary.clone()),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::ConditionalAcceptance(vault_id),
            VAULT_TTL_THRESHOLD,
            vault_ttl_ledgers(vault.check_in_interval),
        );
        Ok(())
    }

    /// Owner approves conditional acceptance.
    pub fn approve_conditional_acceptance(env: Env, vault_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        let vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();

        let key = DataKey::ConditionalAcceptance(vault_id);
        let mut entry = env
            .storage()
            .persistent()
            .get::<DataKey, ConditionalAcceptanceEntry>(&key)
            .ok_or(ContractError::InvalidBeneficiary)?;

        entry.approved_by_owner = true;
        env.storage().persistent().set(&key, &entry);
        Ok(())
    }

    /// Gets conditional acceptance entry if it exists.
    pub fn get_conditional_acceptance(
        env: Env,
        vault_id: u64,
    ) -> Option<ConditionalAcceptanceEntry> {
        env.storage()
            .persistent()
            .get::<DataKey, ConditionalAcceptanceEntry>(&DataKey::ConditionalAcceptance(vault_id))
    }

    // --- Issue #399: Dispute Resolution ---

    /// Files a dispute. Beneficiary-only.
    pub fn file_dispute(env: Env, vault_id: u64, reason: String) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        let vault = Self::load_vault(&env, vault_id);
        vault.beneficiary.require_auth();

        if reason.len() == 0 {
            return Err(ContractError::InvalidAmount);
        }

        let current_status = env
            .storage()
            .persistent()
            .get::<DataKey, DisputeStatus>(&DataKey::DisputeStatus(vault_id))
            .unwrap_or(DisputeStatus::None);

        if current_status == DisputeStatus::Filed {
            return Err(ContractError::DisputeFiled);
        }

        env.storage()
            .persistent()
            .set(&DataKey::DisputeStatus(vault_id), &DisputeStatus::Filed);

        env.events().publish(
            (DISPUTE_FILED_TOPIC,),
            (vault_id, vault.beneficiary.clone(), reason),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::DisputeStatus(vault_id),
            VAULT_TTL_THRESHOLD,
            vault_ttl_ledgers(vault.check_in_interval),
        );
        Ok(())
    }

    /// Resolves a dispute. Admin-only.
    pub fn resolve_dispute(env: Env, vault_id: u64, resolution: String) -> Result<(), ContractError> {
        Self::require_admin(&env);

        let current_status = env
            .storage()
            .persistent()
            .get::<DataKey, DisputeStatus>(&DataKey::DisputeStatus(vault_id))
            .unwrap_or(DisputeStatus::None);

        if current_status != DisputeStatus::Filed {
            return Err(ContractError::InvalidBeneficiary);
        }

        env.storage()
            .persistent()
            .set(&DataKey::DisputeStatus(vault_id), &DisputeStatus::Resolved);

        env.events().publish(
            (DISPUTE_RESOLVED_TOPIC,),
            (vault_id, resolution),
        );
        Ok(())
    }

    /// Gets the dispute status for a vault.
    pub fn get_dispute_status(env: Env, vault_id: u64) -> DisputeStatus {
        env.storage()
            .persistent()
            .get::<DataKey, DisputeStatus>(&DataKey::DisputeStatus(vault_id))
            .unwrap_or(DisputeStatus::None)
    }
}
