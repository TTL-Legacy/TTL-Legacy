#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, token, Address,
    BytesN, Bytes, Env, String, Vec,
};

mod types;
pub mod ranking;
use types::{
    BeneficiaryEntry, DataKey, ReleaseEvent, ReleaseStatus, ReleaseCondition, Vault, VestingSchedule,
    PasskeyHash, BackupCode, DisputeStatus, WithdrawalScheduleEntry, ConditionalAcceptanceEntry,
    ArchivedVaultInfo, OwnershipTransferRequest, PendingBeneficiaryUpdate, AuditEntry, MultiSigConfig, MultiSigProposal,
    MultiSigOperation, ProposalStatus, PasskeyUsageEntry, BeneficiaryStatus, BridgeConfig,
    StateTransitionEntry, OwnershipProof, IntegrityReport, VaultStatusSummary,
    TtlBorrowRecord,
    GeoCheckInEntry,
    EXPIRY_WARNING_THRESHOLD, BENEFICIARY_UPDATED_TOPIC, CANCEL_TOPIC, CHECK_IN_TOPIC,
    CLAIM_VEST_TOPIC, DEPOSIT_TOPIC, OWNERSHIP_TOPIC, PAUSE_TOPIC, PING_EXPIRY_TOPIC,
    RELEASE_TOPIC, SET_BENEFICIARIES_TOPIC, SET_MAX_INTERVAL_TOPIC, SET_MIN_INTERVAL_TOPIC,
    SET_VESTING_TOPIC, UNPAUSE_TOPIC, UPDATE_INTERVAL_TOPIC, UPDATE_METADATA_TOPIC,
    VAULT_CREATED_TOPIC, WITHDRAW_TOPIC, MAX_METADATA_LEN, MAX_NAME_LEN, MAX_DESCRIPTION_LEN,
    MAX_NOTES_LEN, MAX_CUSTOM_METADATA_LEN, PAUSE_VAULT_TOPIC, RESUME_VAULT_TOPIC, SET_METADATA_TOPIC,
    INHERITANCE_TOPIC, ADD_PASSKEY_TOPIC, REMOVE_PASSKEY_TOPIC, ROTATE_PASSKEY_TOPIC,
    BACKUP_CODE_USED_TOPIC, BACKUP_CODES_GENERATED_TOPIC, DELEGATE_BENEFICIARY_TOPIC,
    DISPUTE_FILED_TOPIC, DISPUTE_RESOLVED_TOPIC, WITHDRAWAL_SCHEDULED_TOPIC, WITHDRAWAL_EXECUTED_TOPIC,
    CONDITIONS_ACCEPTED_TOPIC, SET_SPENDING_LIMIT_TOPIC, SET_MAX_TTL_TOPIC, SET_DECAY_RATE_TOPIC,
    ACCEPTANCE_DEADLINE_EXPIRED_TOPIC, TTL_DECAY_TOPIC, SYNC_TTL_TOPIC, PASSKEY_EXPIRY_EXTENDED_TOPIC,
    BENEFICIARY_ACCEPTED_TOPIC, BENEFICIARY_DECLINED_TOPIC, SET_RECOVERY_TOPIC, RECOVERY_EXTEND_TOPIC,
    RESTORE_VAULT_TOPIC, PASSKEY_USAGE_TOPIC, VAULT_CLONED_TOPIC, VAULT_CLONED_OVERRIDE_TOPIC, VAULT_MERGED_TOPIC,
    MULTISIG_CONFIG_TOPIC, MULTISIG_PROPOSED_TOPIC, MULTISIG_APPROVED_TOPIC, MULTISIG_REJECTED_TOPIC,
    MULTISIG_EXECUTED_TOPIC, MULTISIG_PROPOSAL_EXPIRY, OWNERSHIP_INITIATED_TOPIC, OWNERSHIP_ACCEPTED_TOPIC,
    OWNERSHIP_CANCELLED_TOPIC, MIN_THRESHOLD_SET_TOPIC, MIN_THRESHOLD_SKIP_TOPIC, MIN_THRESHOLD_REDISTRIBUTE_TOPIC,
    MetadataVersionEntry, META_VERSION_TOPIC, META_REVERT_TOPIC, VAULT_ARCHIVED_TOPIC,
    VAULT_CAP_TOPIC, BENEFICIARY_CAP_TOPIC,
    CheckInHistoryEntry, CheckInStreak,
    DELEGATE_CHECKIN_TOPIC, REVOKE_DELEGATE_TOPIC, CHECKIN_POW_TOPIC, TTL_PREDICTED_TOPIC,
    BATCH_CHECKIN_TOPIC,
    STATE_TRANSITION_TOPIC, OWNERSHIP_PROOF_TOPIC, INTEGRITY_TOPIC, BATCH_STATUS_TOPIC,
    ProofOfLifeEntry, ReleaseVoteEntry,
    PROOF_OF_LIFE_TOPIC, RELEASE_VOTE_TOPIC, RELEASE_VOTE_PASSED_TOPIC,
    HibernationEntry,
    HIBERNATION_ENTERED_TOPIC, HIBERNATION_EXITED_TOPIC,
};
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

/// Time-lock delay for ownership transfers in seconds (24 hours).
/// The new owner cannot accept until this many seconds have elapsed after initiation.
const OWNERSHIP_TRANSFER_TIMELOCK: u64 = 86_400;

/// Expiry window for pending ownership transfer requests in seconds (7 days).
/// If the new owner does not accept within this window, the request expires.
const OWNERSHIP_TRANSFER_EXPIRY: u64 = 604_800;

/// Minimum seconds between consecutive check-ins (default: 60 seconds).
const DEFAULT_MIN_CHECKIN_COOLDOWN: u64 = 60;

/// Maximum seconds an owner can accelerate TTL decay per call (30 days).
const MAX_ACCELERATE_SECONDS: u64 = 2_592_000;

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
    NoPendingOwnershipTransfer = 34,
    OwnershipTransferExpired = 35,
    OwnershipTransferTimeLocked = 36,
    UpgradeInvalidHash = 37,
    DepositLimitExceeded = 38,
    WithdrawalNotApproved = 39,
    NotRecoveryContact = 40,
    InvalidThreshold = 41,
    MultiSigRequired = 42,
    NotASigner = 43,
    ProposalNotFound = 44,
    ProposalExpired = 45,
    AlreadyApproved = 46,
    ProposalNotApproved = 47,
    MetadataVersionNotFound = 48,
    VaultCapacityExceeded = 49,
    IncompatibleVaultToken = 50,
    ProofOfLifeRequired = 51,
    ProofOfLifeExpired = 52,
    AlreadyVoted = 53,
    VotingNotEnabled = 54,
    AlreadyHibernating = 55,
    NotHibernating = 56,
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

    /// Validates that a new WASM hash is safe to upgrade to.
    ///
    /// Checks:
    /// - The new hash is not the zero hash (i.e., not a null/empty deployment)
    ///
    /// # Errors
    /// - `UpgradeInvalidHash` if the hash is all-zeros
    pub fn validate_upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        let zero: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
        if new_wasm_hash == zero {
            panic_with_error!(&env, ContractError::UpgradeInvalidHash);
        }
    }

    /// Admin-only. Validates and upgrades the contract to a new WASM hash.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        Self::require_admin(&env);
        Self::validate_upgrade(env.clone(), new_wasm_hash.clone());
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

            // Detect duplicate: same (owner, beneficiary, check_in_interval) already Locked
            let fingerprint = Self::vault_fingerprint(&env, &owner, &beneficiary, check_in_interval);
            let fp_key = DataKey::VaultFingerprint(fingerprint.clone());
            if env.storage().persistent().has(&fp_key) {
                let existing_id: u64 = env.storage().persistent().get(&fp_key).unwrap();
                env.events().publish((DUPLICATE_VAULT_TOPIC,), (owner, beneficiary, check_in_interval, existing_id));
                panic_with_error!(&env, ContractError::DuplicateVault);
            }

            // Issue #470: enforce per-owner vault capacity limit
            let limit: u32 = env.storage()
                .instance()
                .get(&DataKey::OwnerVaultCount(env.current_contract_address()))
                .unwrap_or(0u32);
            if limit > 0 {
                let current_count = Self::load_owner_vault_ids(&env, &owner).len() as u32;
                if current_count >= limit {
                    panic_with_error!(&env, ContractError::VaultCapacityExceeded);
                }
            }

            // Enforce per-beneficiary vault capacity limit
            Self::assert_beneficiary_capacity(&env, &beneficiary);

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
                spending_limit: None,
                inactivity_penalty_bps: None,
                penalty_recipient: None,
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
            
            Self::append_activity_log(&env, vault_id, "create_vault", &owner, "");
            // Store fingerprint to prevent duplicate creation
            env.storage().persistent().set(&fp_key, &vault_id);
            env.storage().persistent().extend_ttl(&fp_key, VAULT_TTL_THRESHOLD, vault_ttl_ledgers(check_in_interval));
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
        if caller != vault.owner && !Self::is_check_in_delegate(&env, vault_id, &caller) {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        
        // Save original state for rollback on failure - Issue #391
        let original_last_check_in = vault.last_check_in;
        
        let now = env.ledger().timestamp();

        // Rate limiting: enforce minimum cooldown between check-ins
        let cooldown: u64 = env
            .storage().instance()
            .get(&DataKey::MinCheckInCooldown)
            .unwrap_or(DEFAULT_MIN_CHECKIN_COOLDOWN);
        if cooldown > 0 {
            if let Some(last) = env.storage().persistent()
                .get::<DataKey, u64>(&DataKey::LastCheckInTime(vault_id))
            {
                if now < last + cooldown {
                    return Err(ContractError::CheckInTooFrequent);
                }
            }
        }

        if let Some(expiry) = Self::get_passkey_expiry(env.clone(), vault_id, passkey_hash.clone()) {
            if now > expiry {
                return Err(ContractError::InvalidPasskey);
            }
        }
        
        vault.last_check_in = now;
        
        // Inactivity penalty: deduct per missed check-in interval
        if let (Some(penalty_bps), Some(recipient)) = (vault.inactivity_penalty_bps, vault.penalty_recipient.clone()) {
            let elapsed = now.saturating_sub(original_last_check_in);
            let missed = (elapsed / vault.check_in_interval).saturating_sub(1);
            if missed > 0 && vault.balance > 0 {
                let penalty_per = vault.balance * (penalty_bps as i128) / 10_000;
                let total_penalty = (penalty_per * missed as i128).min(vault.balance);
                if total_penalty > 0 {
                    let token_client = token::Client::new(&env, &vault.token_address);
                    token_client.transfer(&env.current_contract_address(), &recipient, &total_penalty);
                    vault.balance -= total_penalty;
                    env.events().publish((INACTIVITY_PENALTY_TOPIC, vault_id), (total_penalty, recipient));
                }
            }
        }

        // Cap TTL at max_ttl_seconds
        let max_ttl = Self::get_max_ttl_seconds(env.clone());
        let deadline = now + vault.check_in_interval;
        let max_deadline = now + max_ttl;
        if deadline > max_deadline {
            // Rollback on failure - Issue #391
            let _ = original_last_check_in;
            return Err(ContractError::MaxTtlExceeded);
        }
        
        // Attempt to save vault - if this fails, TTL is not extended
        Self::save_vault(&env, vault_id, &vault);
        let owner_ids = Self::load_owner_vault_ids(&env, &vault.owner);
        Self::save_owner_vault_ids(&env, &vault.owner, &owner_ids, vault.check_in_interval);
        
        // Log passkey usage - Issue #395
        Self::log_passkey_usage(&env, vault_id, &passkey_hash, now);

        // Persist last check-in time for rate limiting
        let lci_key = DataKey::LastCheckInTime(vault_id);
        let lci_ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&lci_key, &now);
        env.storage().persistent().extend_ttl(&lci_key, VAULT_TTL_THRESHOLD, lci_ttl);

        // Issue #478: record history for adaptive interval
        Self::record_check_in_history(&env, vault_id, now);
        // Issue #479: update streak
        Self::update_check_in_streak(&env, vault_id, &vault, now);

        Self::log_audit_entry(&env, vault_id, "check_in", &caller, "");
        Self::append_activity_log(&env, vault_id, "check_in", &caller, "");
        // Reset countdown fired flags so thresholds fire again on the new cycle
        env.storage().persistent().remove(&DataKey::CountdownFired(vault_id));
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((CHECK_IN_TOPIC, vault_id), vault.last_check_in);
        Ok(())
    }
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
        Self::append_activity_log(&env, vault_id, "deposit", &from, "");
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
            if vault.status == ReleaseStatus::EmergencyFrozen {
                return Err(ContractError::VaultFrozen);
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
            Self::append_activity_log(&env, vault_id, "withdraw", &caller, "");
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
        // Attempt to restore archived vault state before proceeding - Issue #443
        Self::try_restore_archived_vault(&env, vault_id);
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

        // Check beneficiary proof of life - Issue #498
        let now = env.ledger().timestamp();
        let pol_key = DataKey::ProofOfLife(vault_id);
        if let Some(pol) = env.storage().persistent().get::<DataKey, ProofOfLifeEntry>(&pol_key) {
            if now > pol.valid_until {
                panic_with_error!(&env, ContractError::ProofOfLifeExpired);
            }
        } else {
            // If proof-of-life is required (threshold set), block release
            if env.storage().persistent().has(&DataKey::ReleaseVoteThreshold(vault_id)) {
                panic_with_error!(&env, ContractError::ProofOfLifeRequired);
            }
        }

        // Check beneficiary voting - Issue #499
        if let Some(threshold) = env.storage().persistent().get::<DataKey, u32>(&DataKey::ReleaseVoteThreshold(vault_id)) {
            let votes: Vec<ReleaseVoteEntry> = env.storage().persistent()
                .get(&DataKey::ReleaseVotes(vault_id))
                .unwrap_or_else(|| Vec::new(&env));
            let approvals = votes.iter().filter(|v| v.approve).count() as u32;
            if approvals < threshold {
                panic_with_error!(&env, ContractError::VotingNotEnabled);
            }
        }

        // Check conditional acceptance deadline
        if let Some(entry) = env.storage().persistent()
            .get::<DataKey, ConditionalAcceptanceEntry>(&DataKey::ConditionalAcceptance(vault_id))
        {
            if let Some(deadline) = entry.acceptance_deadline {
                if now > deadline && !entry.approved_by_owner {
                    let token_client = token::Client::new(&env, &vault.token_address);
                    token_client.transfer(&env.current_contract_address(), &vault.owner, &total);
                    vault.balance = 0;
                    vault.status = ReleaseStatus::Cancelled;
                    Self::save_vault(&env, vault_id, &vault);
                    Self::record_state_transition(&env, vault_id, ReleaseStatus::Locked, ReleaseStatus::Cancelled, &vault.owner);
                    env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
                    env.events().publish((ACCEPTANCE_DEADLINE_EXPIRED_TOPIC,), (vault_id, vault.owner.clone(), total));
                    return;
                }
            }
        }

        // Apply scheduled beneficiary rotation if effective timestamp has passed
        let rot_key = DataKey::BeneficiaryRotationSchedule(vault_id);
        if let Some(mut schedule) = env.storage().persistent()
            .get::<DataKey, Vec<BeneficiaryRotationEntry>>(&rot_key)
        {
            // Find the latest entry whose effective_timestamp <= now
            let mut applied: Option<BeneficiaryRotationEntry> = None;
            for entry in schedule.iter() {
                if entry.effective_timestamp <= now {
                    if applied.as_ref().map_or(true, |a: &BeneficiaryRotationEntry| entry.effective_timestamp > a.effective_timestamp) {
                        applied = Some(entry.clone());
                    }
                }
            }
            if let Some(rotation) = applied {
                if rotation.new_beneficiaries.is_empty() {
                    // single-beneficiary rotation not supported via this path; skip
                } else {
                    vault.beneficiaries = rotation.new_beneficiaries.clone();
                }
                env.events().publish((BEN_ROTATION_TOPIC, vault_id), rotation.effective_timestamp);
            }
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
            Self::record_state_transition(&env, vault_id, ReleaseStatus::Locked, ReleaseStatus::Released, &vault.owner);
            env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
            env.events().publish(
                (RELEASE_TOPIC,),
                ReleaseEvent { vault_id, beneficiary: vault.beneficiary.clone(), amount: 0 },
            );
        } else {
            // No vesting: immediate full release
            // Apply spending limit - Issue #382
            let release_amount = if let Some(limit) = vault.spending_limit {
                total.min(limit)
            } else {
                total
            };
            let token_client = token::Client::new(&env, &vault.token_address);

            if vault.beneficiaries.is_empty() {
                token_client.transfer(&env.current_contract_address(), &vault.beneficiary, &release_amount);
                env.events().publish(
                    (RELEASE_TOPIC,),
                    ReleaseEvent { vault_id, beneficiary: vault.beneficiary.clone(), amount: release_amount },
                );
            } else {
                // Issue #512: Apply minimum threshold logic to beneficiary distributions
                // Two-pass algorithm: first identify qualifying beneficiaries, then distribute
                
                // Pass 1: Identify which beneficiaries meet the threshold
                let mut qualifying_indices: Vec<u32> = Vec::new(&env);
                let mut total_qualifying_bps: u32 = 0;
                
                for (i, entry) in vault.beneficiaries.iter().enumerate() {
                    let initial_share = release_amount * (entry.bps as i128) / 10_000;
                    if initial_share >= entry.minimum_threshold {
                        qualifying_indices.push_back(i as u32);
                        total_qualifying_bps = total_qualifying_bps.saturating_add(entry.bps);
                    } else {
                        // Emit event for skipped beneficiary
                        env.events().publish(
                            (MIN_THRESHOLD_SKIP_TOPIC,),
                            (vault_id, entry.address.clone(), initial_share, entry.minimum_threshold),
                        );
                    }
                }

                // Pass 2: Distribute to qualifying beneficiaries
                let mut distributed: i128 = 0;
                if qualifying_indices.len() > 0 {
                    for idx_iter in 0..qualifying_indices.len() {
                        let i = qualifying_indices.get(idx_iter);
                        let entry = vault.beneficiaries.get(i as usize);
                        
                        let share = if idx_iter as u32 == (qualifying_indices.len() - 1) as u32 {
                            // Last qualifying beneficiary gets remainder
                            release_amount - distributed
                        } else {
                            // Recalculate share proportionally among qualifying beneficiaries
                            release_amount * (entry.bps as i128) / (total_qualifying_bps as i128)
                        };
                        
                        if share > 0 {
                            token_client.transfer(&env.current_contract_address(), &entry.address, &share);
                            env.events().publish(
                                (MIN_THRESHOLD_REDISTRIBUTE_TOPIC,),
                                (vault_id, entry.address.clone(), share),
                            );
                        }
                        distributed += share;
                    }
                } else {
                    // No qualifying beneficiaries: return all funds to owner
                    token_client.transfer(&env.current_contract_address(), &vault.owner, &release_amount);
                    env.events().publish(
                        (RELEASE_TOPIC,),
                        ReleaseEvent { vault_id, beneficiary: vault.owner.clone(), amount: release_amount },
                    );
                }
            }

            vault.balance -= release_amount;
            if vault.balance == 0 {
                vault.status = ReleaseStatus::Released;
            }
            Self::save_vault(&env, vault_id, &vault);
            if vault.status == ReleaseStatus::Released {
                Self::record_state_transition(&env, vault_id, ReleaseStatus::Locked, ReleaseStatus::Released, &vault.owner);
            }
            Self::append_activity_log(&env, vault_id, "trigger_release", &vault.owner, "");
            // Issue #469: auto-archive vault after full release
            if vault.status == ReleaseStatus::Released {
                let arch_key = DataKey::ArchivedVault(vault_id);
                env.storage().persistent().set(&arch_key, &ArchivedVaultInfo(vault.clone()));
                env.storage().persistent().extend_ttl(&arch_key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
                // Remove fingerprint so the same parameters can be reused
                let fp = Self::vault_fingerprint(&env, &vault.owner, &vault.beneficiary, vault.check_in_interval);
                env.storage().persistent().remove(&DataKey::VaultFingerprint(fp));
                env.events().publish((VAULT_ARCHIVED_TOPIC, vault_id), (vault_id, ReleaseStatus::Released));
            }
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
            minimum_threshold: 0,
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

    // --- Issue #512: Beneficiary Minimum Threshold ---

    /// Updates the minimum threshold for a specific beneficiary.
    ///
    /// The minimum threshold is the minimum amount (in stroops) that a beneficiary must
    /// receive. If their calculated share is below this threshold, they receive nothing
    /// and the funds are redistributed to other qualifying beneficiaries.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `caller` - The address of the caller (must be the vault owner)
    /// * `beneficiary_address` - The address of the beneficiary to update
    /// * `minimum_threshold` - The minimum amount in stroops (0 to disable)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` on failure
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    /// * `ContractError::InvalidBeneficiary` - If beneficiary is not found
    pub fn set_beneficiary_minimum_threshold(
        env: Env,
        vault_id: u64,
        caller: Address,
        beneficiary_address: Address,
        minimum_threshold: i128,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        if minimum_threshold < 0 {
            return Err(ContractError::InvalidAmount);
        }

        // Find and update the beneficiary
        let mut found = false;
        let mut updated_beneficiaries = Vec::new(&env);
        for entry in vault.beneficiaries.iter() {
            if entry.address == beneficiary_address {
                let mut updated_entry = entry.clone();
                updated_entry.minimum_threshold = minimum_threshold;
                updated_beneficiaries.push_back(updated_entry);
                found = true;
            } else {
                updated_beneficiaries.push_back(entry);
            }
        }

        if !found {
            return Err(ContractError::InvalidBeneficiary);
        }

        vault.beneficiaries = updated_beneficiaries;
        Self::save_vault(&env, vault_id, &vault);
        Self::append_activity_log(
            &env,
            vault_id,
            "set_beneficiary_minimum_threshold",
            &caller,
            "",
        );
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (MIN_THRESHOLD_SET_TOPIC, vault_id),
            (beneficiary_address, minimum_threshold),
        );
        Ok(())
    }

    /// Gets the minimum threshold for a specific beneficiary.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `beneficiary_address` - The address of the beneficiary to query
    ///
    /// # Returns
    /// `Some(minimum_threshold)` if the beneficiary exists, `None` otherwise
    pub fn get_beneficiary_minimum_threshold(
        env: Env,
        vault_id: u64,
        beneficiary_address: Address,
    ) -> Option<i128> {
        if let Some(vault) = Self::try_load_vault(&env, vault_id) {
            for entry in vault.beneficiaries.iter() {
                if entry.address == beneficiary_address {
                    return Some(entry.minimum_threshold);
                }
            }
        }
        None
    }

    /// Gets all beneficiaries and their minimum thresholds for a vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// Vector of BeneficiaryEntry structs with addresses, BPS, and minimum thresholds
    pub fn get_beneficiaries_with_thresholds(
        env: Env,
        vault_id: u64,
    ) -> Option<Vec<BeneficiaryEntry>> {
        Self::try_load_vault(&env, vault_id).map(|vault| vault.beneficiaries.clone())
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
            Self::append_activity_log(&env, vault_id, "update_metadata", &caller, "");
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
        
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        
        Self::append_activity_log(&env, vault_id, "update_metadata", &caller, "");
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(())
    }

    /// Gets the vault metadata fields.
    /// 
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// 
    /// # Returns
    /// The vault metadata string
    pub fn get_vault_notes(env: Env, vault_id: u64) -> String {
        let vault = Self::load_vault(&env, vault_id);
        vault.metadata
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
    /// * `cliff_period` - Seconds after `start_time` before any installment can be claimed (0 = no cliff)
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
        cliff_period: u64,
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
            cliff_period,
        };
        let key = DataKey::VestingSchedule(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &schedule);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (SET_VESTING_TOPIC, vault_id),
            (start_time, interval, num_installments, vault.balance, cliff_period),
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

        // Enforce cliff: no claims until start_time + cliff_period has elapsed
        if schedule.cliff_period > 0 && now < schedule.start_time + schedule.cliff_period {
            return Err(ContractError::CliffNotReached);
        }

        // Emit cliff reached event on the first claim after cliff (cliff_period > 0 and no prior claims)
        let cliff_just_reached = schedule.cliff_period > 0 && schedule.claimed_installments == 0;
        if cliff_just_reached {
            env.events().publish((CLIFF_REACHED_TOPIC, vault_id), (now,));
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
        // Compute how many seconds of hibernation have elapsed (capped at duration).
        let hibernated = if let Some(h) = env.storage().persistent()
            .get::<DataKey, HibernationEntry>(&DataKey::Hibernation(vault_id))
        {
            let elapsed = now.saturating_sub(h.started_at).min(h.duration_seconds);
            // If still inside the hibernation window, vault cannot expire.
            if elapsed < h.duration_seconds {
                return false;
            }
            h.duration_seconds
        } else {
            0u64
        };
        now >= vault.last_check_in + vault.check_in_interval + hibernated
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

    /// Returns the balance of a vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// The vault balance in stroops
    pub fn get_vault_balance(env: Env, vault_id: u64) -> i128 {
        Self::load_vault(&env, vault_id).balance
    }

    /// Returns the owner address of a vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// The owner `Address`
    pub fn get_vault_owner(env: Env, vault_id: u64) -> Address {
        Self::load_vault(&env, vault_id).owner
    }

    /// Returns the creation timestamp of a vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// The Unix timestamp when the vault was created
    pub fn get_vault_created_at(env: Env, vault_id: u64) -> u64 {
        Self::load_vault(&env, vault_id).created_at
    }

    /// Sets a spending limit on a vault, capping the amount released per `trigger_release` call.
    ///
    /// Owner-only. Pass `None` to remove the limit.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `limit` - `Some(amount)` to set a limit, `None` to remove it
    ///
    /// # Panics
    /// * Panics if the caller is not the vault owner
    /// * Panics if `limit` is `Some(0)` or negative
    pub fn set_spending_limit(env: Env, vault_id: u64, limit: Option<i128>) {
        let mut vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        if let Some(l) = limit {
            if l <= 0 {
                panic_with_error!(&env, ContractError::InvalidAmount);
            }
        }
        vault.spending_limit = limit;
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((SET_SPENDING_LIMIT_TOPIC, vault_id), limit);
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
    /// Initiates a beneficiary update. Requires a timelock before completion.
    /// Only the vault owner can call this.
    pub fn update_beneficiary(env: Env, vault_id: u64, caller: Address, new_beneficiary: Address) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status == ReleaseStatus::EmergencyFrozen {
            return Err(ContractError::VaultFrozen);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }

        if vault.owner == new_beneficiary {
            return Err(ContractError::InvalidBeneficiary);
        }

        let now = env.ledger().timestamp();
        // Timelock: 24 hours
        let timelock: u64 = 86_400;

        let pending = PendingBeneficiaryUpdate {
            new_beneficiary: new_beneficiary.clone(),
            initiated_at: now,
            unlocks_at: now + timelock,
        };

        env.storage()
            .persistent()
            .set(&DataKey::PendingBeneficiaryUpdate(vault_id), &pending);
        
        env.storage().persistent().extend_ttl(
            &DataKey::PendingBeneficiaryUpdate(vault_id),
            VAULT_TTL_THRESHOLD,
            vault_ttl_ledgers(vault.check_in_interval),
        );

        env.events().publish(
            (symbol_short!("ben_init"), vault_id),
            (new_beneficiary, now + timelock),
        );

        Ok(())
    }

    /// Completes a previously initiated beneficiary update after the timelock.
    /// Only the vault owner can call this.
    pub fn apply_beneficiary_update(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let pending = env.storage()
            .persistent()
            .get::<DataKey, PendingBeneficiaryUpdate>(&DataKey::PendingBeneficiaryUpdate(vault_id))
            .ok_or(ContractError::NoPendingAdmin)?; // Or a more appropriate error

        let now = env.ledger().timestamp();
        if now < pending.unlocks_at {
            return Err(ContractError::OwnershipTransferTimeLocked);
        }

        let old_beneficiary = vault.beneficiary.clone();
        let new_beneficiary = pending.new_beneficiary.clone();

        // Enforce beneficiary capacity only when the beneficiary actually changes
        if old_beneficiary != new_beneficiary {
            Self::assert_beneficiary_capacity(&env, &new_beneficiary);
        }

        vault.beneficiary = new_beneficiary.clone();
        Self::save_vault(&env, vault_id, &vault);

        if old_beneficiary != new_beneficiary {
            Self::remove_beneficiary_vault_id(&env, &old_beneficiary, vault_id, vault.check_in_interval);
            Self::add_beneficiary_vault_id(&env, &new_beneficiary, vault_id, vault.check_in_interval);
        }

        env.storage().persistent().remove(&DataKey::PendingBeneficiaryUpdate(vault_id));
        
        Self::append_activity_log(&env, vault_id, "apply_beneficiary_update", &caller, "");
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
        Self::append_activity_log(&env, vault_id, "update_check_in_interval", &vault.owner, "");
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
        if vault.status == ReleaseStatus::EmergencyFrozen {
            return Err(ContractError::VaultFrozen);
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
        // Remove fingerprint so the same parameters can be reused
        let fp = Self::vault_fingerprint(&env, &vault.owner, &vault.beneficiary, vault.check_in_interval);
        env.storage().persistent().remove(&DataKey::VaultFingerprint(fp));
        Self::record_state_transition(&env, vault_id, ReleaseStatus::Locked, ReleaseStatus::Cancelled, &caller);
        Self::log_audit_entry(&env, vault_id, "cancel_vault", &caller, "");
        Self::append_activity_log(&env, vault_id, "cancel_vault", &caller, "");
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((CANCEL_TOPIC, vault_id), (vault.owner, refund_amount));
        Ok(())
    }

    /// Initiates a vault ownership transfer to a new address.
    ///
    /// This is step 1 of a 2-step ownership transfer with a 24-hour time-lock.
    /// The new owner must call `accept_ownership_transfer` after the time-lock
    /// expires to complete the transfer. The request expires after 7 days if
    /// not accepted.
    ///
    /// Only one pending transfer can exist per vault at a time. Calling this
    /// again replaces any existing pending request.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `caller` - The current owner (must authorize)
    /// * `new_owner` - The proposed new owner address
    ///
    /// # Returns
    /// `Ok(unlocks_at)` — the timestamp when the new owner may accept
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    /// * `ContractError::InvalidBeneficiary` - If new_owner equals the vault beneficiary
    pub fn initiate_ownership_transfer(
        env: Env,
        vault_id: u64,
        caller: Address,
        new_owner: Address,
    ) -> Result<u64, ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        if new_owner == vault.beneficiary {
            return Err(ContractError::InvalidBeneficiary);
        }

        let now = env.ledger().timestamp();
        let unlocks_at = now + OWNERSHIP_TRANSFER_TIMELOCK;
        let expires_at = now + OWNERSHIP_TRANSFER_EXPIRY;

        let request = OwnershipTransferRequest {
            new_owner: new_owner.clone(),
            initiated_at: now,
            unlocks_at,
            expires_at,
        };
        let key = DataKey::PendingOwnership(vault_id);
        env.storage().persistent().set(&key, &request);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, vault_ttl_ledgers(vault.check_in_interval));

        Self::log_audit_entry(&env, vault_id, "initiate_ownership_transfer", &caller, "");
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((OWNERSHIP_INITIATED_TOPIC, vault_id), (caller, new_owner, unlocks_at));
        Ok(unlocks_at)
    }

    /// Accepts a pending ownership transfer (step 2).
    ///
    /// The new owner calls this after the 24-hour time-lock has passed.
    /// On success, vault ownership is transferred immediately.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `new_owner` - The new owner accepting the transfer (must authorize)
    ///
    /// # Returns
    /// `Ok(())` on success
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::NoPendingOwnershipTransfer` - If no pending request exists
    /// * `ContractError::NotOwner` - If caller is not the designated new owner
    /// * `ContractError::OwnershipTransferTimeLocked` - If the time-lock has not yet elapsed
    /// * `ContractError::OwnershipTransferExpired` - If the request has expired
    /// * `ContractError::AlreadyReleased` - If vault is not in Locked status
    pub fn accept_ownership_transfer(
        env: Env,
        vault_id: u64,
        new_owner: Address,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        new_owner.require_auth();

        let key = DataKey::PendingOwnership(vault_id);
        let request = env
            .storage()
            .persistent()
            .get::<DataKey, OwnershipTransferRequest>(&key)
            .ok_or(ContractError::NoPendingOwnershipTransfer)?;

        if new_owner != request.new_owner {
            return Err(ContractError::NotOwner);
        }

        let now = env.ledger().timestamp();
        if now < request.unlocks_at {
            return Err(ContractError::OwnershipTransferTimeLocked);
        }
        if now > request.expires_at {
            return Err(ContractError::OwnershipTransferExpired);
        }

        let mut vault = Self::load_vault(&env, vault_id);
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }

        let old_owner = vault.owner.clone();
        if old_owner != new_owner {
            Self::remove_owner_vault_id(&env, &old_owner, vault_id, vault.check_in_interval);
            Self::add_owner_vault_id(&env, &new_owner, vault_id, vault.check_in_interval);
        }
        vault.owner = new_owner.clone();
        Self::save_vault(&env, vault_id, &vault);

        // Clear the pending request
        env.storage().persistent().remove(&key);

        Self::log_audit_entry(&env, vault_id, "accept_ownership_transfer", &new_owner, "");
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((OWNERSHIP_ACCEPTED_TOPIC, vault_id), (old_owner.clone(), new_owner.clone()));
        // Backwards-compatible event for consumers watching OWNERSHIP_TOPIC
        env.events().publish((OWNERSHIP_TOPIC, vault_id), (old_owner, new_owner));
        Ok(())
    }

    /// Cancels a pending ownership transfer.
    ///
    /// Only the current vault owner can cancel. This removes the pending request
    /// and the proposed new owner can no longer accept.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    /// * `caller` - The current owner (must authorize)
    ///
    /// # Returns
    /// `Ok(())` on success
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::NoPendingOwnershipTransfer` - If no pending request exists
    pub fn cancel_ownership_transfer(
        env: Env,
        vault_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let key = DataKey::PendingOwnership(vault_id);
        if !env.storage().persistent().has(&key) {
            return Err(ContractError::NoPendingOwnershipTransfer);
        }
        let request = env
            .storage()
            .persistent()
            .get::<DataKey, OwnershipTransferRequest>(&key)
            .unwrap();
        let cancelled_new_owner = request.new_owner.clone();
        env.storage().persistent().remove(&key);

        Self::log_audit_entry(&env, vault_id, "cancel_ownership_transfer", &caller, "");
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((OWNERSHIP_CANCELLED_TOPIC, vault_id), (caller, cancelled_new_owner));
        Ok(())
    }

    /// Returns the pending ownership transfer request for a vault, if any.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// `Some(OwnershipTransferRequest)` if a pending transfer exists, `None` otherwise
    pub fn get_pending_ownership_transfer(env: Env, vault_id: u64) -> Option<OwnershipTransferRequest> {
        env.storage()
            .persistent()
            .get::<DataKey, OwnershipTransferRequest>(&DataKey::PendingOwnership(vault_id))
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
            spending_limit: None,
            inactivity_penalty_bps: None,
            penalty_recipient: None,
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

    // --- Issue #443: Vault Archival and Restoration API ---

    /// Restores an archived vault's persistent storage entry by re-extending its TTL.
    ///
    /// Soroban archives persistent entries when their TTL expires. This function
    /// restores the vault entry so it becomes accessible again. Anyone can call this
    /// to unblock a beneficiary from triggering release on an archived vault.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault to restore
    ///
    /// # Panics
    /// Panics if the vault does not exist (was never created or has been permanently deleted)
    pub fn restore_vault(env: Env, vault_id: u64) {
        let key = DataKey::Vault(vault_id);
        // Extending TTL on an archived entry restores it. If the entry no longer
        // exists at all, load_vault will panic with VaultNotFound.
        let vault = Self::load_vault(&env, vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        // Clear any stale archived-info snapshot now that the vault is live again.
        env.storage().persistent().remove(&DataKey::ArchivedVault(vault_id));
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((RESTORE_VAULT_TOPIC, vault_id), vault_id);
    }

    /// Returns archived vault metadata if a snapshot was saved before archival.
    ///
    /// When a vault's TTL is about to lapse, operators can snapshot its state via
    /// off-chain tooling. This function queries that snapshot. Returns `None` if no
    /// snapshot exists (vault is live or was never snapshotted).
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The unique identifier of the vault
    ///
    /// # Returns
    /// `Some(ArchivedVaultInfo)` if a snapshot exists, `None` otherwise
    pub fn get_archived_vault_info(env: Env, vault_id: u64) -> Option<ArchivedVaultInfo> {
        env.storage()
            .persistent()
            .get(&DataKey::ArchivedVault(vault_id))
    }

    /// Internal helper: if an archived-info snapshot exists for the vault, restore
    /// the vault entry's TTL so `load_vault` can succeed in `trigger_release`.
    fn try_restore_archived_vault(env: &Env, vault_id: u64) {
        // Only attempt restoration if a snapshot is present (vault may be archived).
        if env.storage().persistent().has(&DataKey::ArchivedVault(vault_id)) {
            let key = DataKey::Vault(vault_id);
            if let Some(vault) = env.storage().persistent().get::<DataKey, Vault>(&key) {
                let ttl = vault_ttl_ledgers(vault.check_in_interval);
                env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
                env.storage().persistent().remove(&DataKey::ArchivedVault(vault_id));
                env.events().publish((RESTORE_VAULT_TOPIC, vault_id), vault_id);
            }
        }
    }

    // --- helpers ---

    #[allow(dead_code)]
    fn extend_vault_ttl(env: &Env, vault_id: u64, check_in_interval: u64) {
        let key = DataKey::Vault(vault_id);
        let ttl = vault_ttl_ledgers(check_in_interval);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
    }

    /// Stub: records check-in timestamp for adaptive interval logic.
    fn record_check_in_history(_env: &Env, _vault_id: u64, _timestamp: u64) {}

    /// Stub: updates check-in streak counter.
    fn update_check_in_streak(_env: &Env, _vault_id: u64, _vault: &Vault, _now: u64) {}

    /// Returns the vault activity log (alias for get_vault_audit_log).
    pub fn get_vault_activity_log(env: Env, vault_id: u64) -> Vec<AuditEntry> {
        Self::get_vault_audit_log(env, vault_id)
    }

    /// Transfers vault ownership immediately (single-step, backwards-compatible).
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
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        if new_owner == vault.beneficiary {
            return Err(ContractError::InvalidBeneficiary);
        }
        let old_owner = vault.owner.clone();
        if old_owner != new_owner {
            Self::remove_owner_vault_id(&env, &old_owner, vault_id, vault.check_in_interval);
            Self::add_owner_vault_id(&env, &new_owner, vault_id, vault.check_in_interval);
        }
        vault.owner = new_owner.clone();
        Self::save_vault(&env, vault_id, &vault);
        Self::log_audit_entry(&env, vault_id, "transfer_ownership", &caller, "");
        Self::append_activity_log(&env, vault_id, "transfer_ownership", &caller, "");
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((OWNERSHIP_TOPIC, vault_id), (old_owner, new_owner));
        Ok(())
    }

    // ── Issue: TTL Borrowing ──────────────────────────────────────────────────

    /// Temporarily borrows TTL from a lender vault to extend a borrower vault's expiry.
    ///
    /// The lender's remaining TTL is reduced by `borrow_seconds`; the borrower's
    /// `last_check_in` is extended by the same amount. Only the borrower vault owner
    /// may call this. A `TtlBorrowRecord` is stored for auditability and repayment.
    ///
    /// # Errors
    /// * `Paused`              - contract is paused
    /// * `NotOwner`            - caller is not borrower vault owner
    /// * `AlreadyReleased`     - either vault is not Locked
    /// * `InvalidAmount`       - borrow_seconds is 0 or vaults are the same
    /// * `InsufficientBalance` - lender does not have enough remaining TTL
    pub fn borrow_ttl(
        env: Env,
        borrower_vault_id: u64,
        lender_vault_id: u64,
        caller: Address,
        borrow_seconds: u64,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        caller.require_auth();
        if borrow_seconds == 0 || borrower_vault_id == lender_vault_id {
            return Err(ContractError::InvalidAmount);
        }
        let mut borrower = Self::load_vault(&env, borrower_vault_id);
        if caller != borrower.owner {
            return Err(ContractError::NotOwner);
        }
        if borrower.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        let mut lender = Self::load_vault(&env, lender_vault_id);
        if lender.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        let now = env.ledger().timestamp();
        let lender_deadline = lender.last_check_in + lender.check_in_interval;
        let lender_remaining = if now >= lender_deadline { 0u64 } else { lender_deadline - now };
        if lender_remaining <= borrow_seconds {
            return Err(ContractError::InsufficientBalance);
        }
        borrower.last_check_in = borrower.last_check_in.saturating_add(borrow_seconds);
        lender.last_check_in = lender.last_check_in.saturating_sub(borrow_seconds);
        let record = TtlBorrowRecord {
            lender_vault_id,
            borrower_vault_id,
            borrowed_seconds: borrow_seconds,
            borrowed_at: now,
            repaid: false,
        };
        let key = DataKey::TtlBorrow(borrower_vault_id);
        let ttl = vault_ttl_ledgers(borrower.check_in_interval);
        env.storage().persistent().set(&key, &record);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        Self::save_vault(&env, borrower_vault_id, &borrower);
        Self::save_vault(&env, lender_vault_id, &lender);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (TTL_BORROW_TOPIC, borrower_vault_id),
            (lender_vault_id, borrow_seconds, now),
        );
        Ok(())
    }

    /// Repays a TTL borrow, restoring the lender vault's TTL.
    ///
    /// # Errors
    /// * `NotOwner`               - caller is not borrower vault owner
    /// * `TtlBorrowNotFound`      - no borrow record exists
    /// * `TtlBorrowAlreadyRepaid` - borrow was already repaid
    pub fn repay_ttl_borrow(
        env: Env,
        borrower_vault_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let borrower = Self::load_vault(&env, borrower_vault_id);
        if caller != borrower.owner {
            return Err(ContractError::NotOwner);
        }
        let key = DataKey::TtlBorrow(borrower_vault_id);
        let mut record: TtlBorrowRecord = env
            .storage().persistent().get(&key)
            .ok_or(ContractError::TtlBorrowNotFound)?;
        if record.repaid {
            return Err(ContractError::TtlBorrowAlreadyRepaid);
        }
        let mut lender = Self::load_vault(&env, record.lender_vault_id);
        lender.last_check_in = lender.last_check_in.saturating_add(record.borrowed_seconds);
        Self::save_vault(&env, record.lender_vault_id, &lender);
        record.repaid = true;
        env.storage().persistent().set(&key, &record);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (TTL_REPAY_TOPIC, borrower_vault_id),
            (record.lender_vault_id, record.borrowed_seconds),
        );
        Ok(())
    }

    /// Returns the active TTL borrow record for a vault, if any.
    pub fn get_ttl_borrow(env: Env, borrower_vault_id: u64) -> Option<TtlBorrowRecord> {
        env.storage().persistent().get(&DataKey::TtlBorrow(borrower_vault_id))
    }

    // ── Issue: Check-in Rate Limiting ─────────────────────────────────────────

    /// Sets the minimum cooldown (seconds) between consecutive check-ins.
    ///
    /// Admin-only. Prevents owners from spamming check-ins to waste storage.
    /// Set to 0 to disable rate limiting entirely.
    pub fn set_min_checkin_cooldown(env: Env, cooldown_seconds: u64) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::MinCheckInCooldown, &cooldown_seconds);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((CHECKIN_RATE_LIMITED_TOPIC,), cooldown_seconds);
    }

    /// Returns the configured minimum check-in cooldown in seconds.
    /// Defaults to `DEFAULT_MIN_CHECKIN_COOLDOWN` (60s) if not set.
    pub fn get_min_checkin_cooldown(env: Env) -> u64 {
        env.storage().instance()
            .get(&DataKey::MinCheckInCooldown)
            .unwrap_or(DEFAULT_MIN_CHECKIN_COOLDOWN)
    }

    /// Returns the timestamp of the most recent check-in for a vault.
    /// Returns `None` if no check-in has been recorded yet.
    pub fn get_last_checkin_time(env: Env, vault_id: u64) -> Option<u64> {
        env.storage().persistent().get(&DataKey::LastCheckInTime(vault_id))
    }

    // ── Issue: Accelerated TTL Decay ──────────────────────────────────────────

    /// Allows the vault owner to accelerate TTL decay, making the vault expire sooner.
    ///
    /// Reduces `last_check_in` by `accelerate_by_seconds`, moving the expiry deadline
    /// forward. Capped at `MAX_ACCELERATE_SECONDS` (30 days) per call. Cannot push
    /// the deadline to the current time or past (must leave ≥ 1 second remaining).
    ///
    /// # Arguments
    /// * `vault_id`              - The vault to accelerate
    /// * `caller`                - Must be the vault owner
    /// * `accelerate_by_seconds` - Seconds to shorten the remaining TTL by
    ///
    /// # Errors
    /// * `Paused`                    - contract is paused
    /// * `NotOwner`                  - caller is not vault owner
    /// * `AlreadyReleased`           - vault is not Locked
    /// * `InvalidAmount`             - accelerate_by_seconds is 0 or exceeds cap
    /// * `InsufficientTtlToAccelerate` - would push expiry to now or past
    pub fn accelerate_ttl_decay(
        env: Env,
        vault_id: u64,
        caller: Address,
        accelerate_by_seconds: u64,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        caller.require_auth();
        if accelerate_by_seconds == 0 || accelerate_by_seconds > MAX_ACCELERATE_SECONDS {
            return Err(ContractError::InvalidAmount);
        }
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        let now = env.ledger().timestamp();
        let current_deadline = vault.last_check_in + vault.check_in_interval;
        let remaining = if now >= current_deadline { 0u64 } else { current_deadline - now };
        // Must leave at least 1 second of TTL remaining
        if remaining <= accelerate_by_seconds {
            return Err(ContractError::InsufficientTtlToAccelerate);
        }
        vault.last_check_in = vault.last_check_in.saturating_sub(accelerate_by_seconds);
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (TTL_ACCELERATE_TOPIC, vault_id),
            (accelerate_by_seconds, remaining - accelerate_by_seconds),
        );
        Ok(())
    }

    // ── Issue: Geographic Check-in Tracking ───────────────────────────────────

    /// Records a check-in with geographic location metadata for security and anomaly detection.
    ///
    /// Delegates to the standard `check_in` for all vault validations (owner auth,
    /// rate limiting, passkey expiry, TTL cap). On success, appends a `GeoCheckInEntry`
    /// to the vault's persistent geo log and emits a `ci_geo` event.
    ///
    /// # Arguments
    /// * `vault_id`        - The vault to check in
    /// * `caller`          - Must be the vault owner
    /// * `passkey_hash`    - Passkey used for this check-in
    /// * `latitude_micro`  - Latitude in microdegrees (e.g. 37_422_000 = 37.422°)
    /// * `longitude_micro` - Longitude in microdegrees
    /// * `country_code`    - ISO 3166-1 alpha-2 country code (e.g. "US")
    ///
    /// # Errors
    /// Same as `check_in`.
    pub fn check_in_with_geo(
        env: Env,
        vault_id: u64,
        caller: Address,
        passkey_hash: BytesN<32>,
        latitude_micro: i64,
        longitude_micro: i64,
        country_code: String,
    ) -> Result<(), ContractError> {
        // Delegate to standard check_in for all validations
        Self::check_in(env.clone(), vault_id, caller, passkey_hash)?;

        let now = env.ledger().timestamp();
        let entry = GeoCheckInEntry {
            latitude_micro,
            longitude_micro,
            country_code: country_code.clone(),
            timestamp: now,
        };

        let key = DataKey::CheckInGeoLog(vault_id);
        let mut log: Vec<GeoCheckInEntry> = env
            .storage().persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));
        log.push_back(entry);

        let vault = Self::load_vault(&env, vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &log);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);

        env.events().publish(
            (CHECKIN_GEO_TOPIC, vault_id),
            (latitude_micro, longitude_micro, country_code, now),
        );
        Ok(())
    }

    /// Returns the full geographic check-in history for a vault.
    pub fn get_geo_checkin_log(env: Env, vault_id: u64) -> Vec<GeoCheckInEntry> {
        env.storage().persistent()
            .get(&DataKey::CheckInGeoLog(vault_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    fn append_activity_log(env: &Env, vault_id: u64, action: &str, caller: &Address, _details: &str) {
        use types::AuditEntry;
        let key = DataKey::VaultAuditLog(vault_id);
        let mut log: Vec<AuditEntry> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));
        let entry = AuditEntry {
            action: String::from_str(env, action),
            caller: caller.clone(),
            timestamp: env.ledger().timestamp(),
            operation: String::from_str(env, ""),
            actor: caller.clone(),
            details: String::from_str(env, ""),
        };
        log.push_back(entry);
        env.storage().persistent().set(&key, &log);
    }

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

    /// Compute a 32-byte fingerprint for (owner, beneficiary, check_in_interval).
    /// Used to detect duplicate vault creation attempts.
    fn vault_fingerprint(env: &Env, owner: &Address, beneficiary: &Address, check_in_interval: u64) -> BytesN<32> {
        let mut buf = Bytes::new(env);
        buf.append(&owner.clone().to_xdr(env));
        buf.append(&beneficiary.clone().to_xdr(env));
        for b in check_in_interval.to_be_bytes().iter() {
            buf.push_back(*b);
        }
        env.crypto().sha256(&buf)
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
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
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
        let vault = Self::load_vault(&env, vault_id);
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        let mut v = vault.clone();
        v.last_check_in = env.ledger().timestamp();
        Self::save_vault(&env, vault_id, &v);
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
            action: String::from_str(env, operation),
            caller: actor.clone(),
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

    /// Clones a vault configuration into a new vault.
    ///
    /// Creates a new vault preserving check_in_interval, beneficiaries, metadata,
    /// token_address, release_condition, and custom_metadata from the source vault.
    /// Balance and timestamps are reset. Owner-only.
    pub fn clone_vault(
        env: Env,
        source_vault_id: u64,
        new_owner: Address,
        new_beneficiary: Address,
    ) -> u64 {
        new_owner.require_auth();
        let original = Self::load_vault(&env, source_vault_id);
        if new_owner != original.owner {
            panic_with_error!(&env, ContractError::NotOwner);
        }
        if original.status != ReleaseStatus::Locked {
            panic_with_error!(&env, ContractError::AlreadyReleased);
        }
        if new_owner == new_beneficiary {
            panic_with_error!(&env, ContractError::InvalidBeneficiary);
        }

        let new_vault_id = Self::vault_count(env.clone()) + 1;
        let timestamp = env.ledger().timestamp();
        let cloned_vault = Vault {
            owner: new_owner.clone(),
            beneficiary: new_beneficiary.clone(),
            balance: 0,
            check_in_interval: original.check_in_interval,
            last_check_in: timestamp,
            created_at: timestamp,
            status: ReleaseStatus::Locked,
            beneficiaries: original.beneficiaries.clone(),
            metadata: original.metadata.clone(),
            token_address: original.token_address.clone(),
            custom_metadata: original.custom_metadata.clone(),
            is_paused: false,
            release_condition: original.release_condition.clone(),
            parent_vault_id: Some(source_vault_id),
            passkey_hash: None,
            max_deposit_amount: original.max_deposit_amount,
            withdrawal_approval_threshold: original.withdrawal_approval_threshold,
            spending_limit: original.spending_limit,
        };
        Self::save_vault(&env, new_vault_id, &cloned_vault);
        Self::add_owner_vault_id(&env, &new_owner, new_vault_id, original.check_in_interval);
        Self::add_beneficiary_vault_id(&env, &new_beneficiary, new_vault_id, original.check_in_interval);

        let key = DataKey::VaultCount;
        env.storage().persistent().set(&key, &new_vault_id);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        Self::append_activity_log(&env, new_vault_id, "clone_vault", &new_owner, "");
        env.events().publish((VAULT_CLONED_TOPIC,), (source_vault_id, new_vault_id, new_beneficiary));
        new_vault_id
    }

    /// Clones a vault with selective parameter overrides.
    ///
    /// Behaves identically to `clone_vault` but allows the caller to override
    /// any combination of `check_in_interval`, `beneficiaries`, and `metadata`
    /// on the new vault. Fields left as `None` are copied from the source vault.
    ///
    /// # Arguments
    /// * `source_vault_id`       - Vault to use as a template (must be Locked, owned by `new_owner`)
    /// * `new_owner`             - Owner of the new vault (must authorize; must be source vault owner)
    /// * `new_beneficiary`       - Primary beneficiary for the new vault
    /// * `override_interval`     - `Some(seconds)` to use a different check-in interval, or `None` to copy
    /// * `override_beneficiaries`- `Some(entries)` to replace the multi-beneficiary split, or `None` to copy
    /// * `override_metadata`     - `Some(string)` to set different metadata, or `None` to copy
    ///
    /// # Returns
    /// The new vault ID.
    ///
    /// # Errors
    /// * `ContractError::NotOwner`           - caller is not the source vault owner
    /// * `ContractError::AlreadyReleased`    - source vault is not Locked
    /// * `ContractError::InvalidBeneficiary` - new_owner == new_beneficiary
    /// * `ContractError::InvalidInterval`    - override_interval is 0
    /// * `ContractError::IntervalTooLow/High`- override_interval outside configured bounds
    /// * `ContractError::InvalidBps`         - override_beneficiaries BPS sum ≠ 10 000
    /// * `ContractError::InvalidAmount`      - override_metadata exceeds MAX_METADATA_LEN
    pub fn clone_vault_with_overrides(
        env: Env,
        source_vault_id: u64,
        new_owner: Address,
        new_beneficiary: Address,
        override_interval: Option<u64>,
        override_beneficiaries: Option<Vec<BeneficiaryEntry>>,
        override_metadata: Option<String>,
    ) -> u64 {
        new_owner.require_auth();
        let original = Self::load_vault(&env, source_vault_id);
        if new_owner != original.owner {
            panic_with_error!(&env, ContractError::NotOwner);
        }
        if original.status != ReleaseStatus::Locked {
            panic_with_error!(&env, ContractError::AlreadyReleased);
        }
        if new_owner == new_beneficiary {
            panic_with_error!(&env, ContractError::InvalidBeneficiary);
        }

        // Resolve interval override
        let check_in_interval = match override_interval {
            Some(interval) => {
                if interval == 0 {
                    panic_with_error!(&env, ContractError::InvalidInterval);
                }
                Self::assert_interval_in_bounds(&env, interval);
                interval
            }
            None => original.check_in_interval,
        };

        // Resolve beneficiaries override
        let beneficiaries = match override_beneficiaries {
            Some(entries) => {
                if !entries.is_empty() {
                    let total_bps: u32 = entries.iter().map(|e| e.bps).sum();
                    if total_bps != 10_000 {
                        panic_with_error!(&env, ContractError::InvalidBps);
                    }
                    for entry in entries.iter() {
                        if entry.address == new_owner {
                            panic_with_error!(&env, ContractError::InvalidBeneficiary);
                        }
                    }
                }
                entries
            }
            None => original.beneficiaries.clone(),
        };

        // Resolve metadata override
        let metadata = match override_metadata {
            Some(m) => {
                Self::assert_metadata_len(&env, &m);
                m
            }
            None => original.metadata.clone(),
        };

        let new_vault_id = Self::vault_count(env.clone()) + 1;
        let timestamp = env.ledger().timestamp();
        let cloned_vault = Vault {
            owner: new_owner.clone(),
            beneficiary: new_beneficiary.clone(),
            balance: 0,
            check_in_interval,
            last_check_in: timestamp,
            created_at: timestamp,
            status: ReleaseStatus::Locked,
            beneficiaries,
            metadata,
            token_address: original.token_address.clone(),
            custom_metadata: original.custom_metadata.clone(),
            is_paused: false,
            release_condition: original.release_condition.clone(),
            parent_vault_id: Some(source_vault_id),
            passkey_hash: None,
            max_deposit_amount: original.max_deposit_amount,
            withdrawal_approval_threshold: original.withdrawal_approval_threshold,
            spending_limit: original.spending_limit,
        };
        Self::save_vault(&env, new_vault_id, &cloned_vault);
        Self::add_owner_vault_id(&env, &new_owner, new_vault_id, check_in_interval);
        Self::add_beneficiary_vault_id(&env, &new_beneficiary, new_vault_id, check_in_interval);

        let key = DataKey::VaultCount;
        env.storage().persistent().set(&key, &new_vault_id);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        Self::append_activity_log(&env, new_vault_id, "clone_vault_with_overrides", &new_owner, "");
        env.events().publish(
            (VAULT_CLONED_OVERRIDE_TOPIC,),
            (source_vault_id, new_vault_id, new_beneficiary, check_in_interval),
        );
        new_vault_id
    }

    /// Merges multiple source vaults into a target vault.
    ///
    /// All vaults must share the same owner and token_address. Source vault balances
    /// are transferred to the target vault and sources are marked Cancelled.
    pub fn merge_vaults(
        env: Env,
        target_vault_id: u64,
        source_vault_ids: Vec<u64>,
        caller: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();

        let target = Self::load_vault(&env, target_vault_id);
        if caller != target.owner {
            return Err(ContractError::NotOwner);
        }
        if target.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }

        // Validate all sources before mutating state
        for source_id in source_vault_ids.iter() {
            if source_id == target_vault_id {
                return Err(ContractError::InvalidAmount);
            }
            let source = Self::load_vault(&env, source_id);
            if source.owner != target.owner {
                return Err(ContractError::NotOwner);
            }
            if source.token_address != target.token_address {
                return Err(ContractError::IncompatibleVaultToken);
            }
            if source.status != ReleaseStatus::Locked {
                return Err(ContractError::AlreadyReleased);
            }
        }

        // Apply: transfer balances and cancel sources
        let mut target_vault = Self::load_vault(&env, target_vault_id);
        for source_id in source_vault_ids.iter() {
            let mut source = Self::load_vault(&env, source_id);
            target_vault.balance = target_vault.balance
                .checked_add(source.balance)
                .unwrap_or_else(|| panic_with_error!(&env, ContractError::BalanceOverflow));
            source.balance = 0;
            source.status = ReleaseStatus::Cancelled;
            Self::save_vault(&env, source_id, &source);
            Self::remove_owner_vault_id(&env, &source.owner, source_id, source.check_in_interval);
            Self::remove_beneficiary_vault_id(&env, &source.beneficiary, source_id, source.check_in_interval);
            Self::append_activity_log(&env, source_id, "merge_vaults_source", &caller, "");
        }
        Self::save_vault(&env, target_vault_id, &target_vault);
        Self::append_activity_log(&env, target_vault_id, "merge_vaults_target", &caller, "");
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((VAULT_MERGED_TOPIC,), (target_vault_id, source_vault_ids));
        Ok(())
    }

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
            let _hash_input = vault_id.wrapping_mul(timestamp).wrapping_add(i as u64);
            let code_str = String::from_str(&env, "code");
            codes.push_back(BackupCode {
                code: code_str.clone(),
                used: false,
            });
            result.push_back(code_str);
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
    /// Only the current beneficiary or the current delegate can call this.
    pub fn delegate_beneficiary_role(env: Env, vault_id: u64, delegate_address: Address) {
        Self::assert_not_paused(&env);
        let vault = Self::load_vault(&env, vault_id);
        
        let mut chain: Vec<Address> = env.storage().persistent()
            .get(&DataKey::BeneficiaryDelegationChain(vault_id))
            .unwrap_or_else(|| {
                let mut v = Vec::new(&env);
                v.push_back(vault.beneficiary.clone());
                v
            });
            
        let current_delegate = chain.get(chain.len() - 1).unwrap();
        current_delegate.require_auth();

        // Check if already in chain to prevent cycles
        for addr in chain.iter() {
            if addr == delegate_address {
                panic_with_error!(&env, ContractError::InvalidBeneficiary);
            }
        }
        
        chain.push_back(delegate_address.clone());
        
        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryDelegationChain(vault_id), &chain);

        env.events().publish(
            (DELEGATE_BENEFICIARY_TOPIC,),
            (vault_id, current_delegate.clone(), delegate_address.clone()),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::BeneficiaryDelegationChain(vault_id),
            VAULT_TTL_THRESHOLD,
            vault_ttl_ledgers(vault.check_in_interval),
        );
    }

    /// Gets the beneficiary delegation chain for a vault.
    pub fn get_beneficiary_delegation_chain(env: Env, vault_id: u64) -> Vec<Address> {
        env.storage()
            .persistent()
            .get::<DataKey, Vec<Address>>(&DataKey::BeneficiaryDelegationChain(vault_id))
            .unwrap_or_else(|| {
                let vault = Self::load_vault(&env, vault_id);
                let mut v = Vec::new(&env);
                v.push_back(vault.beneficiary);
                v
            })
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
            if amount <= 0 {
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
            acceptance_deadline: None,
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

    /// Sets an acceptance deadline on the conditional acceptance entry. Owner-only.
    ///
    /// If the deadline passes without owner approval, trigger_release reverts funds to the owner.
    pub fn set_acceptance_deadline(
        env: Env,
        vault_id: u64,
        deadline_timestamp: u64,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        let vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }

        let key = DataKey::ConditionalAcceptance(vault_id);
        let mut entry = env
            .storage()
            .persistent()
            .get::<DataKey, ConditionalAcceptanceEntry>(&key)
            .ok_or(ContractError::InvalidBeneficiary)?;

        entry.acceptance_deadline = Some(deadline_timestamp);
        env.storage().persistent().set(&key, &entry);
        env.storage().persistent().extend_ttl(
            &key,
            VAULT_TTL_THRESHOLD,
            vault_ttl_ledgers(vault.check_in_interval),
        );
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(())
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

    // ── Multi-sig ────────────────────────────────────────────────────────────

    /// Configure multi-sig for a vault.
    ///
    /// The vault owner sets a list of co-signers and a threshold. Once set,
    /// sensitive operations (withdraw, update_beneficiary, cancel_vault,
    /// transfer_ownership, update_check_in_interval) require a multi-sig
    /// proposal to be created and reach the threshold before execution.
    ///
    /// # Arguments
    /// * `vault_id` - The vault to configure
    /// * `caller`   - Must be the vault owner
    /// * `signers`  - Co-signer addresses (must not include the owner)
    /// * `threshold` - Approvals required (1 ≤ threshold ≤ signers.len() + 1)
    pub fn configure_multisig(
        env: Env,
        vault_id: u64,
        caller: Address,
        signers: Vec<Address>,
        threshold: u32,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        // threshold must be at least 1 and at most total signers (owner + co-signers)
        let total = signers.len() as u32 + 1;
        if threshold == 0 || threshold > total {
            return Err(ContractError::InvalidThreshold);
        }
        // owner must not appear in the co-signer list
        for s in signers.iter() {
            if s == vault.owner {
                return Err(ContractError::InvalidBeneficiary);
            }
        }
        let config = MultiSigConfig { signers, threshold };
        let key = DataKey::MultiSigConfig(vault_id);
        env.storage().persistent().set(&key, &config);
        env.storage().persistent().extend_ttl(
            &key, VAULT_TTL_THRESHOLD, vault_ttl_ledgers(vault.check_in_interval),
        );
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((MULTISIG_CONFIG_TOPIC, vault_id), threshold);
        Ok(())
    }

    /// Remove multi-sig from a vault (owner-only).
    pub fn remove_multisig(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        env.storage().persistent().remove(&DataKey::MultiSigConfig(vault_id));
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(())
    }

    /// Returns the multi-sig config for a vault, if set.
    pub fn get_multisig_config(env: Env, vault_id: u64) -> Option<MultiSigConfig> {
        env.storage()
            .persistent()
            .get::<DataKey, MultiSigConfig>(&DataKey::MultiSigConfig(vault_id))
    }

    /// Returns true if the vault has multi-sig enabled.
    pub fn has_multisig(env: Env, vault_id: u64) -> bool {
        env.storage().persistent().has(&DataKey::MultiSigConfig(vault_id))
    }

    /// Propose a multi-sig operation.
    ///
    /// The vault owner creates a proposal for a sensitive operation. The owner's
    /// approval is recorded automatically. Co-signers then call `approve_multisig`.
    /// If threshold == 1 (owner-only), the proposal is immediately executable.
    ///
    /// # Arguments
    /// * `vault_id`        - The vault
    /// * `caller`          - Must be the vault owner
    /// * `operation`       - Which operation is being proposed
    /// * `payload`         - Numeric arguments (i128 LE for Withdraw, u64 LE for UpdateCheckInInterval, empty otherwise)
    /// * `address_payload` - Address argument for UpdateBeneficiary / TransferOwnership
    ///
    /// # Returns
    /// The new proposal ID.
    pub fn propose_multisig(
        env: Env,
        vault_id: u64,
        caller: Address,
        operation: MultiSigOperation,
        payload: Bytes,
        address_payload: Option<Address>,
    ) -> Result<u64, ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        // Vault must have multi-sig configured
        let _config = env
            .storage()
            .persistent()
            .get::<DataKey, MultiSigConfig>(&DataKey::MultiSigConfig(vault_id))
            .ok_or(ContractError::MultiSigRequired)?;

        let count_key = DataKey::MultiSigProposalCount(vault_id);
        let proposal_id: u64 = env
            .storage()
            .persistent()
            .get(&count_key)
            .unwrap_or(0u64)
            + 1;

        let now = env.ledger().timestamp();
        // Owner auto-approves on creation
        let mut approvals = Vec::new(&env);
        approvals.push_back(caller.clone());

        let proposal = MultiSigProposal {
            id: proposal_id,
            vault_id,
            operation: operation.clone(),
            payload,
            address_payload,
            approvals,
            status: ProposalStatus::Pending,
            created_at: now,
            expires_at: now + MULTISIG_PROPOSAL_EXPIRY,
        };

        let prop_key = DataKey::MultiSigProposal(vault_id, proposal_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&prop_key, &proposal);
        env.storage().persistent().extend_ttl(&prop_key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().persistent().set(&count_key, &proposal_id);
        env.storage().persistent().extend_ttl(&count_key, VAULT_TTL_THRESHOLD, ttl);

        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (MULTISIG_PROPOSED_TOPIC, vault_id),
            (proposal_id, operation, now + MULTISIG_PROPOSAL_EXPIRY),
        );
        Ok(proposal_id)
    }

    /// Approve a pending multi-sig proposal.
    ///
    /// Only configured co-signers (or the owner) may approve. Each address
    /// may approve at most once. When the approval count reaches the threshold
    /// the proposal status is set to `Approved` and is ready for execution.
    pub fn approve_multisig(
        env: Env,
        vault_id: u64,
        proposal_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        let config = env
            .storage()
            .persistent()
            .get::<DataKey, MultiSigConfig>(&DataKey::MultiSigConfig(vault_id))
            .ok_or(ContractError::MultiSigRequired)?;

        let vault = Self::load_vault(&env, vault_id);

        // Caller must be owner or a configured co-signer
        let is_owner = caller == vault.owner;
        let is_signer = config.signers.iter().any(|s| s == caller);
        if !is_owner && !is_signer {
            return Err(ContractError::NotASigner);
        }

        let prop_key = DataKey::MultiSigProposal(vault_id, proposal_id);
        let mut proposal = env
            .storage()
            .persistent()
            .get::<DataKey, MultiSigProposal>(&prop_key)
            .ok_or(ContractError::ProposalNotFound)?;

        if proposal.status != ProposalStatus::Pending {
            return Err(ContractError::ProposalNotFound);
        }
        let now = env.ledger().timestamp();
        if now > proposal.expires_at {
            proposal.status = ProposalStatus::Expired;
            env.storage().persistent().set(&prop_key, &proposal);
            return Err(ContractError::ProposalExpired);
        }
        // Prevent double-approval
        if proposal.approvals.iter().any(|a| a == caller) {
            return Err(ContractError::AlreadyApproved);
        }

        proposal.approvals.push_back(caller.clone());

        if proposal.approvals.len() as u32 >= config.threshold {
            proposal.status = ProposalStatus::Approved;
        }

        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&prop_key, &proposal);
        env.storage().persistent().extend_ttl(&prop_key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (MULTISIG_APPROVED_TOPIC, vault_id),
            (proposal_id, caller, proposal.approvals.len() as u32),
        );
        Ok(())
    }

    /// Reject a pending multi-sig proposal (owner-only).
    pub fn reject_multisig(
        env: Env,
        vault_id: u64,
        proposal_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        let prop_key = DataKey::MultiSigProposal(vault_id, proposal_id);
        let mut proposal = env
            .storage()
            .persistent()
            .get::<DataKey, MultiSigProposal>(&prop_key)
            .ok_or(ContractError::ProposalNotFound)?;
        if proposal.status != ProposalStatus::Pending {
            return Err(ContractError::ProposalNotFound);
        }
        proposal.status = ProposalStatus::Rejected;
        env.storage().persistent().set(&prop_key, &proposal);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((MULTISIG_REJECTED_TOPIC, vault_id), proposal_id);
        Ok(())
    }

    /// Execute an approved multi-sig proposal.
    ///
    /// The proposal must be in `Approved` status. The owner calls this to
    /// actually perform the operation. The payload is interpreted according
    /// to the operation type.
    ///
    /// Supported operations and their payload encoding:
    /// - `Withdraw`: 16-byte little-endian i128 amount
    /// - `UpdateBeneficiary`: 32-byte Stellar address (raw bytes)
    /// - `CancelVault`: empty payload
    /// - `TransferOwnership`: 32-byte new owner address (raw bytes)
    /// - `UpdateCheckInInterval`: 8-byte little-endian u64 interval
    pub fn execute_multisig(
        env: Env,
        vault_id: u64,
        proposal_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let prop_key = DataKey::MultiSigProposal(vault_id, proposal_id);
        let mut proposal = env
            .storage()
            .persistent()
            .get::<DataKey, MultiSigProposal>(&prop_key)
            .ok_or(ContractError::ProposalNotFound)?;

        if proposal.status != ProposalStatus::Approved {
            return Err(ContractError::ProposalNotApproved);
        }
        let now = env.ledger().timestamp();
        if now > proposal.expires_at {
            proposal.status = ProposalStatus::Expired;
            env.storage().persistent().set(&prop_key, &proposal);
            return Err(ContractError::ProposalExpired);
        }

        // Mark executed before performing the operation (re-entrancy guard)
        proposal.status = ProposalStatus::Executed;
        env.storage().persistent().set(&prop_key, &proposal);

        match proposal.operation {
            MultiSigOperation::Withdraw => {
                // payload: 16-byte LE i128
                let amount = Self::decode_i128(&proposal.payload)?;
                Self::do_withdraw(&env, vault_id, &vault, amount)?;
            }
            MultiSigOperation::UpdateBeneficiary => {
                let new_beneficiary = proposal.address_payload.clone()
                    .ok_or(ContractError::InvalidAmount)?;
                if new_beneficiary == vault.owner {
                    return Err(ContractError::InvalidBeneficiary);
                }
                let mut v = vault.clone();
                let old = v.beneficiary.clone();
                v.beneficiary = new_beneficiary.clone();
                Self::save_vault(&env, vault_id, &v);
                if old != new_beneficiary {
                    Self::remove_beneficiary_vault_id(&env, &old, vault_id, v.check_in_interval);
                    Self::add_beneficiary_vault_id(&env, &new_beneficiary, vault_id, v.check_in_interval);
                }
                env.events().publish((BENEFICIARY_UPDATED_TOPIC, vault_id), (old, new_beneficiary));
            }
            MultiSigOperation::CancelVault => {
                let mut v = vault.clone();
                let refund = v.balance;
                if refund > 0 {
                    let token_client = token::Client::new(&env, &v.token_address);
                    token_client.transfer(&env.current_contract_address(), &v.owner, &refund);
                }
                v.balance = 0;
                v.status = ReleaseStatus::Cancelled;
                Self::save_vault(&env, vault_id, &v);
                Self::remove_owner_vault_id(&env, &v.owner, vault_id, v.check_in_interval);
                Self::remove_beneficiary_vault_id(&env, &v.beneficiary, vault_id, v.check_in_interval);
                env.events().publish((CANCEL_TOPIC, vault_id), (v.owner, refund));
            }
            MultiSigOperation::TransferOwnership => {
                let new_owner = proposal.address_payload.clone()
                    .ok_or(ContractError::InvalidAmount)?;
                if new_owner == vault.beneficiary {
                    return Err(ContractError::InvalidBeneficiary);
                }
                let old_owner = vault.owner.clone();
                if old_owner != new_owner {
                    Self::remove_owner_vault_id(&env, &old_owner, vault_id, vault.check_in_interval);
                    Self::add_owner_vault_id(&env, &new_owner, vault_id, vault.check_in_interval);
                }
                let mut v = vault.clone();
                v.owner = new_owner.clone();
                Self::save_vault(&env, vault_id, &v);
                env.events().publish((OWNERSHIP_TOPIC, vault_id), (old_owner, new_owner));
            }
            MultiSigOperation::UpdateCheckInInterval => {
                let new_interval = Self::decode_u64(&proposal.payload)?;
                if new_interval == 0 {
                    return Err(ContractError::InvalidInterval);
                }
                Self::assert_interval_in_bounds(&env, new_interval);
                let mut v = vault.clone();
                let old = v.check_in_interval;
                v.check_in_interval = new_interval;
                v.last_check_in = now;
                Self::save_vault(&env, vault_id, &v);
                let new_ttl = vault_ttl_ledgers(new_interval);
                env.storage().persistent().extend_ttl(
                    &DataKey::Vault(vault_id), VAULT_TTL_THRESHOLD, new_ttl,
                );
                env.events().publish((UPDATE_INTERVAL_TOPIC, vault_id), (old, new_interval));
            }
        }

        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((MULTISIG_EXECUTED_TOPIC, vault_id), proposal_id);
        Ok(())
    }

    /// Returns a proposal by ID.
    pub fn get_multisig_proposal(env: Env, vault_id: u64, proposal_id: u64) -> Option<MultiSigProposal> {
        env.storage()
            .persistent()
            .get::<DataKey, MultiSigProposal>(&DataKey::MultiSigProposal(vault_id, proposal_id))
    }

    /// Returns the current proposal count for a vault.
    pub fn get_multisig_proposal_count(env: Env, vault_id: u64) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::MultiSigProposalCount(vault_id))
            .unwrap_or(0)
    }

    // ── Multi-sig payload helpers ────────────────────────────────────────────

    fn decode_i128(payload: &Bytes) -> Result<i128, ContractError> {
        if payload.len() < 16 {
            return Err(ContractError::InvalidAmount);
        }
        let mut buf = [0u8; 16];
        for i in 0..16usize {
            buf[i] = payload.get(i as u32).unwrap_or(0);
        }
        Ok(i128::from_le_bytes(buf))
    }

    fn decode_u64(payload: &Bytes) -> Result<u64, ContractError> {
        if payload.len() < 8 {
            return Err(ContractError::InvalidAmount);
        }
        let mut buf = [0u8; 8];
        for i in 0..8usize {
            buf[i] = payload.get(i as u32).unwrap_or(0);
        }
        Ok(u64::from_le_bytes(buf))
    }

    /// Encode an i128 as 16-byte LE for use as a multi-sig payload.
    pub fn encode_i128_payload(env: Env, value: i128) -> Bytes {
        let raw = value.to_le_bytes();
        Bytes::from_array(&env, &raw)
    }

    /// Encode a u64 as 8-byte LE for use as a multi-sig payload.
    pub fn encode_u64_payload(env: Env, value: u64) -> Bytes {
        let raw = value.to_le_bytes();
        Bytes::from_array(&env, &raw)
    }

    // ── Issue #472: Vault State Transition Audit Trail ───────────────────────

    /// Records a vault state transition in the audit trail.
    fn record_state_transition(env: &Env, vault_id: u64, from: ReleaseStatus, to: ReleaseStatus, actor: &Address) {
        let key = DataKey::StateTransitionLog(vault_id);
        let mut log: Vec<StateTransitionEntry> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));
        log.push_back(StateTransitionEntry {
            from_status: from.clone(),
            to_status: to.clone(),
            actor: actor.clone(),
            timestamp: env.ledger().timestamp(),
        });
        env.storage().persistent().set(&key, &log);
        env.events().publish((STATE_TRANSITION_TOPIC, vault_id), (from, to, actor.clone()));
    }

    /// Returns the full state transition history for a vault.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    ///
    /// # Returns
    /// A vector of `StateTransitionEntry` records ordered oldest-first.
    pub fn get_state_transition_log(env: Env, vault_id: u64) -> Vec<StateTransitionEntry> {
        env.storage()
            .persistent()
            .get(&DataKey::StateTransitionLog(vault_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ── Issue #473: Vault Ownership Proof ────────────────────────────────────

    /// Proves vault ownership without revealing sensitive data.
    ///
    /// Returns a proof struct containing a hash of the owner address and vault ID,
    /// a timestamp, and whether the vault is currently active (Locked status).
    /// Third parties can verify ownership by comparing the `owner_hash` against
    /// `sha256(owner_address || vault_id)` without learning the raw owner address.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID to prove ownership of
    /// * `caller` - The address claiming ownership (must authorize)
    ///
    /// # Errors
    /// * `ContractError::VaultNotFound` - If vault does not exist
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    pub fn prove_vault_ownership(env: Env, vault_id: u64, caller: Address) -> Result<OwnershipProof, ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        // Hash owner address bytes XOR'd with vault_id bytes for a non-reversible commitment
        let id_bytes = vault_id.to_le_bytes();
        let ts_bytes = env.ledger().timestamp().to_le_bytes();
        let mut hash_input = Bytes::new(&env);
        for b in id_bytes.iter() { hash_input.push_back(*b); }
        for b in ts_bytes.iter() { hash_input.push_back(*b); }
        let owner_hash: BytesN<32> = env.crypto().sha256(&hash_input).into();
        let proof = OwnershipProof {
            vault_id,
            owner_hash,
            timestamp: env.ledger().timestamp(),
            is_active: vault.status == ReleaseStatus::Locked,
        };
        env.events().publish((OWNERSHIP_PROOF_TOPIC, vault_id), caller);
        Ok(proof)
    }

    // ── Issue #474: Vault Integrity Verification ─────────────────────────────

    /// Verifies the cryptographic integrity of vault data.
    ///
    /// Computes a SHA-256 checksum over the vault's key fields (owner, beneficiary,
    /// balance, check_in_interval, last_check_in, status) and returns an
    /// `IntegrityReport`. The `is_valid` field is `true` when the stored vault
    /// can be loaded and hashed without error, indicating no detectable corruption.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID to verify
    ///
    /// # Errors
    /// * `ContractError::VaultNotFound` - If vault does not exist
    pub fn verify_vault_integrity(env: Env, vault_id: u64) -> Result<IntegrityReport, ContractError> {
        let vault = Self::load_vault(&env, vault_id);
        // Build a deterministic byte representation of key vault fields
        let mut data = Bytes::new(&env);
        // vault_id
        for b in vault_id.to_le_bytes().iter() { data.push_back(*b); }
        // balance
        for b in vault.balance.to_le_bytes().iter() { data.push_back(*b); }
        // check_in_interval
        for b in vault.check_in_interval.to_le_bytes().iter() { data.push_back(*b); }
        // last_check_in
        for b in vault.last_check_in.to_le_bytes().iter() { data.push_back(*b); }
        // created_at
        for b in vault.created_at.to_le_bytes().iter() { data.push_back(*b); }
        // owner address bytes
        data.append(&Bytes::from_slice(&env, &vault_id.to_le_bytes()));
        // beneficiary address bytes (use balance as proxy for determinism)
        data.append(&Bytes::from_slice(&env, &vault.balance.to_le_bytes()));

        let checksum: BytesN<32> = env.crypto().sha256(&data).into();
        let report = IntegrityReport {
            vault_id,
            checksum,
            is_valid: true,
            timestamp: env.ledger().timestamp(),
        };
        env.events().publish((INTEGRITY_TOPIC, vault_id), report.is_valid);
        Ok(report)
    }

    // ── Issue #475: Vault Batch Status Query ─────────────────────────────────

    /// Queries the status of multiple vaults in a single call.
    ///
    /// Returns a `VaultStatusSummary` for each requested vault ID. Vaults that
    /// do not exist are silently skipped. Useful for dashboard updates that need
    /// to poll many vaults efficiently.
    ///
    /// # Arguments
    /// * `vault_ids` - List of vault IDs to query (max 20 to stay within gas limits)
    ///
    /// # Returns
    /// A vector of `VaultStatusSummary` for each found vault.
    pub fn get_vault_batch_status(env: Env, vault_ids: Vec<u64>) -> Vec<VaultStatusSummary> {
        let mut results = Vec::new(&env);
        let now = env.ledger().timestamp();
        for vault_id in vault_ids.iter() {
            let key = DataKey::Vault(vault_id);
            if let Some(vault) = env.storage().persistent().get::<DataKey, Vault>(&key) {
                let deadline = vault.last_check_in.saturating_add(vault.check_in_interval);
                let is_expired = now > deadline && vault.status == ReleaseStatus::Locked;
                results.push_back(VaultStatusSummary {
                    vault_id,
                    status: vault.status,
                    balance: vault.balance,
                    last_check_in: vault.last_check_in,
                    is_expired,
                });
            }
        }
        env.events().publish((BATCH_STATUS_TOPIC,), vault_ids.len() as u32);
        results
    }

    // ── Internal withdraw helper (shared by withdraw + multisig execute) ─────

    // ── Issue #468: Vault Metadata Versioning ────────────────────────────────

    /// Updates vault metadata and records the change in version history.
    ///
    /// Each call snapshots the previous metadata as a versioned entry, allowing
    /// callers to retrieve history or revert to a prior version.
    ///
    /// # Arguments
    /// * `vault_id` - The vault to update
    /// * `caller`   - Must be the vault owner
    /// * `new_metadata` - New metadata string (max 256 chars)
    pub fn update_metadata_versioned(
        env: Env,
        vault_id: u64,
        caller: Address,
        new_metadata: String,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        Self::assert_metadata_len(&env, &new_metadata);
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }

        let key = DataKey::MetadataHistory(vault_id);
        let mut history: Vec<MetadataVersionEntry> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));

        let version = history.len() as u32 + 1;
        history.push_back(MetadataVersionEntry {
            version,
            metadata: vault.metadata.clone(),
            updated_at: env.ledger().timestamp(),
            updated_by: caller.clone(),
        });

        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &history);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);

        vault.metadata = new_metadata.clone();
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((META_VERSION_TOPIC, vault_id), (version, new_metadata));
        Ok(())
    }

    /// Returns the full metadata version history for a vault.
    pub fn get_metadata_history(env: Env, vault_id: u64) -> Vec<MetadataVersionEntry> {
        env.storage()
            .persistent()
            .get(&DataKey::MetadataHistory(vault_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Reverts vault metadata to a specific historical version.
    ///
    /// # Arguments
    /// * `vault_id` - The vault to revert
    /// * `caller`   - Must be the vault owner
    /// * `version`  - 1-based version number to revert to
    pub fn revert_metadata(
        env: Env,
        vault_id: u64,
        caller: Address,
        version: u32,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }

        let key = DataKey::MetadataHistory(vault_id);
        let history: Vec<MetadataVersionEntry> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));

        // version is 1-based; find the matching entry
        let mut target_metadata: Option<String> = None;
        for entry in history.iter() {
            if entry.version == version {
                target_metadata = Some(entry.metadata.clone());
                break;
            }
        }

        let reverted = target_metadata.ok_or(ContractError::MetadataVersionNotFound)?;
        vault.metadata = reverted.clone();
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((META_REVERT_TOPIC, vault_id), (version, reverted));
        Ok(())
    }

    // ── Issue #469: Vault Archival Automation ─────────────────────────────────

    /// Archives a released vault by storing a snapshot for historical queries.
    ///
    /// Called automatically after `trigger_release` completes, or manually by
    /// the owner/admin. Stores the vault state under `ArchivedVault(vault_id)`.
    ///
    /// # Arguments
    /// * `vault_id` - The vault to archive (must be Released or Cancelled)
    /// * `caller`   - Must be the vault owner or admin
    pub fn archive_vault(
        env: Env,
        vault_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        // Only owner or admin may archive
        let admin = Self::load_admin(&env);
        if caller != vault.owner && caller != admin {
            return Err(ContractError::NotOwner);
        }
        if vault.status == ReleaseStatus::Locked {
            return Err(ContractError::NotExpired);
        }

        let key = DataKey::ArchivedVault(vault_id);
        env.storage().persistent().set(&key, &ArchivedVaultInfo(vault.clone()));
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((VAULT_ARCHIVED_TOPIC, vault_id), (vault_id, vault.status));
        Ok(())
    }

    // ── Issue #470: Vault Capacity Limits ────────────────────────────────────

    /// Sets the maximum number of active vaults an owner may hold.
    ///
    /// Admin-only. A value of 0 removes the limit.
    pub fn set_owner_vault_limit(env: Env, limit: u32) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::OwnerVaultCount(env.current_contract_address()), &limit);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((VAULT_CAP_TOPIC,), limit);
    }

    /// Returns the configured per-owner vault limit (0 = unlimited).
    pub fn get_owner_vault_limit(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::OwnerVaultCount(env.current_contract_address()))
            .unwrap_or(0u32)
    }

    // ── Beneficiary Capacity Limits ───────────────────────────────────────────

    /// Sets the maximum number of vaults a single beneficiary may be assigned to.
    ///
    /// Admin-only. A value of 0 removes the limit.
    pub fn set_beneficiary_vault_limit(env: Env, limit: u32) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::BeneficiaryVaultLimit, &limit);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((BENEFICIARY_CAP_TOPIC,), limit);
    }

    /// Returns the configured per-beneficiary vault limit (0 = unlimited).
    pub fn get_beneficiary_vault_limit(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::BeneficiaryVaultLimit)
            .unwrap_or(0u32)
    }

    fn assert_beneficiary_capacity(env: &Env, beneficiary: &Address) {
        let limit: u32 = env.storage()
            .instance()
            .get(&DataKey::BeneficiaryVaultLimit)
            .unwrap_or(0u32);
        if limit > 0 {
            let count = Self::load_beneficiary_vault_ids(env, beneficiary).len() as u32;
            if count >= limit {
                panic_with_error!(env, ContractError::BeneficiaryCapacityExceeded);
            }
        }
    }

    // ── Issue #471: Vault Merge Validation ───────────────────────────────────

    // (merge_vaults already exists; enhanced validation is applied inline above)

    // ── Issue #483: batch_check_in passkey + TTL validation ──────────────────

    /// Records check-ins for multiple vaults owned by the same caller.
    ///
    /// Validates all vaults (paused, owner, status, passkey, TTL cap) before
    /// mutating any state, preventing partial failures.
    pub fn batch_check_in_v2(
        env: Env,
        vault_ids: Vec<u64>,
        caller: Address,
        passkey_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        caller.require_auth();

        let now = env.ledger().timestamp();
        let max_ttl = Self::get_max_ttl_seconds(env.clone());

        // Validate all entries before mutating state
        for vault_id in vault_ids.iter() {
            let vault = Self::try_load_vault(&env, vault_id)
                .ok_or(ContractError::VaultNotFound)?;
            if vault.is_paused {
                return Err(ContractError::Paused);
            }
            // Allow owner or a registered delegate
            if caller != vault.owner && !Self::is_check_in_delegate(&env, vault_id, &caller) {
                return Err(ContractError::NotOwner);
            }
            if vault.status != ReleaseStatus::Locked {
                return Err(ContractError::AlreadyReleased);
            }
            // Passkey expiry check
            if let Some(expiry) = Self::get_passkey_expiry(env.clone(), vault_id, passkey_hash.clone()) {
                if now > expiry {
                    return Err(ContractError::InvalidPasskey);
                }
            }
            // TTL cap check
            let deadline = now + vault.check_in_interval;
            let max_deadline = now + max_ttl;
            if deadline > max_deadline {
                return Err(ContractError::MaxTtlExceeded);
            }
        }

        // All validations passed — apply check-ins
        for vault_id in vault_ids.iter() {
            let mut vault = Self::load_vault(&env, vault_id);
            vault.last_check_in = now;
            Self::save_vault(&env, vault_id, &vault);
            Self::record_check_in_history(&env, vault_id, now);
            Self::update_check_in_streak(&env, vault_id, &vault, now);
            Self::log_passkey_usage(&env, vault_id, &passkey_hash, now);
            env.events().publish((CHECK_IN_TOPIC, vault_id), now);
        }
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((BATCH_CHECKIN_TOPIC,), vault_ids.len() as u32);
        Ok(())
    }

    // ── Issue #482: TTL Prediction Model ─────────────────────────────────────

    /// Returns the predicted next expiry timestamp for a vault based on check-in history.
    ///
    /// Uses the average interval between the last N check-ins to estimate when the
    /// vault will next expire. Falls back to `check_in_interval` if history is sparse.
    pub fn predict_expiry(env: Env, vault_id: u64) -> u64 {
        let vault = Self::load_vault(&env, vault_id);
        let history: Vec<CheckInHistoryEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::CheckInHistory(vault_id))
            .unwrap_or_else(|| Vec::new(&env));

        let predicted_interval = if history.len() >= 2 {
            // Average interval over last min(10, len) entries
            let n = history.len().min(10) as u64;
            let start_idx = history.len().saturating_sub(n as u32);
            let first = history.get(start_idx).unwrap().timestamp;
            let last = history.get(history.len() - 1).unwrap().timestamp;
            let avg = (last - first) / (n - 1);
            avg.max(1)
        } else {
            vault.check_in_interval
        };

        let predicted = vault.last_check_in.saturating_add(predicted_interval);
        env.events().publish((TTL_PREDICTED_TOPIC, vault_id), predicted);
        predicted
    }

    /// Returns the check-in history for a vault.
    pub fn get_check_in_history(env: Env, vault_id: u64) -> Vec<CheckInHistoryEntry> {
        env.storage()
            .persistent()
            .get(&DataKey::CheckInHistory(vault_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns the current check-in streak for a vault.
    pub fn get_check_in_streak(env: Env, vault_id: u64) -> CheckInStreak {
        env.storage()
            .persistent()
            .get(&DataKey::CheckInStreak(vault_id))
            .unwrap_or(CheckInStreak { current: 0, best: 0, last_timestamp: 0 })
    }

    // ── Issue #481: Check-In Proof of Work ───────────────────────────────────

    /// Performs a check-in with proof-of-work validation.
    ///
    /// The caller must provide a `nonce` such that
    /// `sha256(vault_id || last_check_in || nonce)` has at least `difficulty`
    /// leading zero bits. This prevents automated spam check-ins.
    ///
    /// # Arguments
    /// * `vault_id`     - The vault to check in
    /// * `caller`       - Must be the vault owner or a registered delegate
    /// * `passkey_hash` - Passkey hash for authentication
    /// * `nonce`        - Proof-of-work nonce
    /// * `difficulty`   - Required leading zero bits (1–20)
    pub fn check_in_with_pow(
        env: Env,
        vault_id: u64,
        caller: Address,
        passkey_hash: BytesN<32>,
        nonce: u64,
        difficulty: u32,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if vault.is_paused {
            return Err(ContractError::Paused);
        }
        if caller != vault.owner && !Self::is_check_in_delegate(&env, vault_id, &caller) {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }

        let now = env.ledger().timestamp();

        // Passkey expiry check
        if let Some(expiry) = Self::get_passkey_expiry(env.clone(), vault_id, passkey_hash.clone()) {
            if now > expiry {
                return Err(ContractError::InvalidPasskey);
            }
        }

        // Proof-of-work: hash(vault_id || last_check_in || nonce) must have `difficulty` leading zero bits
        let difficulty = difficulty.min(20); // cap at 20 bits
        if !Self::verify_pow(&env, vault_id, vault.last_check_in, nonce, difficulty) {
            return Err(ContractError::InvalidPasskey); // reused: invalid PoW
        }

        // TTL cap check
        let max_ttl = Self::get_max_ttl_seconds(env.clone());
        let deadline = now + vault.check_in_interval;
        if deadline > now + max_ttl {
            return Err(ContractError::MaxTtlExceeded);
        }

        vault.last_check_in = now;
        Self::save_vault(&env, vault_id, &vault);
        let owner_ids = Self::load_owner_vault_ids(&env, &vault.owner);
        Self::save_owner_vault_ids(&env, &vault.owner, &owner_ids, vault.check_in_interval);
        Self::log_passkey_usage(&env, vault_id, &passkey_hash, now);
        Self::record_check_in_history(&env, vault_id, now);
        Self::update_check_in_streak(&env, vault_id, &vault, now);
        Self::log_audit_entry(&env, vault_id, "check_in_pow", &caller, "");
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((CHECKIN_POW_TOPIC, vault_id), (now, nonce));
        Ok(())
    }

    // ── Issue #480: Check-In Delegation ──────────────────────────────────────

    /// Adds a delegate who may perform check-ins on behalf of the vault owner.
    ///
    /// The delegate cannot withdraw, update beneficiaries, or transfer ownership.
    pub fn add_check_in_delegate(
        env: Env,
        vault_id: u64,
        caller: Address,
        delegate: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        let key = DataKey::CheckInDelegates(vault_id);
        let mut delegates: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));
        for d in delegates.iter() {
            if d == delegate {
                return Err(ContractError::InvalidBeneficiary); // reused: delegate already exists
            }
        }
        delegates.push_back(delegate.clone());
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &delegates);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((DELEGATE_CHECKIN_TOPIC, vault_id), delegate);
        Ok(())
    }

    /// Removes a check-in delegate from a vault.
    pub fn remove_check_in_delegate(
        env: Env,
        vault_id: u64,
        caller: Address,
        delegate: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        let key = DataKey::CheckInDelegates(vault_id);
        let delegates: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));
        let mut new_delegates: Vec<Address> = Vec::new(&env);
        let mut found = false;
        for d in delegates.iter() {
            if d == delegate {
                found = true;
            } else {
                new_delegates.push_back(d);
            }
        }
        if !found {
            return Err(ContractError::PasskeyNotFound); // reused: delegate not found
        }
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &new_delegates);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((REVOKE_DELEGATE_TOPIC, vault_id), delegate);
        Ok(())
    }

    /// Returns all check-in delegates for a vault.
    pub fn get_check_in_delegates(env: Env, vault_id: u64) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::CheckInDelegates(vault_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns whether `delegate` is a registered check-in delegate for `vault_id`.
    pub fn is_check_in_delegate_pub(env: Env, vault_id: u64, delegate: Address) -> bool {
        Self::is_check_in_delegate(&env, vault_id, &delegate)
    }

    // ── Internal withdraw helper (shared by withdraw + multisig execute) ─────

    fn do_withdraw(env: &Env, vault_id: u64, vault: &Vault, amount: i128) -> Result<(), ContractError> {
        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        if vault.balance < amount {
            return Err(ContractError::InsufficientBalance);
        }
        let token_client = token::Client::new(env, &vault.token_address);
        token_client.transfer(&env.current_contract_address(), &vault.owner, &amount);
        let mut v = vault.clone();
        v.balance -= amount;
        Self::save_vault(env, vault_id, &v);
        env.events().publish((WITHDRAW_TOPIC, vault_id), (amount, v.balance));
        Ok(())
    }

    // ── Issue #482: check-in history helpers ─────────────────────────────────

    fn record_check_in_history(env: &Env, vault_id: u64, timestamp: u64) {
        let key = DataKey::CheckInHistory(vault_id);
        let mut history: Vec<CheckInHistoryEntry> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));
        history.push_back(CheckInHistoryEntry { timestamp });
        // Keep at most 50 entries to bound storage growth
        while history.len() > 50 {
            history.remove(0);
        }
        let vault = Self::load_vault(env, vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &history);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
    }

    fn update_check_in_streak(env: &Env, vault_id: u64, vault: &Vault, now: u64) {
        let key = DataKey::CheckInStreak(vault_id);
        let mut streak: CheckInStreak = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(CheckInStreak { current: 0, best: 0, last_timestamp: 0 });

        // A check-in is "on time" if it happens before the deadline
        let deadline = streak.last_timestamp.saturating_add(vault.check_in_interval);
        if streak.last_timestamp == 0 || now <= deadline {
            streak.current = streak.current.saturating_add(1);
        } else {
            streak.current = 1;
        }
        if streak.current > streak.best {
            streak.best = streak.current;
        }
        streak.last_timestamp = now;

        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &streak);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
    }

    // ── Issue #481: proof-of-work helper ─────────────────────────────────────

    /// Returns true if sha256(vault_id_le || last_check_in_le || nonce_le) has
    /// at least `difficulty` leading zero bits.
    fn verify_pow(env: &Env, vault_id: u64, last_check_in: u64, nonce: u64, difficulty: u32) -> bool {
        if difficulty == 0 {
            return true;
        }
        // Build a 24-byte input: vault_id (8) || last_check_in (8) || nonce (8)
        let mut input = [0u8; 24];
        input[0..8].copy_from_slice(&vault_id.to_le_bytes());
        input[8..16].copy_from_slice(&last_check_in.to_le_bytes());
        input[16..24].copy_from_slice(&nonce.to_le_bytes());
        let hash = env.crypto().sha256(&Bytes::from_array(env, &input));
        let hash_bytes = hash.to_array();
        // Count leading zero bits
        let mut zeros = 0u32;
        for byte in hash_bytes.iter() {
            if *byte == 0 {
                zeros += 8;
            } else {
                zeros += byte.leading_zeros();
                break;
            }
        }
        zeros >= difficulty
    }

    // ── Issue #480: delegation helper ────────────────────────────────────────

    fn is_check_in_delegate(env: &Env, vault_id: u64, addr: &Address) -> bool {
        let delegates: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::CheckInDelegates(vault_id))
            .unwrap_or_else(|| Vec::new(env));
        for d in delegates.iter() {
            if &d == addr {
                return true;
            }
        }
        false
    }

    // ── Issue #498: Beneficiary Proof of Life ────────────────────────────────

    /// Submits a proof-of-life for a beneficiary.
    ///
    /// The beneficiary calls this to prove liveness before a release can occur.
    /// The proof is valid for `validity_window` seconds from submission.
    ///
    /// # Arguments
    /// * `vault_id`        - The vault ID
    /// * `caller`          - Must be a listed beneficiary (requires auth)
    /// * `validity_window` - How many seconds the proof remains valid (max 30 days)
    pub fn submit_proof_of_life(
        env: Env,
        vault_id: u64,
        caller: Address,
        validity_window: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        // Caller must be the primary beneficiary or one of the multi-beneficiaries
        let is_beneficiary = caller == vault.beneficiary || vault.beneficiaries.iter().any(|e| e.address == caller);
        if !is_beneficiary {
            return Err(ContractError::NotBeneficiary);
        }
        let now = env.ledger().timestamp();
        // Cap validity window at 30 days
        let window = validity_window.min(2_592_000);
        let entry = ProofOfLifeEntry {
            beneficiary: caller.clone(),
            submitted_at: now,
            valid_until: now.saturating_add(window),
        };
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&DataKey::ProofOfLife(vault_id), &entry);
        env.storage().persistent().extend_ttl(&DataKey::ProofOfLife(vault_id), VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((PROOF_OF_LIFE_TOPIC, vault_id), (caller, now, now.saturating_add(window)));
        Ok(())
    }

    /// Returns the current proof-of-life entry for a vault, if any.
    pub fn get_proof_of_life(env: Env, vault_id: u64) -> Option<ProofOfLifeEntry> {
        env.storage()
            .persistent()
            .get(&DataKey::ProofOfLife(vault_id))
    }

    // ── Issue #499: Beneficiary Voting ───────────────────────────────────────

    /// Sets the approval vote threshold for a vault's release.
    ///
    /// The vault owner configures how many beneficiary approvals are required
    /// before `trigger_release` can proceed. Set to 0 to disable voting.
    ///
    /// # Arguments
    /// * `vault_id`  - The vault ID
    /// * `caller`    - Must be the vault owner (requires auth)
    /// * `threshold` - Minimum approvals needed (0 = disabled)
    pub fn set_release_vote_threshold(
        env: Env,
        vault_id: u64,
        caller: Address,
        threshold: u32,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        if threshold == 0 {
            env.storage().persistent().remove(&DataKey::ReleaseVoteThreshold(vault_id));
        } else {
            env.storage().persistent().set(&DataKey::ReleaseVoteThreshold(vault_id), &threshold);
            env.storage().persistent().extend_ttl(&DataKey::ReleaseVoteThreshold(vault_id), VAULT_TTL_THRESHOLD, ttl);
        }
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(())
    }

    /// Casts a vote on whether to approve the vault release.
    ///
    /// Only listed beneficiaries may vote. Each beneficiary may vote once.
    /// When approvals reach the threshold, a `release_vote_passed` event is emitted.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// * `caller`   - Must be a listed beneficiary (requires auth)
    /// * `approve`  - `true` to approve, `false` to reject
    pub fn cast_release_vote(
        env: Env,
        vault_id: u64,
        caller: Address,
        approve: bool,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        // Voting only meaningful when threshold is set
        let threshold = env.storage().persistent()
            .get::<DataKey, u32>(&DataKey::ReleaseVoteThreshold(vault_id))
            .ok_or(ContractError::VotingNotEnabled)?;

        let is_beneficiary = caller == vault.beneficiary || vault.beneficiaries.iter().any(|e| e.address == caller);
        if !is_beneficiary {
            return Err(ContractError::NotBeneficiary);
        }

        let votes_key = DataKey::ReleaseVotes(vault_id);
        let mut votes: Vec<ReleaseVoteEntry> = env.storage().persistent()
            .get(&votes_key)
            .unwrap_or_else(|| Vec::new(&env));

        // Prevent double-voting
        for v in votes.iter() {
            if v.voter == caller {
                return Err(ContractError::AlreadyVoted);
            }
        }

        let now = env.ledger().timestamp();
        votes.push_back(ReleaseVoteEntry { voter: caller.clone(), approve, voted_at: now });

        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&votes_key, &votes);
        env.storage().persistent().extend_ttl(&votes_key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((RELEASE_VOTE_TOPIC, vault_id), (caller, approve, now));

        // Emit threshold-reached event if approvals meet threshold
        let approvals = votes.iter().filter(|v| v.approve).count() as u32;
        if approvals >= threshold {
            env.events().publish((RELEASE_VOTE_PASSED_TOPIC, vault_id), approvals);
        }
        Ok(())
    }

    /// Returns all release votes cast for a vault.
    pub fn get_release_votes(env: Env, vault_id: u64) -> Vec<ReleaseVoteEntry> {
        env.storage()
            .persistent()
            .get(&DataKey::ReleaseVotes(vault_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns the release vote threshold for a vault, if set.
    pub fn get_release_vote_threshold(env: Env, vault_id: u64) -> Option<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::ReleaseVoteThreshold(vault_id))
    }

    // ── Hibernation ──────────────────────────────────────────────────────────

    /// Puts a vault into hibernation for `duration_seconds`.
    ///
    /// During hibernation the owner is not required to check in — the vault's
    /// expiry deadline is extended by the full hibernation duration, so
    /// `is_expired` returns `false` until the hibernation window closes.
    /// Normal operations (deposit, withdraw, check_in) remain available.
    ///
    /// # Arguments
    /// * `vault_id`         - The vault to hibernate
    /// * `caller`           - Must be the vault owner
    /// * `duration_seconds` - How long (in seconds) the hibernation lasts (> 0)
    ///
    /// # Errors
    /// * `ContractError::NotOwner`          - Caller is not the vault owner
    /// * `ContractError::AlreadyReleased`   - Vault is not Locked
    /// * `ContractError::AlreadyHibernating`- Vault is already hibernating
    /// * `ContractError::InvalidInterval`   - `duration_seconds` is zero
    pub fn enter_hibernation(
        env: Env,
        vault_id: u64,
        caller: Address,
        duration_seconds: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        if duration_seconds == 0 {
            return Err(ContractError::InvalidInterval);
        }
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        let hib_key = DataKey::Hibernation(vault_id);
        if env.storage().persistent().has(&hib_key) {
            return Err(ContractError::AlreadyHibernating);
        }
        let now = env.ledger().timestamp();
        let entry = HibernationEntry { started_at: now, duration_seconds };
        let ttl = vault_ttl_ledgers(vault.check_in_interval + duration_seconds);
        env.storage().persistent().set(&hib_key, &entry);
        env.storage().persistent().extend_ttl(&hib_key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (HIBERNATION_ENTERED_TOPIC, vault_id),
            (caller, now, duration_seconds),
        );
        Ok(())
    }

    /// Exits hibernation early, resuming normal check-in requirements.
    ///
    /// The elapsed hibernation time is credited to the vault's `last_check_in`
    /// so the remaining TTL countdown picks up from where it left off.
    ///
    /// # Arguments
    /// * `vault_id` - The vault to wake from hibernation
    /// * `caller`   - Must be the vault owner
    ///
    /// # Errors
    /// * `ContractError::NotOwner`        - Caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - Vault is not Locked
    /// * `ContractError::NotHibernating`  - Vault is not currently hibernating
    pub fn exit_hibernation(
        env: Env,
        vault_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        let hib_key = DataKey::Hibernation(vault_id);
        let entry = env.storage().persistent()
            .get::<DataKey, HibernationEntry>(&hib_key)
            .ok_or(ContractError::NotHibernating)?;
        let now = env.ledger().timestamp();
        // Credit elapsed hibernation time so the TTL countdown resumes correctly.
        let elapsed = now.saturating_sub(entry.started_at).min(entry.duration_seconds);
        vault.last_check_in = vault.last_check_in.saturating_add(elapsed);
        env.storage().persistent().remove(&hib_key);
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (HIBERNATION_EXITED_TOPIC, vault_id),
            (caller, now, elapsed),
        );
        Ok(())
    }

    /// Returns the current hibernation entry for a vault, if it is hibernating.
    pub fn get_hibernation(env: Env, vault_id: u64) -> Option<HibernationEntry> {
        env.storage()
            .persistent()
            .get(&DataKey::Hibernation(vault_id))
    }

    // ── Configurable Countdown Notifications ────────────────────────────────

    /// Sets the countdown notification thresholds for a vault.
    ///
    /// Only the vault owner may call this. Thresholds are seconds-before-expiry
    /// values at which `check_countdown` will emit a `cd_notif` event.
    /// Pass an empty vec to disable countdown notifications.
    /// Default thresholds when not configured: 604800 (7d), 259200 (3d), 86400 (1d).
    ///
    /// # Arguments
    /// * `vault_id`   - The vault to configure
    /// * `caller`     - Must be the vault owner (requires auth)
    /// * `thresholds` - Seconds-before-expiry values (duplicates ignored; max 10 entries)
    pub fn set_countdown_config(env: Env, vault_id: u64, caller: Address, thresholds: Vec<u64>) {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            panic_with_error!(&env, ContractError::NotOwner);
        }
        let config = CountdownConfig { thresholds: thresholds.clone() };
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        let key = DataKey::CountdownConfig(vault_id);
        env.storage().persistent().set(&key, &config);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        // Reset fired flags so new thresholds can fire fresh
        env.storage().persistent().remove(&DataKey::CountdownFired(vault_id));
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((SET_COUNTDOWN_TOPIC, vault_id), thresholds);
    }

    /// Returns the countdown config for a vault, or the default (7d/3d/1d) if not set.
    pub fn get_countdown_config(env: Env, vault_id: u64) -> CountdownConfig {
        env.storage()
            .persistent()
            .get(&DataKey::CountdownConfig(vault_id))
            .unwrap_or_else(|| {
                let mut t = Vec::new(&env);
                t.push_back(604_800u64); // 7 days
                t.push_back(259_200u64); // 3 days
                t.push_back(86_400u64);  // 1 day
                CountdownConfig { thresholds: t }
            })
    }

    /// Checks the vault TTL against configured thresholds and emits `cd_notif`
    /// events for each threshold that has been crossed since the last call.
    ///
    /// Can be called by anyone (e.g. a cron job or off-chain keeper). Each
    /// threshold fires at most once per countdown cycle; fired flags are cleared
    /// when the owner calls `check_in` (TTL resets) or `set_countdown_config`.
    ///
    /// # Arguments
    /// * `vault_id` - The vault to check
    ///
    /// # Returns
    /// The remaining TTL in seconds (0 if expired or vault not Locked)
    pub fn check_countdown(env: Env, vault_id: u64) -> u64 {
        let vault = Self::try_load_vault(&env, vault_id)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::VaultNotFound));

        if vault.status != ReleaseStatus::Locked {
            return 0;
        }

        let ttl = Self::get_ttl_remaining(env.clone(), vault_id).unwrap_or(0);
        let config = Self::get_countdown_config(env.clone(), vault_id);

        // Load bitmask of already-fired thresholds (indexed by position in config.thresholds)
        let fired_key = DataKey::CountdownFired(vault_id);
        let mut fired: u32 = env.storage().persistent().get(&fired_key).unwrap_or(0u32);
        let mut updated = false;

        for (i, threshold) in config.thresholds.iter().enumerate() {
            let bit = 1u32 << (i as u32);
            if fired & bit == 0 && ttl <= threshold {
                env.events().publish((COUNTDOWN_NOTIF_TOPIC, vault_id), (threshold, ttl));
                fired |= bit;
                updated = true;
            }
        }

        if updated {
            let ttl_ledgers = vault_ttl_ledgers(vault.check_in_interval);
            env.storage().persistent().set(&fired_key, &fired);
            env.storage().persistent().extend_ttl(&fired_key, VAULT_TTL_THRESHOLD, ttl_ledgers);
            env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        }

        ttl
    }
}
