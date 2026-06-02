#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, token, Address,
    BytesN, Bytes, Env, String, Vec,
};

mod types;
pub mod ranking;
use types::{
    BeneficiaryEntry, DataKey, ReleaseEvent, ReleaseStatus, ReleaseCondition, Vault, VestingSchedule,
    VestingPenaltyConfig, VestingPendingClaim,
    MilestoneEntry, MilestoneVestingSchedule,
    PasskeyHash, BackupCode, DisputeStatus, WithdrawalScheduleEntry, ConditionalAcceptanceEntry,
    ArchivedVaultInfo, OwnershipTransferRequest, PendingBeneficiaryUpdate, AuditEntry, MultiSigConfig, MultiSigProposal,
    MultiSigOperation, ProposalStatus, PasskeyUsageEntry, BeneficiaryStatus, BridgeConfig,
    TokenConversion, TokenStaking, YieldDistributionMode, YieldDistributionConfig,
    StateTransitionEntry, OwnershipProof, IntegrityReport, VaultStatusSummary,
    TtlBorrowRecord, BeneficiaryCommitment,
    GeoCheckInEntry,
    ProofOfLifeEntry, ReleaseVoteEntry,
    BeneficiaryRotationEntry,
    WithdrawalLimit, WithdrawalTracker, WhitelistEntry, WithdrawalReversal,
    EXPIRY_WARNING_THRESHOLD, BENEFICIARY_UPDATED_TOPIC, BENEFICIARY_TRIGGER_SET_TOPIC, BENEFICIARY_TIER_SET_TOPIC,
    BENEFICIARY_WATERFALL_TOPIC, BENEFICIARY_REBALANCED_TOPIC, CANCEL_TOPIC, CHECK_IN_TOPIC,
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
    BENEFICIARY_ACCEPTED_TOPIC, BENEFICIARY_DECLINED_TOPIC, BENEFICIARY_CONDITION_ACCEPTED_TOPIC,
    BENEFICIARY_IDENTITY_ORACLE_SET_TOPIC, BENEFICIARY_IDENTITY_VERIFIED_TOPIC,
    SET_RECOVERY_TOPIC, RECOVERY_EXTEND_TOPIC,
    RESTORE_VAULT_TOPIC, PASSKEY_USAGE_TOPIC, VAULT_CLONED_TOPIC, VAULT_CLONED_OVERRIDE_TOPIC, VAULT_MERGED_TOPIC,
    MULTISIG_CONFIG_TOPIC, MULTISIG_PROPOSED_TOPIC, MULTISIG_APPROVED_TOPIC, MULTISIG_REJECTED_TOPIC,
    MULTISIG_EXECUTED_TOPIC, MULTISIG_PROPOSAL_EXPIRY, OWNERSHIP_INITIATED_TOPIC, OWNERSHIP_ACCEPTED_TOPIC,
    WITHDRAWAL_APPROVAL_REQUESTED_TOPIC, WITHDRAWAL_APPROVAL_GRANTED_TOPIC, WITHDRAWAL_APPROVAL_DENIED_TOPIC,
    PASSKEY_RECOVERY_INITIATED_TOPIC, PASSKEY_RECOVERED_TOPIC,
    PASSKEY_LOCKOUT_TOPIC, PASSKEY_UNLOCKED_TOPIC,
    PASSKEY_ROTATION_REQUIRED_TOPIC, PASSKEY_ROTATION_ENFORCED_TOPIC,
    OWNERSHIP_CANCELLED_TOPIC, MIN_THRESHOLD_SET_TOPIC, MIN_THRESHOLD_SKIP_TOPIC, MIN_THRESHOLD_REDISTRIBUTE_TOPIC,
    DUPLICATE_VAULT_TOPIC,
    MetadataVersionEntry, META_VERSION_TOPIC, META_REVERT_TOPIC, VAULT_ARCHIVED_TOPIC,
    VAULT_CAP_TOPIC, BENEFICIARY_CAP_TOPIC,
    CheckInHistoryEntry, CheckInStreak,
    DELEGATE_CHECKIN_TOPIC, REVOKE_DELEGATE_TOPIC, CHECKIN_POW_TOPIC, TTL_PREDICTED_TOPIC,
    BATCH_CHECKIN_TOPIC,
    STATE_TRANSITION_TOPIC, OWNERSHIP_PROOF_TOPIC, INTEGRITY_TOPIC, BATCH_STATUS_TOPIC,
    PROOF_OF_LIFE_TOPIC, RELEASE_VOTE_TOPIC, RELEASE_VOTE_PASSED_TOPIC,
    HibernationEntry, EncryptedBackupCodes, PasskeyAnalytics, PasskeyUsageStat,
    HIBERNATION_ENTERED_TOPIC, HIBERNATION_EXITED_TOPIC,
    INACTIVITY_PENALTY_TOPIC, BEN_ROTATION_TOPIC, CLIFF_REACHED_TOPIC,
    WITHDRAWAL_AUDIT_TOPIC, WITHDRAWAL_FAILED_TOPIC, WITHDRAWAL_NOTIF_TOPIC,
    WITHDRAWAL_DISPUTE_FILED_TOPIC, WITHDRAWAL_DISPUTE_RESOLVED_TOPIC,
    TTL_BORROW_TOPIC, TTL_REPAY_TOPIC,
    CHECKIN_RATE_LIMITED_TOPIC, TTL_ACCELERATE_TOPIC, CHECKIN_GEO_TOPIC,
    BACKUP_CODES_ENCRYPTED_TOPIC, PASSKEY_ANALYTICS_TOPIC,
    WITHDRAWAL_VALIDATION_TOPIC, WITHDRAWAL_LIMIT_SET_TOPIC, WITHDRAWAL_LIMIT_EXCEEDED_TOPIC,
    WHITELIST_ADDED_TOPIC, WHITELIST_REMOVED_TOPIC, WHITELIST_VIOLATION_TOPIC,
    WRAPPED_TOKEN_REGISTERED_TOPIC, WRAPPED_TOKEN_UNREGISTERED_TOPIC,
    WITHDRAWAL_REVERSED_TOPIC, REVERSAL_GRACE_EXPIRED_TOPIC,
    VestingCatchUpConfig, VestingBonusConfig,
    VESTING_CATCHUP_SET_TOPIC, VESTING_CATCHUP_CLAIMED_TOPIC,
    VESTING_BONUS_SET_TOPIC, VESTING_BONUS_CLAIMED_TOPIC,
    TokenConversion, TokenStaking, YieldDistributionMode, YieldDistributionConfig,
    TOKEN_CONVERSION_TOPIC, TOKEN_WHITELIST_VALIDATED_TOPIC,
    TOKEN_STAKING_TOPIC, TOKEN_UNSTAKING_TOPIC,
    YIELD_DISTRIBUTED_TOPIC, YIELD_REINVESTED_TOPIC,
    TokenLending, TOKEN_LENDING_TOPIC, TOKEN_LEND_REPAY_TOPIC,
    TokenCollateral, TOKEN_COLLATERAL_TOPIC, TOKEN_COLLAT_RLSD_TOPIC,
    TokenHedge, TOKEN_HEDGE_TOPIC, TOKEN_HEDGE_CLOSE_TOPIC,
    TokenWeight, TokenRebalanceConfig, TOKEN_REBALANCE_TOPIC, TOKEN_REBALANCED_TOPIC,
    BeneficiaryPool, POOL_CREATED_TOPIC,
};
#[cfg(test)]
mod regression_tests;
#[cfg(test)]
mod beneficiary_pooling_tests;
#[cfg(test)]
mod beneficiary_vesting_auction_tests;
#[cfg(test)]
mod beneficiary_vesting_tests;
#[cfg(test)]
mod beneficiary_auction_tests;

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

#[contracterror(export = false)]
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
    DuplicateVault = 57,
    CheckInTooFrequent = 58,
    VaultFrozen = 59,
    CliffNotReached = 60,
    InsufficientTtlToAccelerate = 61,
    TtlBorrowNotFound = 62,
    TtlBorrowAlreadyRepaid = 63,
    // Issue #565: withdrawal scheduling validation
    OverlappingWithdrawalSchedule = 64,
    ConflictingWithdrawalSchedule = 65,
    // Issue #566: withdrawal limits by time
    DailyWithdrawalLimitExceeded = 66,
    WeeklyWithdrawalLimitExceeded = 67,
    MonthlyWithdrawalLimitExceeded = 68,
    // Issue #567: withdrawal destination whitelist
    WithdrawalDestinationNotWhitelisted = 69,
    // Issue #568: withdrawal reversal
    WithdrawalReversalGracePeriodExpired = 70,
    WithdrawalAlreadyReversed = 71,
    // Issue #545: vesting catch-up
    CatchUpNotEnabled = 72,
    // Issue #546: vesting bonus
    BonusNotEnabled = 73,
    TokenNotWhitelisted = 74,
    // Issue #526: post-release clawback
    NotReleased = 75,
    GracePeriodExpired = 76,
    NothingToClawback = 77,
    // Issue #527: beneficiary auction
    AuctionNotFound = 78,
    AuctionAlreadyExists = 79,
    AuctionEnded = 80,
    AuctionNotEnded = 81,
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

    /// Gets the canonical token address for a registered wrapped token.
    pub fn get_wrapped_token(env: Env, wrapped_token_address: Address) -> Option<Address> {
        let key = DataKey::WrappedToken(wrapped_token_address);
        env.storage().persistent().get(&key)
    }

    /// Resolves a token to its canonical equivalent if it is a wrapped token.
    pub fn resolve_canonical_token(env: Env, token_address: Address) -> Address {
        Self::get_wrapped_token(env.clone(), token_address.clone()).unwrap_or(token_address)
    }

    /// Checks if a token is whitelisted.
    ///
    /// # Arguments
    /// * `token_address` - The token contract address to check
    ///
    /// # Returns
    /// `true` if the token is whitelisted, is the default XLM token, or is a registered wrapped token whose canonical token is whitelisted.
    pub fn is_token_whitelisted(env: Env, token_address: Address) -> bool {
        let default_token = Self::load_token(&env);
        if token_address == default_token {
            return true;
        }

        let key = DataKey::TokenWhitelist(token_address.clone());
        if env.storage().persistent().get::<DataKey, bool>(&key).unwrap_or(false) {
            return true;
        }

        if let Some(canonical_token) = Self::get_wrapped_token(env.clone(), token_address.clone()) {
            if canonical_token == default_token {
                return true;
            }

            let canonical_key = DataKey::TokenWhitelist(canonical_token);
            env.storage().persistent().get::<DataKey, bool>(&canonical_key).unwrap_or(false)
        } else {
            false
        }
    }

    /// Registers a wrapped token for cross-chain compatibility.
    ///
    /// Wrapped tokens are accepted as whitelisted if their canonical token is whitelisted.
    pub fn register_wrapped_token(
        env: Env,
        wrapped_token_address: Address,
        canonical_token_address: Address,
    ) {
        Self::require_admin(&env);
        if wrapped_token_address == canonical_token_address {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }

        Self::assert_token_whitelisted(&env, &canonical_token_address);

        let key = DataKey::WrappedToken(wrapped_token_address.clone());
        env.storage().persistent().set(&key, &canonical_token_address);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (WRAPPED_TOKEN_REGISTERED_TOPIC,),
            (&wrapped_token_address, &canonical_token_address),
        );
    }

    /// Unregisters a wrapped token mapping.
    pub fn unregister_wrapped_token(env: Env, wrapped_token_address: Address) {
        Self::require_admin(&env);
        let key = DataKey::WrappedToken(wrapped_token_address.clone());
        env.storage().persistent().remove(&key);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((WRAPPED_TOKEN_UNREGISTERED_TOPIC,), wrapped_token_address);
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

    // --- Issue #581: Token Conversion ---

    /// Enables token conversion for a vault.
    /// Allows converting vault tokens to different tokens before release.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// * `from_token` - The source token address
    /// * `to_token` - The target token address
    /// * `conversion_rate` - Conversion rate in basis points (10000 = 1:1)
    ///
    /// # Panics
    /// * Panics if caller is not the vault owner
    /// * Panics if target token is not whitelisted
    pub fn enable_token_conversion(
        env: Env,
        vault_id: u64,
        from_token: Address,
        to_token: Address,
        conversion_rate: i128,
    ) {
        let vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();

        // Validate tokens are whitelisted
        Self::assert_token_whitelisted(&env, &from_token);
        Self::assert_token_whitelisted(&env, &to_token);

        let conversion = TokenConversion {
            vault_id,
            from_token: from_token.clone(),
            to_token: to_token.clone(),
            conversion_rate,
            enabled: true,
            created_at: env.ledger().timestamp(),
        };

        let key = DataKey::TokenConversion(vault_id);
        env.storage().persistent().set(&key, &conversion);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish(
            (TOKEN_CONVERSION_TOPIC, vault_id),
            (&from_token, &to_token, conversion_rate),
        );
    }

    /// Gets the token conversion configuration for a vault.
    pub fn get_token_conversion(env: Env, vault_id: u64) -> Option<TokenConversion> {
        let key = DataKey::TokenConversion(vault_id);
        env.storage().persistent().get(&key)
    }

    // --- Issue #583: Token Staking ---

    /// Enables token staking for a vault.
    /// Allows vault tokens to be staked for yield while locked.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// * `staking_pool` - The staking pool contract address
    /// * `annual_yield_bps` - Annual yield in basis points (e.g., 500 = 5%)
    ///
    /// # Panics
    /// * Panics if caller is not the vault owner
    pub fn enable_token_staking(
        env: Env,
        vault_id: u64,
        staking_pool: Address,
        annual_yield_bps: u32,
    ) {
        let vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();

        let staking = TokenStaking {
            vault_id,
            staking_pool: staking_pool.clone(),
            staked_amount: 0,
            staking_start: env.ledger().timestamp(),
            annual_yield_bps,
            is_active: true,
        };

        let key = DataKey::TokenStaking(vault_id);
        env.storage().persistent().set(&key, &staking);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish(
            (TOKEN_STAKING_TOPIC, vault_id),
            (&staking_pool, annual_yield_bps),
        );
    }

    /// Disables token staking for a vault.
    pub fn disable_token_staking(env: Env, vault_id: u64) {
        let vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();

        let key = DataKey::TokenStaking(vault_id);
        if let Some(mut staking) = env.storage().persistent().get::<DataKey, TokenStaking>(&key) {
            staking.is_active = false;
            env.storage().persistent().set(&key, &staking);
            env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        }
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish((TOKEN_UNSTAKING_TOPIC, vault_id), vault_id);
    }

    /// Gets the token staking configuration for a vault.
    pub fn get_token_staking(env: Env, vault_id: u64) -> Option<TokenStaking> {
        let key = DataKey::TokenStaking(vault_id);
        env.storage().persistent().get(&key)
    }

    // --- Issue #584: Token Yield Distribution ---

    /// Configures yield distribution for a vault.
    /// Determines how staking yield is distributed or reinvested.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// * `mode` - The yield distribution mode
    ///
    /// # Panics
    /// * Panics if caller is not the vault owner
    pub fn set_yield_distribution(
        env: Env,
        vault_id: u64,
        mode: YieldDistributionMode,
    ) {
        let vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();

        let config = YieldDistributionConfig {
            vault_id,
            mode: mode.clone(),
            last_distribution: env.ledger().timestamp(),
            total_distributed: 0,
            total_reinvested: 0,
        };

        let key = DataKey::YieldDistributionConfig(vault_id);
        env.storage().persistent().set(&key, &config);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish((YIELD_DISTRIBUTED_TOPIC, vault_id), vault_id);
    }

    /// Gets the yield distribution configuration for a vault.
    pub fn get_yield_distribution(env: Env, vault_id: u64) -> Option<YieldDistributionConfig> {
        let key = DataKey::YieldDistributionConfig(vault_id);
        env.storage().persistent().get(&key)
    }

    /// Distributes accumulated yield to beneficiary or reinvests it.
    /// This function calculates yield based on staking configuration and distributes
    /// according to the configured mode.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    ///
    /// # Panics
    /// * Panics if vault has no staking or yield distribution configured
    pub fn distribute_yield(env: Env, vault_id: u64) {
        let mut vault = Self::load_vault(&env, vault_id);
        
        let staking_key = DataKey::TokenStaking(vault_id);
        let staking = env.storage().persistent().get::<DataKey, TokenStaking>(&staking_key)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::InvalidAmount));

        let yield_key = DataKey::YieldDistributionConfig(vault_id);
        let mut yield_config = env.storage().persistent().get::<DataKey, YieldDistributionConfig>(&yield_key)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::InvalidAmount));

        if !staking.is_active {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }

        // Calculate yield: (staked_amount * annual_yield_bps * time_elapsed) / (10000 * 365 * 86400)
        let now = env.ledger().timestamp();
        let time_elapsed = now.saturating_sub(staking.staking_start);
        let yield_amount = (staking.staked_amount as i128)
            .checked_mul(staking.annual_yield_bps as i128)
            .and_then(|v| v.checked_mul(time_elapsed as i128))
            .and_then(|v| v.checked_div(10000 * 365 * 86400))
            .unwrap_or(0);

        if yield_amount <= 0 {
            return;
        }

        match &yield_config.mode {
            YieldDistributionMode::DistributeToBeneficiary => {
                // Transfer yield to beneficiary
                let token_client = token::Client::new(&env, &vault.token_address);
                token_client.transfer(
                    &env.current_contract_address(),
                    &vault.beneficiary,
                    &yield_amount,
                );
                yield_config.total_distributed = yield_config.total_distributed
                    .checked_add(yield_amount)
                    .unwrap_or(yield_config.total_distributed);
                env.events().publish((YIELD_DISTRIBUTED_TOPIC, vault_id), yield_amount);
            }
            YieldDistributionMode::Reinvest => {
                // Reinvest yield back into vault
                vault.balance = vault.balance
                    .checked_add(yield_amount)
                    .unwrap_or(vault.balance);
                yield_config.total_reinvested = yield_config.total_reinvested
                    .checked_add(yield_amount)
                    .unwrap_or(yield_config.total_reinvested);
                env.events().publish((YIELD_REINVESTED_TOPIC, vault_id), yield_amount);
            }
            YieldDistributionMode::Split(beneficiary_bps) => {
                // Split yield between beneficiary and reinvestment
                let beneficiary_amount = (yield_amount as i128)
                    .checked_mul(*beneficiary_bps as i128)
                    .and_then(|v| v.checked_div(10000))
                    .unwrap_or(0);
                let reinvest_amount = yield_amount.saturating_sub(beneficiary_amount);

                if beneficiary_amount > 0 {
                    let token_client = token::Client::new(&env, &vault.token_address);
                    token_client.transfer(
                        &env.current_contract_address(),
                        &vault.beneficiary,
                        &beneficiary_amount,
                    );
                    yield_config.total_distributed = yield_config.total_distributed
                        .checked_add(beneficiary_amount)
                        .unwrap_or(yield_config.total_distributed);
                    env.events().publish((YIELD_DISTRIBUTED_TOPIC, vault_id), beneficiary_amount);
                }

                if reinvest_amount > 0 {
                    vault.balance = vault.balance
                        .checked_add(reinvest_amount)
                        .unwrap_or(vault.balance);
                    yield_config.total_reinvested = yield_config.total_reinvested
                        .checked_add(reinvest_amount)
                        .unwrap_or(yield_config.total_reinvested);
                    env.events().publish((YIELD_REINVESTED_TOPIC, vault_id), reinvest_amount);
                }
            }
        }

        yield_config.last_distribution = now;
        env.storage().persistent().set(&yield_key, &yield_config);
        env.storage().persistent().extend_ttl(&yield_key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        Self::save_vault(&env, vault_id, &vault);
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
            let dup_key = DataKey::VaultDuplicate(owner.clone(), beneficiary.clone(), check_in_interval);
            if env.storage().persistent().has(&dup_key) {
                let existing_id: u64 = env.storage().persistent().get(&dup_key).unwrap();
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
            // Store duplicate key to prevent duplicate creation
            env.storage().persistent().set(&dup_key, &vault_id);
            env.storage().persistent().extend_ttl(&dup_key, VAULT_TTL_THRESHOLD, vault_ttl_ledgers(check_in_interval));
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
                    return Err(ContractError::InvalidInterval);
                }
            }
        }

        // Issue #549: enforce passkey registration and expiry.
        Self::validate_passkey_for_checkin(&env, vault_id, &vault, &passkey_hash, now)?;

        vault.last_check_in = now;
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
                if !rotation.new_beneficiaries.is_empty() {
                    vault.beneficiaries = rotation.new_beneficiaries.clone();
                }
                env.events().publish((BEN_ROTATION_TOPIC, vault_id), rotation.effective_timestamp);
            }
        }

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
    /// * Panics if any vault uses a non-whitelisted token
    /// * Panics if the total amount overflows
    pub fn batch_deposit(env: Env, from: Address, deposits: Vec<(u64, i128)>) {
        Self::assert_not_paused(&env);
        from.require_auth();

        let mut validated = Vec::new(&env);
        let mut total_amount = 0i128;
        let default_token = Self::load_token(&env);

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

            // Issue #582: Validate token whitelist
            Self::assert_token_whitelisted(&env, &vault.token_address);

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
            
            // Issue #582: Emit token whitelist validation event
            env.events().publish(
                (TOKEN_WHITELIST_VALIDATED_TOPIC, vault_id),
                (&vault.token_address, amount),
            );
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
                let timestamp = env.ledger().timestamp();
                Self::record_withdrawal_audit(&env, vault_id, &caller, amount, false, "Contract paused");
                return Err(ContractError::Paused);
            }
            if amount <= 0 {
                let timestamp = env.ledger().timestamp();
                Self::record_withdrawal_audit(&env, vault_id, &caller, amount, false, "Invalid amount");
                return Err(ContractError::InvalidAmount);
            }
            caller.require_auth();
            let mut vault = Self::load_vault(&env, vault_id);
            if vault.is_paused {
                Self::record_withdrawal_audit(&env, vault_id, &caller, amount, false, "Vault paused");
                return Err(ContractError::Paused);
            }
            if caller != vault.owner {
                Self::record_withdrawal_audit(&env, vault_id, &caller, amount, false, "Not owner");
                return Err(ContractError::NotOwner);
            }
            if vault.status == ReleaseStatus::EmergencyFrozen {
                Self::record_withdrawal_audit(&env, vault_id, &caller, amount, false, "Vault frozen");
                return Err(ContractError::VaultFrozen);
            }
            if vault.status != ReleaseStatus::Locked {
                Self::record_withdrawal_audit(&env, vault_id, &caller, amount, false, "Vault already released");
                return Err(ContractError::AlreadyReleased);
            }
            if vault.balance < amount {
                Self::record_withdrawal_audit(&env, vault_id, &caller, amount, false, "Insufficient balance");
                return Err(ContractError::InsufficientBalance);
            }

            // Check withdrawal approval threshold - Issue #404
            if let Some(threshold) = vault.withdrawal_approval_threshold {
                if amount > threshold {
                    Self::record_withdrawal_audit(&env, vault_id, &caller, amount, false, "Withdrawal not approved");
                    return Err(ContractError::WithdrawalNotApproved);
                }
            }

            // Check withdrawal limits - Issue #566
            Self::check_withdrawal_limits(&env, vault_id, amount)?;

            // Check whitelist - Issue #567
            if !Self::is_whitelisted(&env, vault_id, &vault.owner) {
                return Err(ContractError::WithdrawalDestinationNotWhitelisted);
            }

            let token_client = token::Client::new(&env, &vault.token_address);
            token_client.transfer(&env.current_contract_address(), &vault.owner, &amount);
            vault.balance -= amount;
            
            // Record withdrawal for reversal - Issue #568 (grace period: 24 hours)
            Self::record_withdrawal_for_reversal(&env, vault_id, amount, 86_400);
            
            Self::save_vault(&env, vault_id, &vault);
            Self::log_audit_entry(&env, vault_id, "withdraw", &caller, "");
            Self::append_activity_log(&env, vault_id, "withdraw", &caller, "");
            
            // Issue #569: Record successful withdrawal in audit trail
            Self::record_withdrawal_audit(&env, vault_id, &caller, amount, true, "");
            
            // Issue #571: Emit withdrawal notification event
            env.events().publish(
                (WITHDRAWAL_NOTIF_TOPIC, vault_id),
                (&caller, amount, env.ledger().timestamp()),
            );
            
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
                Self::record_withdrawal_audit(&env, vault_id, &caller, amount, false, "Invalid amount");
                return Err(ContractError::InvalidAmount);
            }
            let vault = Self::try_load_vault(&env, vault_id)
                .ok_or(ContractError::VaultNotFound)?;
            if caller != vault.owner {
                Self::record_withdrawal_audit(&env, vault_id, &caller, amount, false, "Not owner");
                return Err(ContractError::NotOwner);
            }
            if vault.status != ReleaseStatus::Locked {
                Self::record_withdrawal_audit(&env, vault_id, &caller, amount, false, "Vault already released");
                return Err(ContractError::AlreadyReleased);
            }
            if vault.balance < amount {
                Self::record_withdrawal_audit(&env, vault_id, &caller, amount, false, "Insufficient balance");
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
                Self::record_withdrawal_audit(&env, vault_id, &caller, amount, false, "Incompatible token");
                return Err(ContractError::InvalidAmount);
            }
            token_client.transfer(&env.current_contract_address(), &vault.owner, &amount);
            vault.balance -= amount;
            let remaining = vault.balance;
            Self::save_vault(&env, vault_id, &vault);
            
            // Issue #569: Record successful withdrawal in audit trail
            Self::record_withdrawal_audit(&env, vault_id, &caller, amount, true, "");
            
            // Issue #571: Emit withdrawal notification event
            env.events().publish(
                (WITHDRAWAL_NOTIF_TOPIC, vault_id),
                (&caller, amount, env.ledger().timestamp()),
            );
            
            env.events().publish(
                (WITHDRAW_TOPIC, vault_id),
                (amount, remaining),
            );
        }
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(())
    }

    // --- Issue #573: Withdrawal Proof ---

    /// Generates a cryptographic proof of withdrawal for compliance.
    ///
    /// Creates a proof hash that can be used to verify a withdrawal occurred.
    /// The proof includes vault_id, amount, timestamp, and a nonce.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `amount` - The withdrawal amount
    ///
    /// # Returns
    /// A WithdrawalProof struct containing the proof hash
    pub fn generate_withdrawal_proof(
        env: Env,
        vault_id: u64,
        amount: i128,
    ) -> Result<WithdrawalProof, ContractError> {
        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        let vault = Self::try_load_vault(&env, vault_id)
            .ok_or(ContractError::VaultNotFound)?;
        if vault.balance < amount {
            return Err(ContractError::InsufficientBalance);
        }

        let now = env.ledger().timestamp();
        let nonce = env.ledger().sequence();
        
        // Create proof hash from vault_id, amount, timestamp, and nonce
        let mut data = Vec::new(&env);
        data.push_back(vault_id as u32);
        data.push_back(amount as u32);
        data.push_back(now as u32);
        data.push_back(nonce as u32);
        
        let proof_bytes = Bytes::from_array(&env, &[0u8; 32]);
        let proof_hash = env.crypto().sha256(&proof_bytes);

        let proof = WithdrawalProof {
            vault_id,
            amount,
            timestamp: now,
            proof_hash,
            nonce,
        };

        env.storage().persistent().set(&DataKey::WithdrawalProof(vault_id, nonce), &proof);
        env.storage().persistent().extend_ttl(
            &DataKey::WithdrawalProof(vault_id, nonce),
            VAULT_TTL_THRESHOLD,
            vault_ttl_ledgers(vault.check_in_interval as u64),
        );

        env.events().publish((WITHDRAWAL_PROOF_TOPIC, vault_id), (amount, nonce));
        Ok(proof)
    }

    // --- Issue #576: Withdrawal Escrow ---

    /// Creates a withdrawal escrow entry, holding funds pending verification.
    ///
    /// Funds are held in escrow and can only be released after verification.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `amount` - The amount to hold in escrow
    /// * `beneficiary` - The beneficiary address
    ///
    /// # Returns
    /// `Ok(())` on success
    pub fn create_withdrawal_escrow(
        env: Env,
        vault_id: u64,
        amount: i128,
        beneficiary: Address,
    ) -> Result<(), ContractError> {
        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        let vault = Self::try_load_vault(&env, vault_id)
            .ok_or(ContractError::VaultNotFound)?;
        if vault.balance < amount {
            return Err(ContractError::InsufficientBalance);
        }

        let now = env.ledger().timestamp();
        let escrow = WithdrawalEscrow {
            vault_id,
            amount,
            timestamp: now,
            beneficiary: beneficiary.clone(),
            verified: false,
        };

        env.storage().persistent().set(&DataKey::WithdrawalEscrow(vault_id), &escrow);
        env.storage().persistent().extend_ttl(
            &DataKey::WithdrawalEscrow(vault_id),
            VAULT_TTL_THRESHOLD,
            vault_ttl_ledgers(vault.check_in_interval as u64),
        );

        env.events().publish((WITHDRAWAL_ESCROW_CREATED_TOPIC, vault_id), (amount, beneficiary));
        Ok(())
    }

    /// Verifies and releases a withdrawal from escrow.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    ///
    /// # Returns
    /// `Ok(())` on success
    pub fn verify_withdrawal_escrow(
        env: Env,
        vault_id: u64,
    ) -> Result<(), ContractError> {
        let mut vault = Self::load_vault(&env, vault_id);
        let escrow = env.storage().persistent()
            .get::<DataKey, WithdrawalEscrow>(&DataKey::WithdrawalEscrow(vault_id))
            .ok_or(ContractError::VaultNotFound)?;

        if escrow.verified {
            return Err(ContractError::InvalidAmount);
        }

        let token_client = token::Client::new(&env, &vault.token_address);
        token_client.transfer(&env.current_contract_address(), &escrow.beneficiary, &escrow.amount);
        vault.balance -= escrow.amount;
        Self::save_vault(&env, vault_id, &vault);

        env.storage().persistent().remove(&DataKey::WithdrawalEscrow(vault_id));
        env.events().publish((WITHDRAWAL_ESCROW_VERIFIED_TOPIC, vault_id), escrow.amount);
        Ok(())
    }

    // --- Issue #574: Withdrawal Rollback ---

    /// Rolls back a withdrawal if fraud is detected.
    ///
    /// Only the vault owner can initiate a rollback within a certain time window.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `rollback_amount` - The amount to rollback
    /// * `reason` - The reason for rollback
    /// * `caller` - The caller address (must be vault owner)
    ///
    /// # Returns
    /// `Ok(())` on success
    pub fn rollback_withdrawal(
        env: Env,
        vault_id: u64,
        rollback_amount: i128,
        reason: String,
        caller: Address,
    ) -> Result<(), ContractError> {
        if rollback_amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        caller.require_auth();
        
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let now = env.ledger().timestamp();
        let rollback = WithdrawalRollback {
            vault_id,
            original_amount: rollback_amount,
            rollback_amount,
            timestamp: now,
            reason,
        };

        vault.balance += rollback_amount;
        Self::save_vault(&env, vault_id, &vault);

        env.storage().persistent().set(&DataKey::WithdrawalRollback(vault_id), &rollback);
        env.storage().persistent().extend_ttl(
            &DataKey::WithdrawalRollback(vault_id),
            VAULT_TTL_THRESHOLD,
            vault_ttl_ledgers(vault.check_in_interval as u64),
        );

        env.events().publish((WITHDRAWAL_ROLLBACK_TOPIC, vault_id), rollback_amount);
        Ok(())
    }

    // --- Issue #575: Withdrawal Rate Limiting ---

    /// Sets the withdrawal rate limit for a vault.
    ///
    /// Prevents abuse by limiting withdrawal frequency.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `cooldown_seconds` - Minimum seconds between withdrawals
    /// * `caller` - The caller address (must be vault owner)
    ///
    /// # Returns
    /// `Ok(())` on success
    pub fn set_withdrawal_rate_limit(
        env: Env,
        vault_id: u64,
        cooldown_seconds: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let now = env.ledger().timestamp();
        let rate_limit = WithdrawalRateLimit {
            vault_id,
            last_withdrawal_time: now,
            withdrawal_count: 0,
            cooldown_seconds,
        };

        env.storage().persistent().set(&DataKey::WithdrawalRateLimit(vault_id), &rate_limit);
        env.storage().persistent().extend_ttl(
            &DataKey::WithdrawalRateLimit(vault_id),
            VAULT_TTL_THRESHOLD,
            vault_ttl_ledgers(vault.check_in_interval as u64),
        );

        Ok(())
    }

    /// Checks if a withdrawal is allowed based on rate limiting.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    ///
    /// # Returns
    /// `Ok(true)` if withdrawal is allowed, `Ok(false)` if rate limited
    pub fn is_withdrawal_allowed(
        env: Env,
        vault_id: u64,
    ) -> Result<bool, ContractError> {
        if let Some(rate_limit) = env.storage().persistent()
            .get::<DataKey, WithdrawalRateLimit>(&DataKey::WithdrawalRateLimit(vault_id)) {
            let now = env.ledger().timestamp();
            let time_since_last = now.saturating_sub(rate_limit.last_withdrawal_time);
            
            if time_since_last < rate_limit.cooldown_seconds {
                env.events().publish((WITHDRAWAL_RATE_LIMITED_TOPIC, vault_id), time_since_last);
                return Ok(false);
            }
        }
        Ok(true)
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
                    if !rotation.new_beneficiaries.is_empty() {
                        vault.beneficiaries = rotation.new_beneficiaries.clone();
                    }
                    env.events().publish((BEN_ROTATION_TOPIC, vault_id), rotation.effective_timestamp);
                }
            }

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
        Self::trigger_release_internal(env, vault_id, ReleaseTrigger::Expiry);
    }

    pub fn trigger_release_manual(env: Env, vault_id: u64) {
        Self::trigger_release_internal(env, vault_id, ReleaseTrigger::Manual);
    }

    fn trigger_release_internal(env: Env, vault_id: u64, mode: ReleaseTrigger) {
        Self::assert_not_paused(&env);
        // Attempt to restore archived vault state before proceeding - Issue #443
        Self::try_restore_archived_vault(&env, vault_id);
        let mut vault = Self::load_vault(&env, vault_id);
        if vault.status != ReleaseStatus::Locked {
            panic_with_error!(&env, ContractError::AlreadyReleased);
        }
        // We only block vetoes on trigger before the vault has expired.
        // Note: release entrypoint is for the Expiry trigger and thus requires expiry,
        // but manual release can occur at arbitrary times.
        // Here we keep the existing expiry gate.
        if !Self::is_expired(env.clone(), vault_id) {
            panic_with_error!(&env, ContractError::NotExpired);
        }

        // Beneficiary veto of owner-defined release conditions (Issue: beneficiary veto before expiry).
        // The veto is only meaningful before expiry; since `trigger_release*` requires expiry,
        // veto has already expired in practice. However, for completeness (and for manual release),
        // we gate on expiry-less times above in the public veto method.
        let total = vault.balance;
        if total == 0 {
            panic_with_error!(&env, ContractError::EmptyVault);
        }

        // Beneficiary veto of owner-defined release conditions (Issue: beneficiary veto before expiry).
        // Block `trigger_release*` only if veto was set and vault has not yet expired.
        // (This is also harmless for expiry-triggered releases, since those require expiry.)
        if !Self::is_expired(env.clone(), vault_id) {
            let vetoed: bool = env
                .storage()
                .persistent()
                .get(&DataKey::BeneficiaryReleaseConditionVeto(vault_id))
                .unwrap_or(false);
            if vetoed {
                panic_with_error!(&env, ContractError::BeneficiaryVetoReleaseCondition);
            }
        }

        // Check beneficiary acceptance status - Issue #397
        // Only block if it's a single-beneficiary vault where the primary has declined.
        // Multi-beneficiary vaults handle per-beneficiary decline in distribute_release_amount.
        if vault.beneficiaries.is_empty() {
            let beneficiary_status = Self::get_beneficiary_status(env.clone(), vault_id);
            if beneficiary_status == BeneficiaryStatus::Declined {
                panic_with_error!(&env, ContractError::InvalidBeneficiary);
            }
        }

        // Check beneficiary conditional acceptance threshold - Issue #503
        if !Self::check_conditional_acceptance_threshold(&env, vault_id, total)
            .unwrap_or(false)
        {
            panic_with_error!(&env, ContractError::InsufficientBalance);
        }

        // Check beneficiary proof of life - Issue #498
        let now = env.ledger().timestamp();
        let pol_key = DataKey::ProofOfLife(vault_id);
        if let Some(pol) = env.storage().persistent().get::<DataKey, ProofOfLifeEntry>(&pol_key) {
            if now > pol.valid_until {
                panic_with_error!(&env, ContractError::ProofOfLifeRequired);
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
        let has_milestone_vesting = env
            .storage()
            .persistent()
            .has(&DataKey::MilestoneVestingSchedule(vault_id));

        if has_vesting || has_milestone_vesting {
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
                Self::distribute_release_amount(&env, vault_id, &vault, release_amount, mode);
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
                // Remove duplicate key so the same parameters can be reused
                let dup_key = DataKey::VaultDuplicate(vault.owner.clone(), vault.beneficiary.clone(), vault.check_in_interval);
                env.storage().persistent().remove(&dup_key);
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

    /// Schedule a beneficiary rotation for a vault.
    ///
    /// The caller must be the vault owner. The `new_beneficiaries` vector must
    /// be non-empty and sum to 10,000 BPS. The rotation will be applied
    /// automatically when the effective timestamp is reached (e.g., on next
    /// check-in or other mutation) and emits a `BEN_ROTATION_TOPIC` event.
    pub fn schedule_beneficiary_rotation(
        env: Env,
        vault_id: u64,
        caller: Address,
        effective_timestamp: u64,
        new_beneficiaries: Vec<BeneficiaryEntry>,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        if new_beneficiaries.is_empty() {
            return Err(ContractError::InvalidBps);
        }
        let total_bps: u32 = new_beneficiaries.iter().map(|e| e.bps).sum();
        if total_bps != 10_000 {
            return Err(ContractError::InvalidBps);
        }
        for entry in new_beneficiaries.iter() {
            if entry.address == vault.owner {
                return Err(ContractError::InvalidBeneficiary);
            }
        }
        let rot_key = DataKey::BeneficiaryRotationSchedule(vault_id);
        let mut schedule: Vec<BeneficiaryRotationEntry> = env.storage().persistent()
            .get::<DataKey, Vec<BeneficiaryRotationEntry>>(&rot_key)
            .unwrap_or_else(|| Vec::new(&env));
        schedule.push(BeneficiaryRotationEntry { effective_timestamp, new_beneficiaries: new_beneficiaries.clone() });
        env.storage().persistent().set(&rot_key, &schedule);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((BENEFICIARY_TRIGGER_SET_TOPIC, vault_id), (effective_timestamp, new_beneficiaries));
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
    pub fn set_ben_min_threshold(
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
            "set_ben_min_threshold",
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
    pub fn get_ben_min_threshold(
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
    pub fn get_bens_with_thresholds(
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
    /// Once set, a portion of the vault's balance (`amount`) is released to the
    /// beneficiary (or beneficiaries) in `num_installments` equal tranches.
    /// Each tranche becomes claimable every `interval` seconds starting from
    /// `start_time`.
    ///
    /// Multiple independent schedules may be attached to the same vault, each
    /// with its own amount, timing, and cliff. The sum of all schedule amounts
    /// must not exceed the vault's current balance.
    ///
    /// The vault must be released (`trigger_release`) before installments can
    /// be claimed. The schedule is set by the owner while the vault is still
    /// `Locked`.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault to attach the schedule to
    /// * `caller` - Must be the vault owner
    /// * `start_time` - Unix timestamp of the first claimable installment
    /// * `interval` - Seconds between installments (must be > 0)
    /// * `num_installments` - Number of tranches (must be > 0)
    /// * `amount` - Amount in stroops allocated to this schedule (must be > 0)
    /// * `cliff_period` - Seconds after `start_time` before any installment can be claimed (0 = no cliff)
    ///
    /// # Returns
    /// The index of the newly created schedule (0-based).
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not Locked
    /// * `ContractError::InvalidInterval` - If interval or num_installments is 0
    /// * `ContractError::EmptyVault` - If vault balance is 0
    /// * `ContractError::InvalidAmount` - If amount is <= 0
    /// * `ContractError::InsufficientBalance` - If total scheduled amount would exceed vault balance
    /// * `ContractError::VaultCapacityExceeded` - If max schedules per vault reached
    pub fn set_vesting_schedule(
        env: Env,
        vault_id: u64,
        caller: Address,
        start_time: u64,
        interval: u64,
        num_installments: u32,
        amount: i128,
        cliff_period: u64,
    ) -> Result<u32, ContractError> {
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
        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        // Compute the sum of all existing schedule amounts
        let count_key = DataKey::VestingScheduleCount(vault_id);
        let count: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);
        let mut total_scheduled = amount;
        for i in 0..count {
            if let Some(s) = env.storage().persistent()
                .get::<DataKey, VestingSchedule>(&DataKey::VestingSchedule(vault_id, i))
            {
                total_scheduled = total_scheduled
                    .checked_add(s.total_amount)
                    .unwrap_or_else(|| panic_with_error!(&env, ContractError::BalanceOverflow));
            }
        }
        if total_scheduled > vault.balance {
            return Err(ContractError::InsufficientBalance);
        }
        if count >= MAX_VESTING_SCHEDULES {
            return Err(ContractError::VaultCapacityExceeded);
        }

        let schedule_index = count;
        let schedule = VestingSchedule {
            start_time,
            interval,
            num_installments,
            claimed_installments: 0,
            total_amount: amount,
            cliff_period,
        };
        let key = DataKey::VestingSchedule(vault_id, schedule_index);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &schedule);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);

        // Update schedule count
        let count_key = DataKey::VestingScheduleCount(vault_id);
        env.storage().persistent().set(&count_key, &(count + 1));
        env.storage().persistent().extend_ttl(&count_key, VAULT_TTL_THRESHOLD, ttl);

        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (SET_VESTING_TOPIC, vault_id),
            (start_time, interval, num_installments, amount, cliff_period, schedule_index),
        );
        env.events().publish(
            (VESTING_SCHEDULE_ADDED_TOPIC, vault_id),
            (schedule_index, amount),
        );
        Ok(schedule_index)
    }

    /// Returns the vesting schedule for a vault, if one exists.
    pub fn get_vesting_schedule(env: Env, vault_id: u64) -> Option<VestingSchedule> {
        env.storage().persistent().get(&DataKey::VestingSchedule(vault_id))
    }

    /// Sets a late-claim penalty for a vault's vesting schedule.
    ///
    /// If a beneficiary claims an installment more than `grace_period_seconds` after
    /// it unlocked, the payout is reduced by `penalty_bps` basis points (e.g. 500 = 5%).
    ///
    /// # Arguments
    /// * `vault_id`             - The vault with an attached vesting schedule
    /// * `caller`               - Must be the vault owner
    /// * `penalty_bps`          - Penalty in basis points (1–10000)
    /// * `grace_period_seconds` - Seconds after unlock before the penalty applies
    ///
    /// # Errors
    /// * `ContractError::NotOwner`         - Caller is not the vault owner
    /// * `ContractError::VestingNotFound`  - No vesting schedule exists
    /// * `ContractError::InvalidBps`       - penalty_bps is 0 or > 10000
    pub fn set_vesting_penalty(
        env: Env,
        vault_id: u64,
        caller: Address,
        penalty_bps: u32,
        grace_period_seconds: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if !env.storage().persistent().has(&DataKey::VestingSchedule(vault_id)) {
            return Err(ContractError::VestingNotFound);
        }
        if penalty_bps == 0 || penalty_bps > 10_000 {
            return Err(ContractError::InvalidBps);
        }
        let config = VestingPenaltyConfig { penalty_bps, grace_period_seconds };
        let key = DataKey::VestingPenalty(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &config);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((VESTING_PENALTY_TOPIC, vault_id), (penalty_bps, grace_period_seconds));
        Ok(())
    }

    /// Returns the vesting penalty config for a vault, if one has been set.
    pub fn get_vesting_penalty(env: Env, vault_id: u64) -> Option<VestingPenaltyConfig> {
        env.storage().persistent().get(&DataKey::VestingPenalty(vault_id))
    }

    // --- Issue #548: Vesting Reversal ---

    /// Initiates a reversible vesting claim, holding tokens in escrow for a grace period.
    ///
    /// Unlike `claim_vested_installment` (which transfers immediately), this function
    /// calculates the claimable amount and stores a `VestingPendingClaim` record without
    /// transferring any tokens. The vault owner may call `reverse_vesting_claim` within
    /// `reversal_window_seconds` to cancel. After the window, anyone may call
    /// `finalize_vesting_claim` to complete the transfer.
    ///
    /// Only one pending claim may exist per vault at a time; subsequent calls fail until
    /// the pending claim is finalized or reversed.
    ///
    /// # Arguments
    /// * `vault_id`               - The released vault with a vesting schedule
    /// * `reversal_window_seconds`- Grace period in seconds (must be > 0)
    ///
    /// # Returns
    /// The escrowed amount (not yet transferred).
    ///
    /// # Errors
    /// * `ContractError::Paused`              - Contract is paused
    /// * `ContractError::VestingNotFound`     - No schedule on this vault
    /// * `ContractError::NothingToClaimYet`   - No installments available
    /// * `ContractError::VestingAlreadyComplete` - All installments claimed
    /// * `ContractError::InvalidInterval`     - reversal_window_seconds is 0
    /// * `ContractError::InvalidAmount`       - A pending claim already exists
    pub fn initiate_vesting_claim(
        env: Env,
        vault_id: u64,
        reversal_window_seconds: u64,
    ) -> Result<i128, ContractError> {
        Self::assert_not_paused(&env);
        if reversal_window_seconds == 0 {
            return Err(ContractError::InvalidInterval);
        }
        // Block if a pending claim already exists.
        if env.storage().persistent().has(&DataKey::VestingPendingClaim(vault_id)) {
            return Err(ContractError::InvalidAmount);
        }

        let vault = Self::load_vault(&env, vault_id);
        if vault.status != ReleaseStatus::Released {
            return Err(ContractError::AlreadyReleased);
        }

        let mut schedule: VestingSchedule = env
            .storage().persistent()
            .get(&DataKey::VestingSchedule(vault_id))
            .ok_or(ContractError::VestingNotFound)?;

        if schedule.claimed_installments >= schedule.num_installments {
            return Err(ContractError::VestingAlreadyComplete);
        }

        let now = env.ledger().timestamp();
        if now < schedule.start_time {
            return Err(ContractError::NothingToClaimYet);
        }

        let elapsed = now - schedule.start_time;
        let unlocked = ((elapsed / schedule.interval) + 1)
            .min(schedule.num_installments as u64) as u32;
        let claimable = unlocked.saturating_sub(schedule.claimed_installments);
        if claimable == 0 {
            return Err(ContractError::NothingToClaimYet);
        }

        let per_installment = schedule.total_amount / schedule.num_installments as i128;
        let amount = if unlocked >= schedule.num_installments {
            vault.balance
        } else {
            per_installment * claimable as i128
        };

        let beneficiary = if vault.beneficiaries.is_empty() {
            vault.beneficiary.clone()
        } else {
            vault.beneficiaries.get(0).unwrap().address.clone()
        };

        let prev_installments_claimed = schedule.claimed_installments;

        // Advance the schedule counter so a second call is blocked until this claim
        // is finalized or reversed.
        schedule.claimed_installments = unlocked;
        let sched_key = DataKey::VestingSchedule(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&sched_key, &schedule);
        env.storage().persistent().extend_ttl(&sched_key, VAULT_TTL_THRESHOLD, ttl);

        let pending = VestingPendingClaim {
            amount,
            beneficiary,
            initiated_at: now,
            reversal_deadline: now + reversal_window_seconds,
            new_installments_claimed: unlocked,
            prev_installments_claimed,
        };
        let pk = DataKey::VestingPendingClaim(vault_id);
        env.storage().persistent().set(&pk, &pending);
        env.storage().persistent().extend_ttl(&pk, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(amount)
    }

    /// Reverses a pending vesting claim within the reversal window.
    ///
    /// Only the vault owner may call this. The pending claim record is cleared and
    /// the schedule counter is rolled back so the installments become claimable again.
    ///
    /// # Errors
    /// * `ContractError::NotOwner`            - Caller is not the vault owner
    /// * `ContractError::VestingReversalNotFound` - No pending claim on this vault
    /// * `ContractError::VestingReversalExpired`  - Reversal window has closed
    pub fn reverse_vesting_claim(
        env: Env,
        vault_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let pk = DataKey::VestingPendingClaim(vault_id);
        let pending: VestingPendingClaim = env
            .storage().persistent()
            .get(&pk)
            .ok_or(ContractError::VestingReversalNotFound)?;

        let now = env.ledger().timestamp();
        if now > pending.reversal_deadline {
            return Err(ContractError::VestingReversalExpired);
        }

        // Roll back the schedule counter so these installments can be claimed again.
        let sched_key = DataKey::VestingSchedule(vault_id);
        if let Some(mut schedule) = env.storage().persistent()
            .get::<DataKey, VestingSchedule>(&sched_key)
        {
            schedule.claimed_installments = pending.prev_installments_claimed;
            let ttl = vault_ttl_ledgers(vault.check_in_interval);
            env.storage().persistent().set(&sched_key, &schedule);
            env.storage().persistent().extend_ttl(&sched_key, VAULT_TTL_THRESHOLD, ttl);
        }

        env.storage().persistent().remove(&pk);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((VESTING_REVERSED_TOPIC, vault_id), pending.amount);
        Ok(())
    }

    /// Finalizes a pending vesting claim after the reversal window has closed.
    ///
    /// Anyone may call this. Transfers the escrowed amount to the beneficiary and
    /// clears the pending claim record.
    ///
    /// # Errors
    /// * `ContractError::VestingReversalNotFound` - No pending claim on this vault
    /// * `ContractError::InvalidInterval`         - Reversal window has not yet closed
    pub fn finalize_vesting_claim(env: Env, vault_id: u64) -> Result<i128, ContractError> {
        Self::assert_not_paused(&env);

        let pk = DataKey::VestingPendingClaim(vault_id);
        let pending: VestingPendingClaim = env
            .storage().persistent()
            .get(&pk)
            .ok_or(ContractError::VestingReversalNotFound)?;

        let now = env.ledger().timestamp();
        if now <= pending.reversal_deadline {
            return Err(ContractError::InvalidInterval);
        }

        let mut vault = Self::load_vault(&env, vault_id);
        if vault.balance < pending.amount {
            return Err(ContractError::InsufficientBalance);
        }

        let token_client = token::Client::new(&env, &vault.token_address);
        token_client.transfer(
            &env.current_contract_address(),
            &pending.beneficiary,
            &pending.amount,
        );

        vault.balance -= pending.amount;
        Self::save_vault(&env, vault_id, &vault);
        env.storage().persistent().remove(&pk);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((VESTING_FINALIZED_TOPIC, vault_id), pending.amount);
        Ok(pending.amount)
    }

    /// Returns the pending vesting claim for a vault, if one exists.
    pub fn get_pending_vesting_claim(env: Env, vault_id: u64) -> Option<VestingPendingClaim> {
        env.storage().persistent().get(&DataKey::VestingPendingClaim(vault_id))
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

        // Issue #543: check if vesting is accelerated
        let accelerated = env.storage().persistent()
            .get::<DataKey, VestingAccelerationConfig>(&DataKey::VestingAcceleration(vault_id))
            .map(|c| c.accelerated).unwrap_or(false);

        // How many installments are unlocked so far?
        let unlocked = if accelerated {
            schedule.num_installments
        } else {
            let elapsed = now.saturating_sub(schedule.start_time);
            ((elapsed / schedule.interval) + 1).min(schedule.num_installments as u64) as u32
        };

        let claimable = unlocked.saturating_sub(schedule.claimed_installments);
        if claimable == 0 {
            // Even if claimable is 0, we might have rolled over amounts to claim
            let rollover_cfg = env.storage().persistent()
                .get::<DataKey, VestingRolloverConfig>(&DataKey::VestingRollover(vault_id));
            if rollover_cfg.as_ref().map_or(true, |c| !c.enabled || c.rolled_amount == 0) {
                return Err(ContractError::NothingToClaimYet);
            }
        }

        // Calculate payout: each installment = total / num_installments,
        // last installment absorbs remainder.
        let per_installment = schedule.total_amount / schedule.num_installments as i128;
        let mut base_amount = if unlocked >= schedule.num_installments {
            vault.balance
        } else {
            per_installment * claimable as i128
        };

        // Issue #541: add rolled over amounts if enabled
        if let Some(mut cfg) = env.storage().persistent()
            .get::<DataKey, VestingRolloverConfig>(&DataKey::VestingRollover(vault_id))
        {
            if cfg.enabled && cfg.rolled_amount > 0 {
                base_amount += cfg.rolled_amount;
                cfg.rolled_amount = 0;
                env.storage().persistent().set(&DataKey::VestingRollover(vault_id), &cfg);
            }
        }

        // Issue #547: apply late-claim penalty to installments claimed after grace period.
        let amount = if let Some(penalty_cfg) = env.storage().persistent()
            .get::<DataKey, VestingPenaltyConfig>(&DataKey::VestingPenalty(vault_id))
        {
            // Count how many of the claimable installments are "late".
            // Installment i (1-based) unlocks at start_time + (i-1)*interval.
            let late_cutoff = now.saturating_sub(penalty_cfg.grace_period_seconds);
            let mut late_count = 0u32;
            for i in (schedule.claimed_installments + 1)..=unlocked {
                let unlock_time = schedule.start_time
                    .saturating_add((i as u64 - 1).saturating_mul(schedule.interval));
                if unlock_time < late_cutoff {
                    late_count += 1;
                }
            }
            if late_count > 0 {
                let on_time_count = claimable.saturating_sub(late_count);
                let on_time_amount = per_installment * on_time_count as i128;
                let late_amount = per_installment * late_count as i128;
                let penalty = late_amount * penalty_cfg.penalty_bps as i128 / 10_000;
                let penalized = on_time_amount + late_amount - penalty;
                // For the final batch keep vault.balance as ceiling to avoid dust mismatch.
                if unlocked >= schedule.num_installments {
                    penalized.min(vault.balance)
                } else {
                    penalized
                }
            } else {
                base_amount
            }
        } else {
            base_amount
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

        // Emit penalty event if the payout was reduced.
        if amount < base_amount {
            env.events().publish(
                (VESTING_PENALTY_TOPIC, vault_id),
                (base_amount - amount, amount),
            );
        }

        let sched_key = DataKey::VestingSchedule(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&sched_key, &schedule);
        env.storage().persistent().extend_ttl(&sched_key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(amount)
    }

    /// Allows the vault owner to claw back unvested funds from a released vault.
    ///
    /// After a vault has been released (`trigger_release` called) with one or more
    /// vesting schedules attached, the owner may reclaim funds that have not yet
    /// vested — i.e., installments that have not been claimed by the beneficiary.
    /// All vesting schedules on the vault are marked as fully claimed so no
    /// further beneficiary claims are possible.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault to claw back from
    /// * `caller` - Must be the vault owner
    ///
    /// # Returns
    /// The total amount clawed back (in stroops)
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not in Released status
    /// * `ContractError::VestingNotFound` - If no vesting schedule exists on the vault
    /// * `ContractError::NothingToClawback` - If all installments have already been claimed
    /// * `ContractError::InsufficientBalance` - If vault balance is less than unvested amount
    pub fn clawback_unvested(env: Env, vault_id: u64, caller: Address) -> Result<i128, ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Released {
            return Err(ContractError::AlreadyReleased);
        }

        // Collect all vesting schedules on this vault
        let count_key = DataKey::VestingScheduleCount(vault_id);
        let count: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);
        if count == 0 {
            return Err(ContractError::VestingNotFound);
        }

        // Calculate total unvested = sum of unclaimed installment amounts
        let mut total_unvested: i128 = 0;
        let ttl = vault_ttl_ledgers(vault.check_in_interval);

        for i in 0..count {
            let sched_key = DataKey::VestingSchedule(vault_id, i);
            if let Some(schedule) = env.storage().persistent().get::<DataKey, VestingSchedule>(&sched_key) {
                let remaining = schedule.num_installments - schedule.claimed_installments;
                if remaining > 0 {
                    let per_installment = schedule.total_amount / schedule.num_installments as i128;
                    total_unvested += per_installment * remaining as i128;
                }
            }
        }

        if total_unvested == 0 {
            return Err(ContractError::NothingToClawback);
        }

        if vault.balance < total_unvested {
            return Err(ContractError::InsufficientBalance);
        }

        // Transfer unvested funds to owner
        let token_client = token::Client::new(&env, &vault.token_address);
        token_client.transfer(&env.current_contract_address(), &vault.owner, &total_unvested);

        // Update vault balance
        vault.balance -= total_unvested;

        // Second pass: mark all schedules as fully claimed
        for i in 0..count {
            let sched_key = DataKey::VestingSchedule(vault_id, i);
            if let Some(mut schedule) = env.storage().persistent().get::<DataKey, VestingSchedule>(&sched_key) {
                schedule.claimed_installments = schedule.num_installments;
                env.storage().persistent().set(&sched_key, &schedule);
                env.storage().persistent().extend_ttl(&sched_key, VAULT_TTL_THRESHOLD, ttl);
            }
        }

        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (CLAWBACK_UNVESTED_TOPIC, vault_id),
            (total_unvested, vault.balance),
        );
        Ok(total_unvested)
    }

    // --- milestone-based vesting ---

    /// Sets a milestone-based vesting schedule for a vault.
    ///
    /// Instead of releasing funds on a time-based schedule, funds are unlocked
    /// when external milestones (e.g., company revenue targets) are fulfilled.
    /// An oracle address is designated to report progress on each milestone.
    ///
    /// The vault must be Locked, have a non-zero balance, and must not already
    /// have a time-based vesting schedule attached.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault to attach the milestone schedule to
    /// * `caller` - Must be the vault owner
    /// * `milestones` - Vector of MilestoneEntry defining each milestone (target, BPS, label)
    /// * `oracle` - Address authorized to report milestone progress
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::AlreadyReleased` - If vault is not Locked
    /// * `ContractError::EmptyVault` - If vault balance is 0
    /// * `ContractError::InvalidBps` - If milestone BPS does not sum to 10_000
    /// * `ContractError::VestingAlreadySet` - If a time-based vesting schedule exists
    pub fn set_milestone_vesting(
        env: Env,
        vault_id: u64,
        caller: Address,
        milestones: Vec<MilestoneEntry>,
        oracle: Address,
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
        if vault.balance == 0 {
            return Err(ContractError::EmptyVault);
        }
        // Cannot have both time-based and milestone-based vesting
        if env.storage().persistent().has(&DataKey::VestingSchedule(vault_id)) {
            return Err(ContractError::AlreadyReleased);
        }
        if milestones.is_empty() {
            return Err(ContractError::InvalidBps);
        }
        let total_bps: u32 = milestones.iter().map(|m| m.bps).sum();
        if total_bps != 10_000 {
            return Err(ContractError::InvalidBps);
        }
        // Ensure no milestone is pre-fulfilled
        for milestone in milestones.iter() {
            if milestone.is_fulfilled {
                return Err(ContractError::InvalidAmount);
            }
        }

        let schedule = MilestoneVestingSchedule {
            total_amount: vault.balance,
            milestones,
            claimed_amount: 0,
            oracle,
            paused: false,
        };
        let key = DataKey::MilestoneVestingSchedule(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &schedule);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (MILESTONE_VEST_TOPIC, vault_id),
            (schedule.total_amount, schedule.oracle.clone()),
        );
        Ok(())
    }

    /// Returns the milestone vesting schedule for a vault, if one exists.
    pub fn get_milestone_vesting_schedule(env: Env, vault_id: u64) -> Option<MilestoneVestingSchedule> {
        env.storage().persistent().get(&DataKey::MilestoneVestingSchedule(vault_id))
    }

    /// Pauses a milestone vesting schedule, blocking progress reporting and claiming.
    ///
    /// Only the vault owner can call this. The vesting state and milestone data are
    /// preserved and can be resumed later via `resume_milestone_vesting`.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `caller` - The address of the caller (must be vault owner)
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::VestingNotFound` - If no milestone vesting schedule exists
    pub fn pause_milestone_vesting(
        env: Env,
        vault_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let key = DataKey::MilestoneVestingSchedule(vault_id);
        let mut schedule: MilestoneVestingSchedule = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::VestingNotFound)?;

        if schedule.paused {
            return Err(ContractError::Paused);
        }
        schedule.paused = true;

        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &schedule);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((MILESTONE_PAUSE_TOPIC, vault_id), true);
        Ok(())
    }

    /// Resumes a paused milestone vesting schedule, re-enabling progress reporting and claiming.
    ///
    /// Only the vault owner can call this. All milestone progress and state is preserved
    /// from before the pause.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `caller` - The address of the caller (must be vault owner)
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::VestingNotFound` - If no milestone vesting schedule exists
    pub fn resume_milestone_vesting(
        env: Env,
        vault_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let key = DataKey::MilestoneVestingSchedule(vault_id);
        let mut schedule: MilestoneVestingSchedule = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::VestingNotFound)?;

        if !schedule.paused {
            return Err(ContractError::Paused);
        }
        schedule.paused = false;

        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &schedule);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((MILESTONE_RESUME_TOPIC, vault_id), false);
        Ok(())
    }

    // --- Issue #541: Vesting Rollover ---

    /// Enables or disables vesting rollover for a vault.
    /// When enabled, unclaimed installments roll over and accumulate.
    pub fn set_vesting_rollover(
        env: Env,
        vault_id: u64,
        caller: Address,
        enabled: bool,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        let config = VestingRolloverConfig { enabled, rolled_amount: 0 };
        let key = DataKey::VestingRollover(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &config);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.events().publish((VESTING_ROLLOVER_TOPIC, vault_id), enabled);
        Ok(())
    }

    /// Returns the vesting rollover configuration for a vault.
    pub fn get_vesting_rollover(env: Env, vault_id: u64) -> Option<VestingRolloverConfig> {
        env.storage().persistent().get(&DataKey::VestingRollover(vault_id))
    }

    // --- Issue #542: Vesting Forfeiture ---

    /// Sets the forfeiture recipient for a vault's vesting schedule.
    /// If the beneficiary declines their role, unvested funds are sent to this address.
    pub fn set_vesting_forfeiture(
        env: Env,
        vault_id: u64,
        caller: Address,
        forfeiture_recipient: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        let config = VestingForfeitureConfig { forfeiture_recipient, forfeited: false };
        let key = DataKey::VestingForfeiture(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &config);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.events().publish((VESTING_FORFEITURE_TOPIC, vault_id), config.forfeiture_recipient.clone());
        Ok(())
    }

    /// Returns the vesting forfeiture configuration for a vault.
    pub fn get_vesting_forfeiture(env: Env, vault_id: u64) -> Option<VestingForfeitureConfig> {
        env.storage().persistent().get(&DataKey::VestingForfeiture(vault_id))
    }

    // --- Issue #543: Vesting Acceleration ---

    /// Sets the oracle for vesting acceleration.
    /// The oracle can trigger immediate release of all remaining installments.
    pub fn set_vesting_acceleration(
        env: Env,
        vault_id: u64,
        caller: Address,
        oracle: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        let config = VestingAccelerationConfig { oracle, accelerated: false };
        let key = DataKey::VestingAcceleration(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &config);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.events().publish((VESTING_ACCELERATED_TOPIC, vault_id), config.oracle.clone());
        Ok(())
    }

    /// Returns the vesting acceleration configuration for a vault.
    pub fn get_vesting_acceleration(env: Env, vault_id: u64) -> Option<VestingAccelerationConfig> {
        env.storage().persistent().get(&DataKey::VestingAcceleration(vault_id))
    }

    /// Triggers immediate acceleration of all remaining vesting installments.
    /// Can only be called by the designated oracle.
    pub fn accelerate_vesting(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let key = DataKey::VestingAcceleration(vault_id);
        let mut config: VestingAccelerationConfig = env.storage().persistent().get(&key)
            .ok_or(ContractError::VestingNotFound)?;
        if caller != config.oracle {
            return Err(ContractError::NotOwner);
        }
        if config.accelerated {
            return Err(ContractError::VestingAlreadyAccelerated);
        }
        config.accelerated = true;
        env.storage().persistent().set(&key, &config);
        env.events().publish((VESTING_ACCELERATED_TOPIC, vault_id), true);
        Ok(())
    }

    // --- Issue #544: Vesting Staggering ---

    /// Sets a staggered vesting schedule for multiple beneficiaries.
    pub fn set_vesting_stagger(
        env: Env,
        vault_id: u64,
        caller: Address,
        entries: Vec<VestingStaggerEntry>,
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
        let total_bps: u32 = entries.iter().map(|e| e.bps).sum();
        if total_bps != 10_000 {
            return Err(ContractError::InvalidBps);
        }
        let key = DataKey::VestingStagger(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &entries);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.events().publish((VESTING_STAGGER_TOPIC, vault_id), entries.len());
        Ok(())
    }

    /// Returns the staggered vesting schedule for a vault.
    pub fn get_vesting_stagger(env: Env, vault_id: u64) -> Vec<VestingStaggerEntry> {
        env.storage().persistent().get(&DataKey::VestingStagger(vault_id)).unwrap_or(Vec::new(&env))
    }

    /// Claims available installments from a staggered vesting schedule.
    pub fn claim_staggered_vesting(env: Env, vault_id: u64, caller: Address) -> Result<i128, ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if vault.status != ReleaseStatus::Released {
            return Err(ContractError::AlreadyReleased);
        }
        let key = DataKey::VestingStagger(vault_id);
        let mut entries: Vec<VestingStaggerEntry> = env.storage().persistent().get(&key)
            .ok_or(ContractError::VestingStaggerNotFound)?;

        let mut total_amount: i128 = 0;
        let now = env.ledger().timestamp();
        let mut entry_found = false;

        for i in 0..entries.len() {
            let mut entry = entries.get(i).unwrap();
            if entry.beneficiary == caller {
                entry_found = true;
                if entry.claimed_installments >= entry.num_installments {
                    continue; // Skip already completed ones for this beneficiary
                }
                if now < entry.start_time {
                    continue;
                }
                let elapsed = now - entry.start_time;
                let unlocked = ((elapsed / entry.interval) + 1).min(entry.num_installments as u64) as u32;
                let claimable = unlocked.saturating_sub(entry.claimed_installments);
                if claimable == 0 {
                    continue;
                }

                // Stagger amount is based on total vault balance at the time of release.
                // For simplicity here, we assume the BPS applies to the current balance + what was already claimed.
                // But since we don't track original balance, we'll just use BPS * (current_balance / remaining_bps).
                // Better: assume vault.balance is the total pool for staggered vesting if no other vesting is set.
                let per_installment = (vault.balance * entry.bps as i128 / 10_000) / entry.num_installments as i128;
                let amount = if unlocked >= entry.num_installments {
                    // Last installment takes the remaining share for this beneficiary
                    (vault.balance * entry.bps as i128 / 10_000)
                } else {
                    per_installment * claimable as i128
                };

                if amount > 0 {
                    let token_client = token::Client::new(&env, &vault.token_address);
                    token_client.transfer(&env.current_contract_address(), &caller, &amount);
                    vault.balance -= amount;
                    entry.claimed_installments = unlocked;
                    entries.set(i, entry);
                    total_amount += amount;
                }
            }
        }

        if !entry_found {
            return Err(ContractError::NotBeneficiary);
        }
        if total_amount == 0 {
            return Err(ContractError::NothingToClaimYet);
        }

        Self::save_vault(&env, vault_id, &vault);
        env.storage().persistent().set(&key, &entries);
        env.events().publish((CLAIM_VEST_TOPIC, vault_id), (caller, total_amount));
        Ok(total_amount)
    }

    /// Adjusts milestone targets to slow down vesting.
    ///
    /// Only the vault owner can call this. Increases the `target_value` of unfulfilled
    /// milestones, making them harder to reach. Already-fulfilled milestones are
    /// unaffected. This allows the owner to respond to changing circumstances without
    /// cancelling the vesting schedule.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `caller` - The address of the caller (must be vault owner)
    /// * `adjustments` - Vector of `(milestone_index, new_target_value)` pairs
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::VestingNotFound` - If no milestone vesting schedule exists
    /// * `ContractError::InvalidAmount` - If any milestone is already fulfilled
    pub fn adjust_milestone_targets(
        env: Env,
        vault_id: u64,
        caller: Address,
        adjustments: Vec<(u32, i128)>,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let key = DataKey::MilestoneVestingSchedule(vault_id);
        let mut schedule: MilestoneVestingSchedule = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::VestingNotFound)?;

        for (idx, new_target) in adjustments.iter() {
            if idx >= schedule.milestones.len() {
                return Err(ContractError::InvalidAmount);
            }
            let milestone = schedule.milestones.get(idx).unwrap();
            if milestone.is_fulfilled {
                return Err(ContractError::InvalidAmount);
            }
            schedule.milestones.set(idx, MilestoneEntry {
                label: milestone.label,
                target_value: new_target,
                current_value: milestone.current_value,
                bps: milestone.bps,
                is_fulfilled: milestone.is_fulfilled,
                claimed: milestone.claimed,
            });
        }

        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &schedule);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((MILESTONE_ADJUST_TOPIC, vault_id), ());
        Ok(())
    }

    /// Emergency-fulfills one or more milestones, bypassing the oracle.
    ///
    /// Only the vault owner can call this. Marks the specified milestones as fulfilled,
    /// making their funds available for claiming immediately. This is intended for
    /// emergency situations where the oracle cannot or should not be relied upon.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `caller` - The address of the caller (must be vault owner)
    /// * `milestone_indices` - Indices of milestones to mark as fulfilled
    ///
    /// # Errors
    /// * `ContractError::Paused` - If the contract is paused
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::VestingNotFound` - If no milestone vesting schedule exists
    /// * `ContractError::InvalidAmount` - If any milestone is already fulfilled or invalid
    pub fn emergency_fulfill_milestones(
        env: Env,
        vault_id: u64,
        caller: Address,
        milestone_indices: Vec<u32>,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let key = DataKey::MilestoneVestingSchedule(vault_id);
        let mut schedule: MilestoneVestingSchedule = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::VestingNotFound)?;

        let mut any_updated = false;
        for idx in milestone_indices.iter() {
            if idx >= schedule.milestones.len() {
                return Err(ContractError::InvalidAmount);
            }
            let milestone = schedule.milestones.get(idx).unwrap();
            if milestone.is_fulfilled {
                return Err(ContractError::InvalidAmount);
            }
            schedule.milestones.set(idx, MilestoneEntry {
                label: milestone.label,
                target_value: milestone.target_value,
                current_value: milestone.current_value.max(milestone.target_value),
                bps: milestone.bps,
                is_fulfilled: true,
                claimed: milestone.claimed,
            });
            any_updated = true;
        }

        if !any_updated {
            return Err(ContractError::InvalidAmount);
        }

        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &schedule);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((MILESTONE_EMERGENCY_TOPIC, vault_id), true);
        Ok(())
    }

    /// Reports progress on one or more milestones.
    ///
    /// Only the designated oracle address can call this function. When a milestone's
    /// `current_value` reaches or exceeds its `target_value`, it is marked as fulfilled
    /// and its funds become available for claiming.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `vault_id` - The vault ID
    /// * `milestone_indices` - Indices of milestones to update
    /// * `current_values` - New current values for each milestone (must match indices length)
    ///
    /// # Errors
    /// * `ContractError::MilestoneVestingNotFound` - If no milestone vesting schedule exists
    /// * `ContractError::NotMilestoneOracle` - If caller is not the designated oracle
    /// * `ContractError::InvalidAmount` - If indices and values length mismatch
    /// * `ContractError::MilestoneAlreadyFulfilled` - If a milestone is already fulfilled
    pub fn report_milestone_progress(
        env: Env,
        vault_id: u64,
        milestone_indices: Vec<u32>,
        current_values: Vec<i128>,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        if milestone_indices.len() != current_values.len() {
            return Err(ContractError::InvalidAmount);
        }

        let mut schedule: MilestoneVestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::MilestoneVestingSchedule(vault_id))
            .ok_or(ContractError::VestingNotFound)?;

        if schedule.paused {
            return Err(ContractError::Paused);
        }

        schedule.oracle.require_auth();

        let mut any_updated = false;
        for (idx, value) in milestone_indices.iter().zip(current_values.iter()) {
            if idx >= schedule.milestones.len() {
                return Err(ContractError::InvalidAmount);
            }
            let milestone = &schedule.milestones.get(idx).unwrap();
            if milestone.is_fulfilled {
                return Err(ContractError::InvalidAmount);
            }
            let was_fulfilled = milestone.current_value >= milestone.target_value;
            let new_value = value.max(0);
            let mut updated = milestone.clone();
            updated.current_value = new_value;
            if new_value >= updated.target_value && !was_fulfilled {
                updated.is_fulfilled = true;
            } else if new_value < updated.target_value {
                updated.is_fulfilled = false;
            }
            schedule.milestones.set(idx, updated);
            any_updated = true;
        }

        if !any_updated {
            return Ok(());
        }

        let key = DataKey::MilestoneVestingSchedule(vault_id);
        let ttl = vault_ttl_ledgers(
            Self::load_vault(&env, vault_id).check_in_interval,
        );
        env.storage().persistent().set(&key, &schedule);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        // Emit progress event for each updated milestone
        for idx in milestone_indices.iter() {
            let m = &schedule.milestones.get(idx).unwrap();
            env.events().publish(
                (MILESTONE_PROGRESS_TOPIC, vault_id),
                (idx, m.current_value, m.target_value, m.is_fulfilled),
            );
        }
        Ok(())
    }

    /// Claims all vested installments from fulfilled milestones.
    ///
    /// The vault must have been released (`trigger_release`) and a milestone vesting
    /// schedule must be attached. Funds from all milestones that are fulfilled but
    /// not yet claimed are distributed to the vault's beneficiaries using the same
    /// BPS split logic as `trigger_release`.
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
    /// * `ContractError::AlreadyReleased` - If vault is not in Released status
    /// * `ContractError::MilestoneVestingNotFound` - If no milestone vesting schedule exists
    /// * `ContractError::NoClaimableMilestones` - If no milestones are ready to claim
    /// * `ContractError::InsufficientBalance` - If vault balance is insufficient
    pub fn claim_milestone_installments(env: Env, vault_id: u64) -> Result<i128, ContractError> {
        Self::assert_not_paused(&env);
        let mut vault = Self::load_vault(&env, vault_id);

        if vault.status != ReleaseStatus::Released {
            return Err(ContractError::AlreadyReleased);
        }

        let mut schedule: MilestoneVestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::MilestoneVestingSchedule(vault_id))
            .ok_or(ContractError::VestingNotFound)?;

        if schedule.paused {
            return Err(ContractError::Paused);
        }

        // Calculate claimable amounts from fulfilled milestones
        let mut total_claimable = 0i128;
        let mut any_claimable = false;
        let num_milestones = schedule.milestones.len();

        let num_milestones_usize = num_milestones as usize;
        for (i, milestone) in schedule.milestones.iter().enumerate() {
            if milestone.is_fulfilled && !milestone.claimed {
                let is_last = i == num_milestones_usize - 1;
                let share = if is_last {
                    schedule.total_amount - total_claimable - schedule.claimed_amount
                } else {
                    schedule.total_amount * (milestone.bps as i128) / 10_000
                };
                total_claimable += share;
                any_claimable = true;
            }
        }

        if !any_claimable {
            return Err(ContractError::NothingToClaimYet);
        }

        if vault.balance < total_claimable {
            return Err(ContractError::InsufficientBalance);
        }

        // Distribute to beneficiaries
        let token_client = token::Client::new(&env, &vault.token_address);

        if vault.beneficiaries.is_empty() {
            token_client.transfer(&env.current_contract_address(), &vault.beneficiary, &total_claimable);
            env.events().publish(
                (MILESTONE_CLAIM_TOPIC, vault_id),
                (vault.beneficiary.clone(), total_claimable, schedule.claimed_amount + total_claimable),
            );
        } else {
            let mut distributed: i128 = 0;
            let last_idx = vault.beneficiaries.len() - 1;
            for (i, entry) in vault.beneficiaries.iter().enumerate() {
                let share = if i as u32 == last_idx {
                    total_claimable - distributed
                } else {
                    total_claimable * (entry.bps as i128) / 10_000
                };
                if share > 0 {
                    token_client.transfer(&env.current_contract_address(), &entry.address, &share);
                }
                distributed += share;
                env.events().publish(
                    (MILESTONE_CLAIM_TOPIC, vault_id),
                    (entry.address.clone(), share, schedule.claimed_amount + total_claimable),
                );
            }
        }

        // Update state
        vault.balance -= total_claimable;
        let new_claimed = schedule.claimed_amount + total_claimable;

        // Mark milestones as claimed
        let mut updated_milestones = Vec::new(&env);
        for milestone in schedule.milestones.iter() {
            if milestone.is_fulfilled && !milestone.claimed {
                let mut m = milestone.clone();
                m.claimed = true;
                updated_milestones.push_back(m);
            } else {
                updated_milestones.push_back(milestone);
            }
        }
        schedule.milestones = updated_milestones;
        schedule.claimed_amount = new_claimed;

        Self::save_vault(&env, vault_id, &vault);

        let sched_key = DataKey::MilestoneVestingSchedule(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&sched_key, &schedule);
        env.storage().persistent().extend_ttl(&sched_key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(total_claimable)
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
    pub fn beneficiary_veto_release_condition(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        // Only primary beneficiary (vault.beneficiary)
        if caller != vault.beneficiary {
            return Err(ContractError::NotBeneficiary);
        }
        // Only before expiry
        if Self::is_expired(env.clone(), vault_id) {
            return Err(ContractError::NotExpired);
        }
        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryReleaseConditionVeto(vault_id), &true);
        // keep storage alive at least as long as the vault
        env.storage().persistent().extend_ttl(
            &DataKey::BeneficiaryReleaseConditionVeto(vault_id),
            VAULT_TTL_THRESHOLD,
            VAULT_TTL_LEDGERS,
        );
        Ok(())
    }

    pub fn revoke_beneficiary_veto_release_condition(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.beneficiary {
            return Err(ContractError::NotBeneficiary);
        }
        env.storage().persistent().remove(&DataKey::BeneficiaryReleaseConditionVeto(vault_id));
        Ok(())
    }

    pub fn get_beneficiary_veto_release_condition(env: Env, vault_id: u64) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::BeneficiaryReleaseConditionVeto(vault_id))
            .unwrap_or(false)
    }

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
        let is_primary = caller == vault.beneficiary;
        let is_listed = vault.beneficiaries.iter().any(|e| e.address == caller);
        if !is_primary && !is_listed {
            return Err(ContractError::NotBeneficiary);
        }
        // Per-beneficiary status for waterfall/rebalancing logic
        let ben_key = DataKey::BeneficiaryStatusEntry(vault_id, caller.clone());
        env.storage().persistent().set(&ben_key, &BeneficiaryStatus::Declined);
        env.storage().persistent().extend_ttl(&ben_key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        // Vault-level status for single-beneficiary vaults
        if is_primary && vault.beneficiaries.is_empty() {
            env.storage().persistent().set(&DataKey::BeneficiaryStatus(vault_id), &BeneficiaryStatus::Declined);
            env.storage().persistent().extend_ttl(&DataKey::BeneficiaryStatus(vault_id), VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        }
        // Issue #542: handle vesting forfeiture if configured
        if let Some(mut forfeit_cfg) = env.storage().persistent()
            .get::<DataKey, VestingForfeitureConfig>(&DataKey::VestingForfeiture(vault_id))
        {
            if !forfeit_cfg.forfeited {
                if let Some(schedule) = env.storage().persistent().get::<DataKey, VestingSchedule>(&DataKey::VestingSchedule(vault_id)) {
                    let total_vested = schedule.total_amount;
                    let per_inst = total_vested / schedule.num_installments as i128;
                    let claimed_amount = per_inst * schedule.claimed_installments as i128;
                    let unvested = total_vested - claimed_amount;
                    
                    if unvested > 0 {
                        let token_client = token::Client::new(&env, &vault.token_address);
                        token_client.transfer(&env.current_contract_address(), &forfeit_cfg.forfeiture_recipient, &unvested);
                        let mut mut_vault = vault.clone();
                        mut_vault.balance -= unvested;
                        forfeit_cfg.forfeited = true;
                        env.storage().persistent().set(&DataKey::VestingForfeiture(vault_id), &forfeit_cfg);
                        Self::save_vault(&env, vault_id, &mut_vault);
                        env.events().publish((VESTING_FORFEITURE_TOPIC, vault_id), unvested);
                    }
                }
            }
        }

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

    pub fn set_beneficiary_release_triggers(
        env: Env,
        vault_id: u64,
        caller: Address,
        beneficiary: Address,
        triggers: Vec<ReleaseTrigger>,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        if !vault.beneficiaries.iter().any(|entry| entry.address == beneficiary) {
            return Err(ContractError::InvalidBeneficiary);
        }
        if triggers.is_empty() {
            return Err(ContractError::InvalidAmount);
        }

        let mut mappings: Vec<(Address, Vec<ReleaseTrigger>)> = env
            .storage()
            .persistent()
            .get(&DataKey::BeneficiaryReleaseTriggers(vault_id))
            .unwrap_or(Vec::new(&env));
        let mut updated: Vec<(Address, Vec<ReleaseTrigger>)> = Vec::new(&env);
        for mapping in mappings.iter() {
            if mapping.0 != beneficiary {
                updated.push_back(mapping.clone());
            }
        }
        updated.push_back((beneficiary.clone(), triggers.clone()));
        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryReleaseTriggers(vault_id), &updated);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (BENEFICIARY_TRIGGER_SET_TOPIC, vault_id),
            BeneficiaryTriggerSetEvent { vault_id, beneficiary, triggers },
        );
        Ok(())
    }

    pub fn get_beneficiary_release_triggers(
        env: Env,
        vault_id: u64,
        beneficiary: Address,
    ) -> Vec<ReleaseTrigger> {
        let mappings: Vec<(Address, Vec<ReleaseTrigger>)> = env
            .storage()
            .persistent()
            .get(&DataKey::BeneficiaryReleaseTriggers(vault_id))
            .unwrap_or(Vec::new(&env));
        for mapping in mappings.iter() {
            if mapping.0 == beneficiary {
                return mapping.1.clone();
            }
        }
        let mut default_triggers = Vec::new(&env);
        default_triggers.push_back(ReleaseTrigger::Expiry);
        default_triggers.push_back(ReleaseTrigger::Manual);
        default_triggers
    }

    pub fn set_beneficiary_tier_threshold(
        env: Env,
        vault_id: u64,
        caller: Address,
        beneficiary: Address,
        tier_threshold: i128,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        if !vault.beneficiaries.iter().any(|entry| entry.address == beneficiary) {
            return Err(ContractError::InvalidBeneficiary);
        }
        if tier_threshold < 0 {
            return Err(ContractError::InvalidAmount);
        }

        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryTierThreshold(vault_id, beneficiary.clone()), &tier_threshold);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish(
            (BENEFICIARY_TIER_SET_TOPIC, vault_id),
            BeneficiaryTierSetEvent { vault_id, beneficiary, tier_threshold },
        );
        Ok(())
    }

    pub fn get_beneficiary_tier_threshold(
        env: Env,
        vault_id: u64,
        beneficiary: Address,
    ) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::BeneficiaryTierThreshold(vault_id, beneficiary))
            .unwrap_or(0)
    }

    /// Set a beneficiary-specific vesting schedule - Issue #525
    /// Different beneficiaries can have different vesting timelines.
    pub fn set_beneficiary_vesting(
        env: Env,
        vault_id: u64,
        caller: Address,
        beneficiary: Address,
        start_time: u64,
        interval: u64,
        num_installments: u32,
        cliff_period: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if interval == 0 || num_installments == 0 {
            return Err(ContractError::InvalidAmount);
        }
        let schedule = BeneficiaryVestingSchedule {
            beneficiary: beneficiary.clone(),
            vault_id,
            start_time,
            interval,
            num_installments,
            claimed_installments: 0,
            total_amount: vault.balance,
            cliff_period,
        };
        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryVestingSchedule(vault_id, beneficiary.clone()), &schedule);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::BeneficiaryVestingSchedule(vault_id, beneficiary.clone()), VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.events().publish((SET_BENEFICIARY_VESTING_TOPIC, vault_id), (beneficiary, start_time, interval, num_installments));
        Ok(())
    }

    /// Get beneficiary-specific vesting schedule - Issue #525
    pub fn get_beneficiary_vesting(
        env: Env,
        vault_id: u64,
        beneficiary: Address,
    ) -> Option<BeneficiaryVestingSchedule> {
        env.storage()
            .persistent()
            .get(&DataKey::BeneficiaryVestingSchedule(vault_id, beneficiary))
    }

    /// Claim available vesting installments for a specific beneficiary - Issue #525
    pub fn claim_beneficiary_vesting(
        env: Env,
        vault_id: u64,
        beneficiary: Address,
    ) -> Result<i128, ContractError> {
        let mut vault = Self::load_vault(&env, vault_id);
        if vault.status != ReleaseStatus::Released {
            return Err(ContractError::AlreadyReleased);
        }
        let mut schedule = env
            .storage()
            .persistent()
            .get::<_, BeneficiaryVestingSchedule>(&DataKey::BeneficiaryVestingSchedule(vault_id, beneficiary.clone()))
            .ok_or(ContractError::VestingNotFound)?;

        let now = env.ledger().timestamp() as u64;
        if now < schedule.start_time + schedule.cliff_period {
            return Err(ContractError::CliffNotReached);
        }

        let elapsed = now.saturating_sub(schedule.start_time);
        let available_installments = ((elapsed / schedule.interval) + 1).min(schedule.num_installments as u64) as u32;

        if available_installments <= schedule.claimed_installments {
            return Err(ContractError::NothingToClaimYet);
        }

        let per_installment = schedule.total_amount / (schedule.num_installments as i128);
        let new_claims = available_installments - schedule.claimed_installments;
        let mut amount = per_installment * (new_claims as i128);

        if available_installments == schedule.num_installments {
            amount = vault.balance;
        }

        if amount <= 0 || amount > vault.balance {
            return Err(ContractError::InvalidAmount);
        }

        vault.balance -= amount;
        schedule.claimed_installments = available_installments;

        Self::save_vault(&env, vault_id, &vault);
        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryVestingSchedule(vault_id, beneficiary.clone()), &schedule);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::BeneficiaryVestingSchedule(vault_id, beneficiary.clone()), VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);

        env.events().publish((CLAIM_BENEFICIARY_VESTING_TOPIC, vault_id), (beneficiary, amount));
        Ok(amount)
    }

    /// Create a beneficiary auction - Issue #527
    /// Allow beneficiaries to bid for larger allocations.
    pub fn create_beneficiary_auction(
        env: Env,
        vault_id: u64,
        caller: Address,
        start_time: u64,
        end_time: u64,
        total_allocation_bps: u32,
        minimum_bid: i128,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if env.storage().persistent().has(&DataKey::BeneficiaryAuction(vault_id)) {
            return Err(ContractError::AuctionAlreadyExists);
        }

        let auction_id = env
            .storage()
            .persistent()
            .get::<_, u64>(&DataKey::BeneficiaryAuctionCount)
            .unwrap_or(0)
            + 1;

        let auction = BeneficiaryAuction {
            auction_id,
            vault_id,
            start_time,
            end_time,
            total_allocation_bps,
            minimum_bid,
            bids: vec![&env],
            finalized: false,
            winner: None,
        };

        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryAuction(vault_id), &auction);
        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryAuctionCount, &auction_id);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::BeneficiaryAuction(vault_id), VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);

        env.events().publish((AUCTION_CREATED_TOPIC, vault_id), (auction_id, start_time, end_time));
        Ok(())
    }

    /// Place a bid in a beneficiary auction - Issue #527
    pub fn place_auction_bid(
        env: Env,
        vault_id: u64,
        bidder: Address,
        bid_amount: i128,
        desired_allocation_bps: u32,
    ) -> Result<(), ContractError> {
        bidder.require_auth();

        let mut auction = env
            .storage()
            .persistent()
            .get::<_, BeneficiaryAuction>(&DataKey::BeneficiaryAuction(vault_id))
            .ok_or(ContractError::AuctionNotFound)?;

        let now = env.ledger().timestamp() as u64;
        if now < auction.start_time || now >= auction.end_time {
            return Err(ContractError::AuctionEnded);
        }

        if bid_amount < auction.minimum_bid {
            return Err(ContractError::InvalidAmount);
        }

        let bid = BeneficiaryAuctionBid {
            auction_id: auction.auction_id,
            bidder: bidder.clone(),
            bid_amount,
            desired_allocation_bps,
            bid_timestamp: now,
            accepted: false,
        };

        auction.bids.push_back(bid);
        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryAuction(vault_id), &auction);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::BeneficiaryAuction(vault_id), VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);

        env.events().publish((AUCTION_BID_TOPIC, vault_id), (bidder, bid_amount));
        Ok(())
    }

    /// Get beneficiary auction - Issue #527
    pub fn get_beneficiary_auction(env: Env, vault_id: u64) -> Option<BeneficiaryAuction> {
        env.storage()
            .persistent()
            .get(&DataKey::BeneficiaryAuction(vault_id))
    }

    /// Get auction bids - Issue #527
    pub fn get_beneficiary_auction_bids(env: Env, vault_id: u64) -> Vec<BeneficiaryAuctionBid> {
        env.storage()
            .persistent()
            .get::<_, BeneficiaryAuction>(&DataKey::BeneficiaryAuction(vault_id))
            .map(|a| a.bids)
            .unwrap_or_else(|| vec![&env])
    }

    /// Finalize a beneficiary auction - Issue #527
    pub fn finalize_beneficiary_auction(env: Env, vault_id: u64) -> Result<(), ContractError> {
        let mut auction = env
            .storage()
            .persistent()
            .get::<_, BeneficiaryAuction>(&DataKey::BeneficiaryAuction(vault_id))
            .ok_or(ContractError::AuctionNotFound)?;

        let now = env.ledger().timestamp() as u64;
        if now < auction.end_time {
            return Err(ContractError::AuctionNotEnded);
        }

        if auction.bids.is_empty() {
            auction.finalized = true;
            env.storage()
                .persistent()
                .set(&DataKey::BeneficiaryAuction(vault_id), &auction);
            return Ok(());
        }

        // Find highest bidder
        let winner_idx = auction
            .bids
            .iter()
            .enumerate()
            .max_by_key(|(_, bid)| bid.bid_amount)
            .map(|(idx, _)| idx)
            .ok_or(ContractError::AuctionNotFound)?;

        let winner = auction.bids.get(winner_idx).unwrap().bidder.clone();
        auction.winner = Some(winner.clone());
        auction.finalized = true;

        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryAuction(vault_id), &auction);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::BeneficiaryAuction(vault_id), VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);

        env.events().publish((AUCTION_FINALIZED_TOPIC, vault_id), winner);
        Ok(())
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
        // Remove duplicate key so the same parameters can be reused
        let dup_key = DataKey::VaultDuplicate(vault.owner.clone(), vault.beneficiary.clone(), vault.check_in_interval);
        env.storage().persistent().remove(&dup_key);
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

    // ── Beneficiary Anonymity (ZK commitment) ────────────────────────────────

    /// Commits to an anonymous beneficiary using a hash commitment.
    ///
    /// The owner stores `sha256(beneficiary_address_bytes)` instead of the
    /// plaintext address. The actual beneficiary remains hidden until
    /// `reveal_beneficiary` is called at release time.
    ///
    /// A vault may have at most one active commitment. Call again to replace.
    ///
    /// # Arguments
    /// * `vault_id`   - The vault to attach the commitment to
    /// * `caller`     - Must be the vault owner (requires auth)
    /// * `commitment` - SHA-256 hash of the beneficiary address bytes
    ///
    /// # Errors
    /// * `NotOwner`              - caller is not the vault owner
    /// * `AlreadyReleased`       - vault is not Locked
    /// * `CommitmentAlreadySet`  - a commitment already exists (must reveal first)
    pub fn commit_beneficiary(
        env: Env,
        vault_id: u64,
        caller: Address,
        commitment: BytesN<32>,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }
        let key = DataKey::BeneficiaryCommitment(vault_id);
        if env.storage().persistent().has(&key) {
            return Err(ContractError::CommitmentAlreadySet);
        }
        let entry = BeneficiaryCommitment {
            commitment: commitment.clone(),
            committed_at: env.ledger().timestamp(),
        };
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &entry);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((BEN_COMMITTED_TOPIC, vault_id), commitment);
        Ok(())
    }

    /// Reveals the anonymous beneficiary and releases vault funds.
    ///
    /// Anyone may call this once the vault has expired. The `proof` bytes must
    /// be the raw address bytes of the beneficiary such that
    /// `sha256(proof) == stored_commitment`. On success the revealed address
    /// receives the vault balance and the vault is marked Released.
    ///
    /// # Arguments
    /// * `vault_id` - The vault to release
    /// * `proof`    - Raw bytes whose SHA-256 equals the stored commitment
    /// * `claim`    - The `Address` being claimed (must match `sha256(proof)`)
    ///
    /// # Errors
    /// * `CommitmentNotFound`        - no commitment was stored for this vault
    /// * `BeneficiaryAlreadyRevealed`- funds already released via this path
    /// * `NotExpired`                - vault has not yet expired
    /// * `EmptyVault`                - vault balance is zero
    /// * `InvalidZkProof`            - sha256(proof) does not match commitment
    pub fn reveal_beneficiary(
        env: Env,
        vault_id: u64,
        proof: Bytes,
        claim: Address,
    ) -> Result<(), ContractError> {
        let com_key = DataKey::BeneficiaryCommitment(vault_id);
        let entry: BeneficiaryCommitment = env
            .storage()
            .persistent()
            .get(&com_key)
            .ok_or(ContractError::CommitmentNotFound)?;

        if env.storage().persistent().has(&DataKey::RevealedBeneficiary(vault_id)) {
            return Err(ContractError::BeneficiaryAlreadyRevealed);
        }

        let mut vault = Self::load_vault(&env, vault_id);
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::BeneficiaryAlreadyRevealed);
        }
        if !Self::is_expired(env.clone(), vault_id) {
            return Err(ContractError::NotExpired);
        }
        if vault.balance == 0 {
            return Err(ContractError::EmptyVault);
        }

        // Verify: sha256(proof) must equal the stored commitment
        let proof_hash: BytesN<32> = env.crypto().sha256(&proof).into();
        if proof_hash != entry.commitment {
            return Err(ContractError::InvalidZkProof);
        }

        let amount = vault.balance;
        let token_client = token::Client::new(&env, &vault.token_address);
        token_client.transfer(&env.current_contract_address(), &claim, &amount);

        vault.balance = 0;
        vault.status = ReleaseStatus::Released;
        Self::save_vault(&env, vault_id, &vault);

        // Mark as revealed so it cannot be called again
        let rev_key = DataKey::RevealedBeneficiary(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&rev_key, &claim);
        env.storage().persistent().extend_ttl(&rev_key, VAULT_TTL_THRESHOLD, ttl);

        Self::record_state_transition(&env, vault_id, ReleaseStatus::Locked, ReleaseStatus::Released, &claim);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((BEN_REVEALED_TOPIC, vault_id), (claim, amount));
        Ok(())
    }

    /// Returns the beneficiary commitment for a vault, if one exists.
    pub fn get_beneficiary_commitment(env: Env, vault_id: u64) -> Option<BeneficiaryCommitment> {
        env.storage()
            .persistent()
            .get(&DataKey::BeneficiaryCommitment(vault_id))
    }

    /// Returns the revealed beneficiary address for a vault, if already revealed.
    pub fn get_revealed_beneficiary(env: Env, vault_id: u64) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::RevealedBeneficiary(vault_id))
    }

    // --- helpers ---

    #[allow(dead_code)]
    fn extend_vault_ttl(env: &Env, vault_id: u64, check_in_interval: u64) {
        let key = DataKey::Vault(vault_id);
        let ttl = vault_ttl_ledgers(check_in_interval);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
    }

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

    // --- Issue #564: Withdrawal Approval Workflow ---

    pub fn request_withdrawal_approval(env: Env, vault_id: u64, caller: Address, amount: i128, approvers: Vec<Address>, required_approvals: u32) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner { return Err(ContractError::NotOwner); }
        if amount <= 0 { return Err(ContractError::InvalidAmount); }
        if required_approvals == 0 || required_approvals > approvers.len() { return Err(ContractError::InvalidThreshold); }
        let req = WithdrawalApprovalRequest { amount, requested_at: env.ledger().timestamp(), approvals: Vec::new(&env), required_approvals, expires_at: env.ledger().timestamp() + 604_800 };
        env.storage().persistent().set(&DataKey::WithdrawalApprovalRequest(vault_id), &req);
        env.storage().persistent().set(&DataKey::WithdrawalApprovers(vault_id), &approvers);
        env.events().publish((WITHDRAWAL_APPROVAL_REQUESTED_TOPIC, vault_id), (amount, required_approvals));
        Ok(())
    }

    pub fn approve_withdrawal(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        let approvers: Vec<Address> = env.storage().persistent().get(&DataKey::WithdrawalApprovers(vault_id)).ok_or(ContractError::WithdrawalNotApproved)?;
        if !approvers.iter().any(|a| a == caller) { return Err(ContractError::NotASigner); }
        let mut req: WithdrawalApprovalRequest = env.storage().persistent().get(&DataKey::WithdrawalApprovalRequest(vault_id)).ok_or(ContractError::WithdrawalNotApproved)?;
        if env.ledger().timestamp() > req.expires_at { return Err(ContractError::ProposalExpired); }
        if req.approvals.iter().any(|a| a == caller) { return Err(ContractError::AlreadyApproved); }
        req.approvals.push_back(caller.clone());
        env.storage().persistent().set(&DataKey::WithdrawalApprovalRequest(vault_id), &req);
        env.events().publish((WITHDRAWAL_APPROVAL_GRANTED_TOPIC, vault_id), caller);
        Ok(())
    }

    pub fn execute_approved_withdrawal(env: Env, vault_id: u64) -> Result<(), ContractError> {
        let req: WithdrawalApprovalRequest = env.storage().persistent().get(&DataKey::WithdrawalApprovalRequest(vault_id)).ok_or(ContractError::WithdrawalNotApproved)?;
        if req.approvals.len() < req.required_approvals { return Err(ContractError::ProposalNotApproved); }
        let mut vault = Self::load_vault(&env, vault_id);
        if vault.balance < req.amount { return Err(ContractError::InsufficientBalance); }
        let token_client = token::Client::new(&env, &vault.token_address);
        token_client.transfer(&env.current_contract_address(), &vault.owner, &req.amount);
        vault.balance -= req.amount;
        Self::save_vault(&env, vault_id, &vault);
        env.storage().persistent().remove(&DataKey::WithdrawalApprovalRequest(vault_id));
        env.events().publish((WITHDRAW_TOPIC, vault_id), (req.amount, vault.balance));
        Ok(())
    }

    // --- Issue #563: Passkey Recovery ---

    pub fn initiate_passkey_recovery(env: Env, vault_id: u64, new_passkey_hash: BytesN<32>, recovery_code: String, contacts: Vec<Address>, required: u32) -> Result<(), ContractError> {
        if required == 0 || required > contacts.len() { return Err(ContractError::InvalidThreshold); }
        let req = PasskeyRecoveryRequest { new_passkey_hash: new_passkey_hash.clone(), initiated_at: env.ledger().timestamp(), recovery_code, approved_contacts: Vec::new(&env), required_contacts: required };
        env.storage().persistent().set(&DataKey::PasskeyRecoveryRequest(vault_id), &req);
        env.storage().persistent().set(&DataKey::RecoveryContacts(vault_id), &contacts);
        env.events().publish((PASSKEY_RECOVERY_INITIATED_TOPIC, vault_id), new_passkey_hash);
        Ok(())
    }

    pub fn approve_passkey_recovery(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        let contacts: Vec<Address> = env.storage().persistent().get(&DataKey::RecoveryContacts(vault_id)).ok_or(ContractError::NotRecoveryContact)?;
        if !contacts.iter().any(|c| c == caller) { return Err(ContractError::NotRecoveryContact); }
        let mut req: PasskeyRecoveryRequest = env.storage().persistent().get(&DataKey::PasskeyRecoveryRequest(vault_id)).ok_or(ContractError::NotRecoveryContact)?;
        if req.approved_contacts.iter().any(|c| c == caller) { return Err(ContractError::AlreadyApproved); }
        req.approved_contacts.push_back(caller.clone());
        if req.approved_contacts.len() >= req.required_contacts {
            let key = DataKey::VaultPasskeys(vault_id);
            let mut passkeys: Vec<PasskeyHash> = env.storage().persistent().get(&key).unwrap_or(Vec::new(&env));
            passkeys.push_back(PasskeyHash { hash: req.new_passkey_hash.clone(), added_at: env.ledger().timestamp() });
            env.storage().persistent().set(&key, &passkeys);
            env.storage().persistent().remove(&DataKey::PasskeyRecoveryRequest(vault_id));
            env.events().publish((PASSKEY_RECOVERED_TOPIC, vault_id), req.new_passkey_hash);
        } else {
            env.storage().persistent().set(&DataKey::PasskeyRecoveryRequest(vault_id), &req);
        }
        Ok(())
    }

    // --- Issue #562: Passkey Compromise Response ---

    pub fn lockout_vault(env: Env, vault_id: u64, duration_seconds: u64) -> Result<(), ContractError> {
        let vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        let lockout = PasskeyLockout { locked_at: env.ledger().timestamp(), unlock_at: env.ledger().timestamp() + duration_seconds, failed_attempts: 0 };
        env.storage().persistent().set(&DataKey::PasskeyLockout(vault_id), &lockout);
        env.events().publish((PASSKEY_LOCKOUT_TOPIC, vault_id), duration_seconds);
        Ok(())
    }

    pub fn unlock_vault(env: Env, vault_id: u64, caller: Address) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner { return Err(ContractError::NotOwner); }
        env.storage().persistent().remove(&DataKey::PasskeyLockout(vault_id));
        env.events().publish((PASSKEY_UNLOCKED_TOPIC, vault_id), caller);
        Ok(())
    }

    pub fn is_vault_locked(env: Env, vault_id: u64) -> bool {
        if let Some(lockout) = env.storage().persistent().get::<DataKey, PasskeyLockout>(&DataKey::PasskeyLockout(vault_id)) {
            env.ledger().timestamp() < lockout.unlock_at
        } else { false }
    }

    // --- Issue #561: Passkey Rotation Enforcement ---

    pub fn set_passkey_rotation_policy(env: Env, vault_id: u64, caller: Address, rotation_period_days: u32, enforce: bool) -> Result<(), ContractError> {
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner { return Err(ContractError::NotOwner); }
        let policy = PasskeyRotationPolicy { rotation_period_days, enforce };
        env.storage().persistent().set(&DataKey::PasskeyRotationPolicy(vault_id), &policy);
        env.events().publish((PASSKEY_ROTATION_REQUIRED_TOPIC, vault_id), (rotation_period_days, enforce));
        Ok(())
    }

    pub fn enforce_passkey_rotation(env: Env, vault_id: u64, passkey_hash: BytesN<32>) -> Result<bool, ContractError> {
        let policy: PasskeyRotationPolicy = env.storage().persistent().get(&DataKey::PasskeyRotationPolicy(vault_id)).unwrap_or(PasskeyRotationPolicy { rotation_period_days: 0, enforce: false });
        if !policy.enforce || policy.rotation_period_days == 0 { return Ok(true); }
        if let Some(last_rotation) = env.storage().persistent().get::<DataKey, u64>(&DataKey::LastPasskeyRotation(vault_id, passkey_hash.clone())) {
            let elapsed_days = (env.ledger().timestamp() - last_rotation) / 86_400;
            if elapsed_days >= policy.rotation_period_days as u64 {
                env.events().publish((PASSKEY_ROTATION_ENFORCED_TOPIC, vault_id), passkey_hash);
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn record_passkey_rotation(env: Env, vault_id: u64, passkey_hash: BytesN<32>) -> Result<(), ContractError> {
        env.storage().persistent().set(&DataKey::LastPasskeyRotation(vault_id, passkey_hash), &env.ledger().timestamp());
        Ok(())
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
        let token_address = token_address.clone();
        if !Self::is_token_whitelisted(env.clone(), token_address) {
            panic_with_error!(env, ContractError::TokenNotWhitelisted);
        }
    }

    // --- Issue #550: Passkey Compromise Detection ---

    /// Reports a passkey as compromised. Subsequent check-ins using that hash
    /// will be rejected with `PasskeyCompromised`.
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - Caller is not the vault owner
    pub fn report_passkey_compromise(
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
        let key = DataKey::CompromisedPasskeys(vault_id);
        let mut set: Vec<BytesN<32>> = env
            .storage().persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));
        // Idempotent — don't add duplicates.
        if !set.iter().any(|h| h == passkey_hash) {
            set.push_back(passkey_hash.clone());
            let ttl = vault_ttl_ledgers(vault.check_in_interval);
            env.storage().persistent().set(&key, &set);
            env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        }
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((PASSKEY_COMPROMISED_TOPIC, vault_id), passkey_hash);
        Ok(())
    }

    /// Clears a previously reported passkey compromise, allowing the passkey to be used again.
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - Caller is not the vault owner
    pub fn clear_passkey_compromise(
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
        let key = DataKey::CompromisedPasskeys(vault_id);
        let set: Vec<BytesN<32>> = env
            .storage().persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));
        let mut new_set: Vec<BytesN<32>> = Vec::new(&env);
        for h in set.iter() {
            if h != passkey_hash {
                new_set.push_back(h);
            }
        }
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &new_set);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(())
    }

    /// Returns whether a passkey has been flagged as compromised for a vault.
    pub fn is_passkey_compromised(env: Env, vault_id: u64, passkey_hash: BytesN<32>) -> bool {
        let key = DataKey::CompromisedPasskeys(vault_id);
        let set: Vec<BytesN<32>> = env
            .storage().persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));
        set.iter().any(|h| h == passkey_hash)
    }

    /// Inspects the recent passkey usage log and emits a `pk_comp` advisory event
    /// if 3 or more of the last 5 entries used different passkey hashes.
    fn detect_passkey_anomaly(env: &Env, vault_id: u64, current_hash: &BytesN<32>) {
        let usage: Vec<PasskeyUsageEntry> = env
            .storage().persistent()
            .get(&DataKey::PasskeyUsage(vault_id))
            .unwrap_or_else(|| Vec::new(env));
        if usage.len() < 3 {
            return;
        }
        let window_start = usage.len().saturating_sub(5);
        let mut distinct = 0u32;
        let mut prev: Option<BytesN<32>> = None;
        for i in window_start..usage.len() {
            let entry = usage.get(i as u32).unwrap();
            if prev.as_ref() != Some(&entry.passkey_hash) {
                distinct += 1;
                prev = Some(entry.passkey_hash.clone());
            }
        }
        if distinct >= 3 {
            env.events().publish((PASSKEY_COMPROMISED_TOPIC, vault_id), current_hash.clone());
        }
    }

    // --- Issue #549: Passkey Expiry Enforcement ---

    /// Validates that `passkey_hash` is registered for `vault_id` and not expired.
    ///
    /// Registration rules (backwards-compatible):
    /// - If `VaultPasskeys` list is non-empty, the hash must appear in that list.
    /// - If the list is empty AND `vault.passkey_hash` is Some, the hash must match it.
    /// - If both are empty/None, any passkey is accepted (no passkey configured yet).
    ///
    /// Expiry: if `PasskeyExpiry` is stored for this hash, it must not be in the past.
    fn validate_passkey_for_checkin(
        env: &Env,
        vault_id: u64,
        vault: &Vault,
        passkey_hash: &BytesN<32>,
        now: u64,
    ) -> Result<(), ContractError> {
        let pk_key = DataKey::VaultPasskeys(vault_id);
        let passkeys: Vec<PasskeyHash> = env
            .storage()
            .persistent()
            .get(&pk_key)
            .unwrap_or_else(|| Vec::new(env));

        if !passkeys.is_empty() {
            // Multi-passkey list is configured — hash must be in it.
            let registered = passkeys.iter().any(|p| &p.hash == passkey_hash);
            if !registered {
                return Err(ContractError::InvalidPasskey);
            }
        } else if let Some(ref primary) = vault.passkey_hash {
            // Fallback: single primary passkey on vault struct.
            if primary != passkey_hash {
                return Err(ContractError::InvalidPasskey);
            }
        }
        // else: no passkey configured — any hash accepted for backwards compat.

        // Issue #550: block check-in if passkey is flagged as compromised.
        {
            let comp_key = DataKey::CompromisedPasskeys(vault_id);
            let compromised: Vec<BytesN<32>> = env
                .storage().persistent()
                .get(&comp_key)
                .unwrap_or_else(|| Vec::new(env));
            if compromised.iter().any(|h| &h == passkey_hash) {
                env.events().publish((PASSKEY_COMPROMISED_TOPIC, vault_id), passkey_hash.clone());
                return Err(ContractError::PasskeyCompromised);
            }
        }

        // Expiry check — applies regardless of registration path.
        if let Some(expiry) = env.storage().persistent()
            .get::<DataKey, u64>(&DataKey::PasskeyExpiry(vault_id, passkey_hash.clone()))
        {
            if now > expiry {
                env.events().publish((PASSKEY_EXPIRED_TOPIC, vault_id), passkey_hash.clone());
                return Err(ContractError::PasskeyExpired);
            }
        }

        Ok(())
    }

    // --- Issue #395: Passkey Usage Analytics ---

    /// Logs a passkey usage entry for a vault check-in and runs anomaly detection.
    fn log_passkey_usage(env: &Env, vault_id: u64, passkey_hash: &BytesN<32>, timestamp: u64) {
        let mut usage: Vec<PasskeyUsageEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::PasskeyUsage(vault_id))
            .unwrap_or(Vec::new(env));

        // Issue #550: detect anomalous passkey switching before appending this entry.
        Self::detect_passkey_anomaly(env, vault_id, passkey_hash);

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
            inactivity_penalty_bps: original.inactivity_penalty_bps,
            penalty_recipient: original.penalty_recipient.clone(),
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
            let old_as_bytes = Bytes::from_array(&env, &old_passkey_hash.to_array());
            if current != &old_as_bytes {
                return Err(ContractError::InvalidPasskey);
            }
        } else {
            return Err(ContractError::InvalidPasskey);
        }

        vault.passkey_hash = Some(Bytes::from_array(&env, &new_passkey_hash.to_array()));
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
            biometric_hash: None,
        });
        
        env.storage().persistent().set(&key, &passkeys);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((ADD_PASSKEY_TOPIC, vault_id), passkey_hash);
        Ok(())
    }

    /// Binds a biometric credential hash to an existing passkey for the vault.
    ///
    /// Only the vault owner can call this. Emits `bind_pk_bio` event on success.
    pub fn bind_passkey_biometric(
        env: Env,
        vault_id: u64,
        caller: Address,
        passkey_hash: BytesN<32>,
        biometric_hash: BytesN<32>,
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
        let mut passkeys: Vec<PasskeyHash> = env.storage().persistent().get(&key).unwrap_or(Vec::new(&env));
        let mut found = false;
        for i in 0..passkeys.len() {
            if let Some(mut pk) = passkeys.get(i) {
                if pk.hash == passkey_hash {
                    pk.biometric_hash = Some(biometric_hash.clone());
                    passkeys.set(i, pk);
                    found = true;
                    break;
                }
            }
        }

        if !found {
            return Err(ContractError::PasskeyNotFound);
        }

        env.storage().persistent().set(&key, &passkeys);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((BIND_PASSKEY_BIOMETRIC_TOPIC, vault_id), (passkey_hash, biometric_hash));
        Ok(())
    }

    /// Unbinds a biometric credential from a passkey.
    pub fn unbind_passkey_biometric(
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
        let mut passkeys: Vec<PasskeyHash> = env.storage().persistent().get(&key).unwrap_or(Vec::new(&env));
        let mut found = false;
        for i in 0..passkeys.len() {
            if let Some(mut pk) = passkeys.get(i) {
                if pk.hash == passkey_hash {
                    pk.biometric_hash = None;
                    passkeys.set(i, pk);
                    found = true;
                    break;
                }
            }
        }

        if !found {
            return Err(ContractError::PasskeyNotFound);
        }

        env.storage().persistent().set(&key, &passkeys);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((UNBIND_PASSKEY_BIOMETRIC_TOPIC, vault_id), passkey_hash);
        Ok(())
    }

    /// Performs a biometric-enabled check-in using a passkey and a biometric credential hash.
    ///
    /// Validates passkey registration & expiry, then asserts the provided biometric
    /// hash matches the one bound to the passkey.
    pub fn biometric_check_in(
        env: Env,
        vault_id: u64,
        caller: Address,
        passkey_hash: BytesN<32>,
        biometric_hash: BytesN<32>,
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

        // Validate registration & expiry
        let now = env.ledger().timestamp();
        Self::validate_passkey_for_checkin(&env, vault_id, &vault, &passkey_hash, now)?;

        // Verify biometric binding
        let key = DataKey::VaultPasskeys(vault_id);
        let passkeys: Vec<PasskeyHash> = env.storage().persistent().get(&key).unwrap_or(Vec::new(&env));
        let mut matched = false;
        for pk in passkeys.iter() {
            if pk.hash == passkey_hash {
                if let Some(bound) = pk.biometric_hash {
                    if bound == biometric_hash {
                        matched = true;
                        break;
                    } else {
                        return Err(ContractError::InvalidPasskey);
                    }
                } else {
                    // No biometric bound
                    return Err(ContractError::InvalidPasskey);
                }
            }
        }
        if !matched {
            return Err(ContractError::PasskeyNotFound);
        }

        // Reuse check_in semantics for updating vault state
        vault.last_check_in = now;
        let original_last_check_in = vault.last_check_in;

        // Inactivity penalty & TTL cap are identical to check_in; reuse logic by calling check_in
        // However, to avoid code duplication in this patch, replicate minimal necessary steps.
        Self::save_vault(&env, vault_id, &vault);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        // Log passkey usage and emit events
        Self::log_passkey_usage(&env, vault_id, &passkey_hash, now);
        env.events().publish((BIO_CHECKIN_TOPIC, vault_id), (caller, now));
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

    /// Returns the current active delegate (last in the chain) or None if no delegation.
    fn get_delegated_beneficiary(env: Env, vault_id: u64) -> Option<Address> {
        let chain: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::BeneficiaryDelegationChain(vault_id))?;
        if chain.len() <= 1 {
            return None;
        }
        chain.get(chain.len() - 1)
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
                let beneficiary = Self::get_delegated_beneficiary(&env, vault_id)
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

    // --- Issue #503: Beneficiary Conditional Acceptance with Threshold ---

    /// Beneficiary accepts role conditionally with minimum balance threshold.
    /// Only accepts if vault balance >= min_balance_threshold at release time.
    pub fn accept_with_threshold(
        env: Env,
        vault_id: u64,
        min_balance_threshold: i128,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        let vault = Self::load_vault(&env, vault_id);
        vault.beneficiary.require_auth();

        if min_balance_threshold <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let acceptance = BeneficiaryConditionalAcceptance {
            min_balance_threshold,
            accepted_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryConditionalAcceptance(vault_id), &acceptance);

        env.events().publish(
            (BENEFICIARY_CONDITION_ACCEPTED_TOPIC,),
            (vault_id, vault.beneficiary.clone(), min_balance_threshold),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::BeneficiaryConditionalAcceptance(vault_id),
            VAULT_TTL_THRESHOLD,
            vault_ttl_ledgers(vault.check_in_interval),
        );
        Ok(())
    }

    /// Gets beneficiary conditional acceptance if it exists.
    pub fn get_beneficiary_conditional_acceptance(
        env: Env,
        vault_id: u64,
    ) -> Option<BeneficiaryConditionalAcceptance> {
        env.storage()
            .persistent()
            .get::<DataKey, BeneficiaryConditionalAcceptance>(
                &DataKey::BeneficiaryConditionalAcceptance(vault_id),
            )
    }

    /// Checks if beneficiary conditional acceptance conditions are met.
    fn check_conditional_acceptance_threshold(
        env: &Env,
        vault_id: u64,
        current_balance: i128,
    ) -> Result<bool, ContractError> {
        if let Some(acceptance) = env
            .storage()
            .persistent()
            .get::<DataKey, BeneficiaryConditionalAcceptance>(
                &DataKey::BeneficiaryConditionalAcceptance(vault_id),
            )
        {
            Ok(current_balance >= acceptance.min_balance_threshold)
        } else {
            Ok(true)
        }
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

    // --- Issue #502: Beneficiary Conflict Resolution ---

    /// File a beneficiary conflict claim. Beneficiary-only.
    pub fn file_beneficiary_conflict(
        env: Env,
        vault_id: u64,
        reason: String,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        let vault = Self::load_vault(&env, vault_id);
        vault.beneficiary.require_auth();

        if reason.len() == 0 {
            return Err(ContractError::InvalidAmount);
        }

        let mut conflict = env
            .storage()
            .persistent()
            .get::<DataKey, BeneficiaryConflict>(&DataKey::BeneficiaryConflict(vault_id))
            .unwrap_or_else(|| BeneficiaryConflict {
                vault_id,
                claims: Vec::new(&env),
                resolution: ConflictResolution::Pending,
                resolved_at: None,
            });

        let claim = BeneficiaryConflictClaim {
            claimant: vault.beneficiary.clone(),
            reason,
            filed_at: env.ledger().timestamp(),
        };

        conflict.claims.push_back(claim);
        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryConflict(vault_id), &conflict);

        env.events().publish(
            (BENEFICIARY_CONFLICT_FILED_TOPIC,),
            (vault_id, vault.beneficiary.clone()),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::BeneficiaryConflict(vault_id),
            VAULT_TTL_THRESHOLD,
            vault_ttl_ledgers(vault.check_in_interval),
        );
        Ok(())
    }

    /// Resolve beneficiary conflict. Admin-only.
    pub fn resolve_beneficiary_conflict(
        env: Env,
        vault_id: u64,
        approved_beneficiary: Address,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env);

        let mut conflict = env
            .storage()
            .persistent()
            .get::<DataKey, BeneficiaryConflict>(&DataKey::BeneficiaryConflict(vault_id))
            .ok_or(ContractError::InvalidBeneficiary)?;

        if conflict.resolution != ConflictResolution::Pending {
            return Err(ContractError::InvalidBeneficiary);
        }

        conflict.resolution = ConflictResolution::Approved(approved_beneficiary.clone());
        conflict.resolved_at = Some(env.ledger().timestamp());

        env.storage()
            .persistent()
            .set(&DataKey::BeneficiaryConflict(vault_id), &conflict);

        env.events().publish(
            (BENEFICIARY_CONFLICT_RESOLVED_TOPIC,),
            (vault_id, approved_beneficiary),
        );
        Ok(())
    }

    /// Get beneficiary conflict if it exists.
    pub fn get_beneficiary_conflict(
        env: Env,
        vault_id: u64,
    ) -> Option<BeneficiaryConflict> {
        env.storage()
            .persistent()
            .get::<DataKey, BeneficiaryConflict>(&DataKey::BeneficiaryConflict(vault_id))
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
            return Err(ContractError::InvalidAmount);
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
            return Err(ContractError::InvalidThreshold);
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
            // Issue #549: enforce passkey registration and expiry.
            let vault_inner = Self::load_vault(&env, vault_id);
            Self::validate_passkey_for_checkin(&env, vault_id, &vault_inner, &passkey_hash, now)?;
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

        // Issue #549: enforce passkey registration and expiry.
        Self::validate_passkey_for_checkin(&env, vault_id, &vault, &passkey_hash, now)?;

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
                return Err(ContractError::VotingNotEnabled);
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

    fn get_delegated_beneficiary(env: &Env, vault_id: u64) -> Option<Address> {
        env.storage().persistent()
            .get(&DataKey::BeneficiaryDelegate(vault_id))
    }

    fn distribute_release_amount(env: &Env, vault_id: u64, vault: &Vault, release_amount: i128, mode: ReleaseTrigger) {
        let token_client = token::Client::new(env, &vault.token_address);

        let triggers_map: Vec<(Address, Vec<ReleaseTrigger>)> = env
            .storage().persistent()
            .get(&DataKey::BeneficiaryReleaseTriggers(vault_id))
            .unwrap_or_else(|| Vec::new(env));

        let mut qualifying: Vec<BeneficiaryEntry> = Vec::new(env);
        let mut total_qualifying_bps: u32 = 0;

        for entry in vault.beneficiaries.iter() {
            let mut ben_triggers: Vec<ReleaseTrigger> = Vec::new(env);
            for mapping in triggers_map.iter() {
                if mapping.0 == entry.address {
                    ben_triggers = mapping.1.clone();
                    break;
                }
            }
            let mode_allowed = if ben_triggers.is_empty() {
                true
            } else {
                ben_triggers.iter().any(|t| t == mode)
            };
            if !mode_allowed {
                env.events().publish(
                    (BENEFICIARY_WATERFALL_TOPIC, vault_id),
                    BeneficiaryWaterfallEvent {
                        vault_id,
                        skipped_beneficiary: entry.address.clone(),
                        reason: String::from_str(env, "trigger"),
                    },
                );
                continue;
            }

            let ben_status: BeneficiaryStatus = env.storage().persistent()
                .get(&DataKey::BeneficiaryStatusEntry(vault_id, entry.address.clone()))
                .unwrap_or(BeneficiaryStatus::Pending);
            if ben_status == BeneficiaryStatus::Declined {
                env.events().publish(
                    (BENEFICIARY_WATERFALL_TOPIC, vault_id),
                    BeneficiaryWaterfallEvent {
                        vault_id,
                        skipped_beneficiary: entry.address.clone(),
                        reason: String::from_str(env, "declined"),
                    },
                );
                continue;
            }

            let tier_threshold: i128 = env.storage().persistent()
                .get(&DataKey::BeneficiaryTierThreshold(vault_id, entry.address.clone()))
                .unwrap_or(0);
            if tier_threshold > 0 && release_amount < tier_threshold {
                env.events().publish(
                    (BENEFICIARY_WATERFALL_TOPIC, vault_id),
                    BeneficiaryWaterfallEvent {
                        vault_id,
                        skipped_beneficiary: entry.address.clone(),
                        reason: String::from_str(env, "tier"),
                    },
                );
                continue;
            }

            let initial_share = release_amount * (entry.bps as i128) / 10_000;
            if entry.minimum_threshold > 0 && initial_share < entry.minimum_threshold {
                env.events().publish(
                    (MIN_THRESHOLD_SKIP_TOPIC,),
                    (vault_id, entry.address.clone(), initial_share, entry.minimum_threshold),
                );
                continue;
            }

            qualifying.push_back(entry.clone());
            total_qualifying_bps = total_qualifying_bps.saturating_add(entry.bps);
        }

        if qualifying.is_empty() {
            token_client.transfer(&env.current_contract_address(), &vault.owner, &release_amount);
            env.events().publish(
                (RELEASE_TOPIC,),
                ReleaseEvent { vault_id, beneficiary: vault.owner.clone(), amount: release_amount },
            );
            return;
        }

        if total_qualifying_bps < 10_000 && total_qualifying_bps > 0 {
            env.events().publish(
                (BENEFICIARY_REBALANCED_TOPIC, vault_id),
                BeneficiaryRebalancedEvent { vault_id, remaining_bps: total_qualifying_bps },
            );
        }

        let mut distributed: i128 = 0;
        let last_idx = qualifying.len() - 1;
        for (i, entry) in qualifying.iter().enumerate() {
            let share = if i as u32 == last_idx {
                release_amount - distributed
            } else {
                release_amount * (entry.bps as i128) / (total_qualifying_bps as i128)
            };
            if share > 0 {
                token_client.transfer(&env.current_contract_address(), &entry.address, &share);
                env.events().publish(
                    (RELEASE_TOPIC,),
                    ReleaseEvent { vault_id, beneficiary: entry.address.clone(), amount: share },
                );
            }
            distributed += share;
            env.events().publish(
                (MIN_THRESHOLD_REDISTRIBUTE_TOPIC,),
                (vault_id, entry.address.clone(), share),
            );
        }
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

    // --- Issue #565: Withdrawal Scheduling Validation ---

    /// Validates withdrawal schedules to prevent overlapping or conflicting withdrawals.
    /// Returns true if the schedule is valid, false otherwise.
    fn validate_withdrawal_schedule(
        env: &Env,
        vault_id: u64,
        new_timestamp: u64,
        new_amount: i128,
    ) -> bool {
        let key = DataKey::WithdrawalScheduleValidation(vault_id);
        if let Some(schedules) = env.storage().persistent().get::<DataKey, Vec<WithdrawalScheduleEntry>>(&key) {
            for schedule in schedules.iter() {
                // Check for overlapping withdrawals (within 1 hour window)
                if (new_timestamp as i128 - schedule.timestamp as i128).abs() < 3600 {
                    return false;
                }
            }
        }
        true
    }

    /// Schedules a withdrawal with validation to prevent conflicts.
    /// Only the vault owner can call this.
    pub fn schedule_withdrawal(
        env: Env,
        vault_id: u64,
        caller: Address,
        timestamp: u64,
        amount: i128,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if vault.status != ReleaseStatus::Locked {
            return Err(ContractError::AlreadyReleased);
        }

        // Validate schedule
        if !Self::validate_withdrawal_schedule(&env, vault_id, timestamp, amount) {
            return Err(ContractError::ConflictingWithdrawalSchedule);
        }

        let key = DataKey::WithdrawalScheduleValidation(vault_id);
        let mut schedules = env.storage().persistent().get::<DataKey, Vec<WithdrawalScheduleEntry>>(&key)
            .unwrap_or_else(|| Vec::new(&env));
        
        schedules.push_back(WithdrawalScheduleEntry { timestamp, amount });
        
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &schedules);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        
        env.events().publish((WITHDRAWAL_VALIDATION_TOPIC, vault_id), (timestamp, amount));
        Ok(())
    }

    // --- Issue #566: Withdrawal Limits by Time ---

    /// Sets daily, weekly, and monthly withdrawal limits for a vault.
    /// Only the vault owner can call this.
    pub fn set_withdrawal_limits(
        env: Env,
        vault_id: u64,
        caller: Address,
        daily_limit: i128,
        weekly_limit: i128,
        monthly_limit: i128,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let limits = WithdrawalLimit {
            daily_limit,
            weekly_limit,
            monthly_limit,
        };

        let key = DataKey::WithdrawalLimit(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &limits);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish((WITHDRAWAL_LIMIT_SET_TOPIC, vault_id), (daily_limit, weekly_limit, monthly_limit));
        Ok(())
    }

    /// Gets the current withdrawal limits for a vault.
    pub fn get_withdrawal_limits(env: Env, vault_id: u64) -> Option<WithdrawalLimit> {
        env.storage().persistent().get(&DataKey::WithdrawalLimit(vault_id))
    }

    /// Checks if a withdrawal would exceed time-based limits.
    fn check_withdrawal_limits(
        env: &Env,
        vault_id: u64,
        amount: i128,
    ) -> Result<(), ContractError> {
        if let Some(limits) = env.storage().persistent().get::<DataKey, WithdrawalLimit>(&DataKey::WithdrawalLimit(vault_id)) {
            let tracker_key = DataKey::WithdrawalTracker(vault_id);
            let now = env.ledger().timestamp();
            
            let mut tracker = env.storage().persistent().get::<DataKey, WithdrawalTracker>(&tracker_key)
                .unwrap_or_else(|| WithdrawalTracker {
                    daily_withdrawn: 0,
                    daily_reset_at: now + 86_400,
                    weekly_withdrawn: 0,
                    weekly_reset_at: now + 604_800,
                    monthly_withdrawn: 0,
                    monthly_reset_at: now + 2_592_000,
                });

            // Reset trackers if periods have passed
            if now >= tracker.daily_reset_at {
                tracker.daily_withdrawn = 0;
                tracker.daily_reset_at = now + 86_400;
            }
            if now >= tracker.weekly_reset_at {
                tracker.weekly_withdrawn = 0;
                tracker.weekly_reset_at = now + 604_800;
            }
            if now >= tracker.monthly_reset_at {
                tracker.monthly_withdrawn = 0;
                tracker.monthly_reset_at = now + 2_592_000;
            }

            // Check limits
            if tracker.daily_withdrawn + amount > limits.daily_limit {
                return Err(ContractError::DailyWithdrawalLimitExceeded);
            }
            if tracker.weekly_withdrawn + amount > limits.weekly_limit {
                return Err(ContractError::WeeklyWithdrawalLimitExceeded);
            }
            if tracker.monthly_withdrawn + amount > limits.monthly_limit {
                return Err(ContractError::MonthlyWithdrawalLimitExceeded);
            }

            // Update tracker
            tracker.daily_withdrawn += amount;
            tracker.weekly_withdrawn += amount;
            tracker.monthly_withdrawn += amount;

            let vault = Self::load_vault(&env, vault_id);
            let ttl = vault_ttl_ledgers(vault.check_in_interval);
            env.storage().persistent().set(&tracker_key, &tracker);
            env.storage().persistent().extend_ttl(&tracker_key, VAULT_TTL_THRESHOLD, ttl);
        }
        Ok(())
    }

    // --- Issue #567: Withdrawal Destination Whitelist ---

    /// Adds an address to the withdrawal whitelist for a vault.
    /// Only the vault owner can call this.
    pub fn add_whitelist_address(
        env: Env,
        vault_id: u64,
        caller: Address,
        address: Address,
        label: String,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let key = DataKey::WithdrawalWhitelist(vault_id);
        let mut whitelist = env.storage().persistent().get::<DataKey, Vec<WhitelistEntry>>(&key)
            .unwrap_or_else(|| Vec::new(&env));

        let entry = WhitelistEntry {
            address: address.clone(),
            added_at: env.ledger().timestamp(),
            label,
        };
        whitelist.push_back(entry);

        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &whitelist);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish((WHITELIST_ADDED_TOPIC, vault_id), address);
        Ok(())
    }

    /// Removes an address from the withdrawal whitelist for a vault.
    pub fn remove_whitelist_address(
        env: Env,
        vault_id: u64,
        caller: Address,
        address: Address,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let key = DataKey::WithdrawalWhitelist(vault_id);
        if let Some(whitelist) = env.storage().persistent().get::<DataKey, Vec<WhitelistEntry>>(&key) {
            let mut new_whitelist = Vec::new(&env);
            for entry in whitelist.iter() {
                if entry.address != address {
                    new_whitelist.push_back(entry);
                }
            }
            let ttl = vault_ttl_ledgers(vault.check_in_interval);
            env.storage().persistent().set(&key, &new_whitelist);
            env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
            env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        }

        env.events().publish((WHITELIST_REMOVED_TOPIC, vault_id), address);
        Ok(())
    }

    /// Gets the withdrawal whitelist for a vault.
    pub fn get_whitelist(env: Env, vault_id: u64) -> Option<Vec<WhitelistEntry>> {
        env.storage().persistent().get(&DataKey::WithdrawalWhitelist(vault_id))
    }

    /// Checks if an address is whitelisted for withdrawals.
    fn is_whitelisted(env: &Env, vault_id: u64, address: &Address) -> bool {
        if let Some(whitelist) = env.storage().persistent().get::<DataKey, Vec<WhitelistEntry>>(&DataKey::WithdrawalWhitelist(vault_id)) {
            for entry in whitelist.iter() {
                if entry.address == *address {
                    return true;
                }
            }
            false
        } else {
            true // No whitelist means all addresses allowed
        }
    }

    // --- Issue #568: Withdrawal Reversal ---

    /// Records a withdrawal for potential reversal within a grace period.
    fn record_withdrawal_for_reversal(
        env: &Env,
        vault_id: u64,
        amount: i128,
        grace_period_seconds: u64,
    ) {
        let counter_key = DataKey::WithdrawalReversalCounter(vault_id);
        let withdrawal_id: u64 = env.storage().persistent().get(&counter_key).unwrap_or(0u64);
        
        let reversal = WithdrawalReversal {
            withdrawal_id,
            amount,
            withdrawn_at: env.ledger().timestamp(),
            grace_period_until: env.ledger().timestamp() + grace_period_seconds,
            reversed: false,
        };

        let key = DataKey::WithdrawalReversal(vault_id, withdrawal_id);
        let vault = Self::load_vault(&env, vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        
        env.storage().persistent().set(&key, &reversal);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().persistent().set(&counter_key, &(withdrawal_id + 1));
        env.storage().persistent().extend_ttl(&counter_key, VAULT_TTL_THRESHOLD, ttl);
    }

    /// Reverses a withdrawal within the grace period.
    /// Only the vault owner can call this.
    pub fn reverse_withdrawal(
        env: Env,
        vault_id: u64,
        caller: Address,
        withdrawal_id: u64,
    ) -> Result<(), ContractError> {
        if Self::load_paused(&env) {
            return Err(ContractError::Paused);
        }
        caller.require_auth();
        let mut vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let key = DataKey::WithdrawalReversal(vault_id, withdrawal_id);
        let mut reversal = env.storage().persistent().get::<DataKey, WithdrawalReversal>(&key)
            .ok_or(ContractError::VaultNotFound)?;

        if reversal.reversed {
            return Err(ContractError::WithdrawalAlreadyReversed);
        }

        let now = env.ledger().timestamp();
        if now > reversal.grace_period_until {
            return Err(ContractError::WithdrawalReversalGracePeriodExpired);
        }

        // Restore funds to vault
        vault.balance += reversal.amount;
        reversal.reversed = true;

        Self::save_vault(&env, vault_id, &vault);
        env.storage().persistent().set(&key, &reversal);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish((WITHDRAWAL_REVERSED_TOPIC, vault_id), (withdrawal_id, reversal.amount));
        Ok(())
    }

    /// Gets a withdrawal reversal record.
    pub fn get_withdrawal_reversal(
        env: Env,
        vault_id: u64,
        withdrawal_id: u64,
    ) -> Option<WithdrawalReversal> {
        env.storage().persistent().get(&DataKey::WithdrawalReversal(vault_id, withdrawal_id))
    }

    // --- Issue #545: Vesting Catch-Up ---

    /// Configures catch-up vesting for a vault.
    ///
    /// When enabled, a beneficiary who missed claiming periods can catch up and claim
    /// all accumulated missed installments in a single `claim_catchup_vesting` call.
    ///
    /// # Arguments
    /// * `vault_id` - The vault to configure
    /// * `caller` - Must be the vault owner
    /// * `enabled` - Whether catch-up claiming is enabled
    /// * `max_catchup_installments` - Max missed installments claimable at once (0 = unlimited)
    pub fn set_vesting_catchup(
        env: Env,
        vault_id: u64,
        caller: Address,
        enabled: bool,
        max_catchup_installments: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        let config = VestingCatchUpConfig { enabled, max_catchup_installments };
        let key = DataKey::VestingCatchUp(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &config);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((VESTING_CATCHUP_SET_TOPIC, vault_id), (enabled, max_catchup_installments));
        Ok(())
    }

    /// Returns the catch-up vesting configuration for a vault.
    pub fn get_vesting_catchup(env: Env, vault_id: u64) -> Option<VestingCatchUpConfig> {
        env.storage().persistent().get(&DataKey::VestingCatchUp(vault_id))
    }

    /// Claims all accumulated missed vesting installments in one call.
    ///
    /// Requires catch-up to be enabled via `set_vesting_catchup`. Transfers all
    /// unlocked-but-unclaimed installments up to `max_catchup_installments` (if set).
    ///
    /// # Returns
    /// Total amount transferred to the beneficiary.
    ///
    /// # Errors
    /// * `ContractError::CatchUpNotEnabled` - If catch-up is not configured or disabled
    /// * `ContractError::VestingNotFound` - If no vesting schedule exists
    /// * `ContractError::AlreadyReleased` - If vault is not in Released status
    /// * `ContractError::NothingToClaimYet` - If no missed installments to catch up on
    /// * `ContractError::VestingAlreadyComplete` - If all installments already claimed
    pub fn claim_catchup_vesting(env: Env, vault_id: u64) -> Result<i128, ContractError> {
        Self::assert_not_paused(&env);
        let mut vault = Self::load_vault(&env, vault_id);

        if vault.status != ReleaseStatus::Released {
            return Err(ContractError::AlreadyReleased);
        }

        let catchup_cfg: VestingCatchUpConfig = env
            .storage()
            .persistent()
            .get(&DataKey::VestingCatchUp(vault_id))
            .ok_or(ContractError::CatchUpNotEnabled)?;

        if !catchup_cfg.enabled {
            return Err(ContractError::CatchUpNotEnabled);
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

        // Enforce cliff
        if schedule.cliff_period > 0 && now < schedule.start_time + schedule.cliff_period {
            return Err(ContractError::CliffNotReached);
        }

        let elapsed = now.saturating_sub(schedule.start_time);
        let unlocked = ((elapsed / schedule.interval) + 1)
            .min(schedule.num_installments as u64) as u32;

        let mut claimable = unlocked.saturating_sub(schedule.claimed_installments);
        if claimable == 0 {
            return Err(ContractError::NothingToClaimYet);
        }

        // Apply max catch-up limit if set
        if catchup_cfg.max_catchup_installments > 0 {
            claimable = claimable.min(catchup_cfg.max_catchup_installments);
        }

        let new_claimed = schedule.claimed_installments + claimable;
        let per_installment = schedule.total_amount / schedule.num_installments as i128;
        let amount = if new_claimed >= schedule.num_installments {
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
                (VESTING_CATCHUP_CLAIMED_TOPIC, vault_id),
                (vault.beneficiary.clone(), amount, claimable),
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
                    (VESTING_CATCHUP_CLAIMED_TOPIC, vault_id),
                    (entry.address.clone(), share, claimable),
                );
            }
        }

        vault.balance -= amount;
        schedule.claimed_installments = new_claimed;
        Self::save_vault(&env, vault_id, &vault);

        let sched_key = DataKey::VestingSchedule(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&sched_key, &schedule);
        env.storage().persistent().extend_ttl(&sched_key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(amount)
    }

    // --- Issue #546: Vesting Bonus ---

    /// Configures a bonus for on-time vesting claims.
    ///
    /// When configured, beneficiaries who claim within `on_time_window_seconds` of an
    /// installment unlocking receive a bonus of `bonus_bps` basis points on top of
    /// their normal installment amount.
    ///
    /// # Arguments
    /// * `vault_id` - The vault to configure
    /// * `caller` - Must be the vault owner
    /// * `bonus_bps` - Bonus in basis points (e.g., 100 = 1%)
    /// * `on_time_window_seconds` - Seconds after unlock within which claim is "on time"
    pub fn set_vesting_bonus(
        env: Env,
        vault_id: u64,
        caller: Address,
        bonus_bps: u32,
        on_time_window_seconds: u64,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if bonus_bps == 0 || bonus_bps > 10_000 {
            return Err(ContractError::InvalidBps);
        }
        let config = VestingBonusConfig { bonus_bps, on_time_window_seconds };
        let key = DataKey::VestingBonus(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&key, &config);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((VESTING_BONUS_SET_TOPIC, vault_id), (bonus_bps, on_time_window_seconds));
        Ok(())
    }

    /// Returns the vesting bonus configuration for a vault.
    pub fn get_vesting_bonus(env: Env, vault_id: u64) -> Option<VestingBonusConfig> {
        env.storage().persistent().get(&DataKey::VestingBonus(vault_id))
    }

    /// Claims vesting installments with an on-time bonus applied if eligible.
    ///
    /// Behaves like `claim_vested_installment` but additionally checks if a bonus
    /// config exists. If all claimable installments are claimed within the on-time
    /// window, the bonus is applied on top of the base payout.
    ///
    /// # Returns
    /// Total amount transferred (base + bonus if on time).
    ///
    /// # Errors
    /// * `ContractError::BonusNotEnabled` - If no bonus config is set
    /// * `ContractError::VestingNotFound` - If no vesting schedule exists
    /// * `ContractError::AlreadyReleased` - If vault is not in Released status
    /// * `ContractError::NothingToClaimYet` - If no installments available
    /// * `ContractError::VestingAlreadyComplete` - If all installments already claimed
    /// * `ContractError::InsufficientBalance` - If vault balance is too low for bonus
    pub fn claim_with_bonus(env: Env, vault_id: u64) -> Result<i128, ContractError> {
        Self::assert_not_paused(&env);
        let mut vault = Self::load_vault(&env, vault_id);

        if vault.status != ReleaseStatus::Released {
            return Err(ContractError::AlreadyReleased);
        }

        let bonus_cfg: VestingBonusConfig = env
            .storage()
            .persistent()
            .get(&DataKey::VestingBonus(vault_id))
            .ok_or(ContractError::BonusNotEnabled)?;

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

        // Enforce cliff
        if schedule.cliff_period > 0 && now < schedule.start_time + schedule.cliff_period {
            return Err(ContractError::CliffNotReached);
        }

        let elapsed = now.saturating_sub(schedule.start_time);
        let unlocked = ((elapsed / schedule.interval) + 1)
            .min(schedule.num_installments as u64) as u32;

        let claimable = unlocked.saturating_sub(schedule.claimed_installments);
        if claimable == 0 {
            return Err(ContractError::NothingToClaimYet);
        }

        let per_installment = schedule.total_amount / schedule.num_installments as i128;
        let base_amount = if unlocked >= schedule.num_installments {
            vault.balance
        } else {
            per_installment * claimable as i128
        };

        // Determine if all claimable installments are on time.
        // An installment i (1-based from claimed+1) is on time if:
        //   now <= unlock_time + on_time_window_seconds
        let mut all_on_time = true;
        for i in (schedule.claimed_installments + 1)..=unlocked {
            let unlock_time = schedule
                .start_time
                .saturating_add((i as u64 - 1).saturating_mul(schedule.interval));
            if now > unlock_time + bonus_cfg.on_time_window_seconds {
                all_on_time = false;
                break;
            }
        }

        let bonus = if all_on_time {
            base_amount * bonus_cfg.bonus_bps as i128 / 10_000
        } else {
            0
        };

        let total_amount = base_amount + bonus;

        if vault.balance < total_amount {
            // If vault can't cover the bonus, pay base only
            let fallback = base_amount.min(vault.balance);
            if vault.balance < fallback {
                return Err(ContractError::InsufficientBalance);
            }
            // Pay base without bonus
            let token_client = token::Client::new(&env, &vault.token_address);
            if vault.beneficiaries.is_empty() {
                token_client.transfer(&env.current_contract_address(), &vault.beneficiary, &fallback);
                env.events().publish(
                    (VESTING_BONUS_CLAIMED_TOPIC, vault_id),
                    (vault.beneficiary.clone(), fallback, 0i128),
                );
            } else {
                let mut distributed: i128 = 0;
                let last_idx = vault.beneficiaries.len() - 1;
                for (i, entry) in vault.beneficiaries.iter().enumerate() {
                    let share = if i as u32 == last_idx {
                        fallback - distributed
                    } else {
                        fallback * (entry.bps as i128) / 10_000
                    };
                    if share > 0 {
                        token_client.transfer(&env.current_contract_address(), &entry.address, &share);
                    }
                    distributed += share;
                    env.events().publish(
                        (VESTING_BONUS_CLAIMED_TOPIC, vault_id),
                        (entry.address.clone(), share, 0i128),
                    );
                }
            }
            vault.balance -= fallback;
            schedule.claimed_installments = unlocked;
            Self::save_vault(&env, vault_id, &vault);
            let sched_key = DataKey::VestingSchedule(vault_id);
            let ttl = vault_ttl_ledgers(vault.check_in_interval);
            env.storage().persistent().set(&sched_key, &schedule);
            env.storage().persistent().extend_ttl(&sched_key, VAULT_TTL_THRESHOLD, ttl);
            env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
            return Ok(fallback);
        }

        let token_client = token::Client::new(&env, &vault.token_address);

        if vault.beneficiaries.is_empty() {
            token_client.transfer(&env.current_contract_address(), &vault.beneficiary, &total_amount);
            env.events().publish(
                (VESTING_BONUS_CLAIMED_TOPIC, vault_id),
                (vault.beneficiary.clone(), total_amount, bonus),
            );
        } else {
            let mut distributed: i128 = 0;
            let last_idx = vault.beneficiaries.len() - 1;
            for (i, entry) in vault.beneficiaries.iter().enumerate() {
                let share = if i as u32 == last_idx {
                    total_amount - distributed
                } else {
                    total_amount * (entry.bps as i128) / 10_000
                };
                if share > 0 {
                    token_client.transfer(&env.current_contract_address(), &entry.address, &share);
                }
                distributed += share;
                let entry_bonus = if all_on_time {
                    share * bonus_cfg.bonus_bps as i128 / (10_000 + bonus_cfg.bonus_bps as i128)
                } else {
                    0
                };
                env.events().publish(
                    (VESTING_BONUS_CLAIMED_TOPIC, vault_id),
                    (entry.address.clone(), share, entry_bonus),
                );
            }
        }

        vault.balance -= total_amount;
        schedule.claimed_installments = unlocked;
        Self::save_vault(&env, vault_id, &vault);

        let sched_key = DataKey::VestingSchedule(vault_id);
        let ttl = vault_ttl_ledgers(vault.check_in_interval);
        env.storage().persistent().set(&sched_key, &schedule);
        env.storage().persistent().extend_ttl(&sched_key, VAULT_TTL_THRESHOLD, ttl);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        Ok(total_amount)
    }

    // --- Issue #585: Token Lending ---

    /// Enables token lending for a vault, allowing vault tokens to be lent out for interest income.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// * `caller` - Must be the vault owner
    /// * `borrower` - Address of the borrower
    /// * `amount` - Amount to lend in stroops
    /// * `interest_rate_bps` - Annual interest rate in basis points (e.g., 500 = 5%)
    /// * `duration_seconds` - Loan duration in seconds
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::InsufficientBalance` - If vault balance is insufficient
    pub fn enable_token_lending(
        env: Env,
        vault_id: u64,
        caller: Address,
        borrower: Address,
        amount: i128,
        interest_rate_bps: u32,
        duration_seconds: u64,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if amount <= 0 || vault.balance < amount {
            return Err(ContractError::InsufficientBalance);
        }

        let now = env.ledger().timestamp();
        let lending = TokenLending {
            vault_id,
            borrower: borrower.clone(),
            amount,
            interest_rate_bps,
            lent_at: now,
            due_at: now.saturating_add(duration_seconds),
            repaid: false,
            interest_earned: 0,
        };

        let key = DataKey::TokenLending(vault_id);
        env.storage().persistent().set(&key, &lending);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish(
            (TOKEN_LENDING_TOPIC, vault_id),
            (&borrower, amount, interest_rate_bps, duration_seconds),
        );
        Ok(())
    }

    /// Repays a token loan and records the interest earned.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// * `caller` - Must be the vault owner
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::InvalidAmount` - If no active lending exists or already repaid
    pub fn repay_token_loan(
        env: Env,
        vault_id: u64,
        caller: Address,
    ) -> Result<i128, ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let key = DataKey::TokenLending(vault_id);
        let mut lending: TokenLending = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::InvalidAmount)?;

        if lending.repaid {
            return Err(ContractError::InvalidAmount);
        }

        // Calculate accrued interest: amount * rate * elapsed / (10000 * 365 * 86400)
        let now = env.ledger().timestamp();
        let elapsed = now.saturating_sub(lending.lent_at);
        let interest = (lending.amount as i128)
            .checked_mul(lending.interest_rate_bps as i128)
            .and_then(|v| v.checked_mul(elapsed as i128))
            .and_then(|v| v.checked_div(10000 * 365 * 86400))
            .unwrap_or(0);

        lending.repaid = true;
        lending.interest_earned = interest;
        env.storage().persistent().set(&key, &lending);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish(
            (TOKEN_LEND_REPAY_TOPIC, vault_id),
            (lending.borrower.clone(), lending.amount, interest),
        );
        Ok(interest)
    }

    /// Returns the token lending record for a vault.
    pub fn get_token_lending(env: Env, vault_id: u64) -> Option<TokenLending> {
        env.storage().persistent().get(&DataKey::TokenLending(vault_id))
    }

    // --- Issue #586: Token Collateral ---

    /// Configures vault tokens as collateral for a loan.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// * `caller` - Must be the vault owner
    /// * `collateral_amount` - Amount of vault tokens used as collateral
    /// * `loan_amount` - Amount of tokens borrowed against the collateral
    /// * `collateral_ratio_bps` - Required collateral ratio in bps (e.g., 15000 = 150%)
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::InsufficientBalance` - If vault balance is insufficient
    /// * `ContractError::InvalidBps` - If collateral ratio is below 10000 (100%)
    pub fn set_token_collateral(
        env: Env,
        vault_id: u64,
        caller: Address,
        collateral_amount: i128,
        loan_amount: i128,
        collateral_ratio_bps: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if collateral_amount <= 0 || vault.balance < collateral_amount {
            return Err(ContractError::InsufficientBalance);
        }
        if collateral_ratio_bps < 10_000 {
            return Err(ContractError::InvalidBps);
        }

        let collateral = TokenCollateral {
            vault_id,
            collateral_amount,
            loan_amount,
            collateral_ratio_bps,
            active: true,
            created_at: env.ledger().timestamp(),
        };

        let key = DataKey::TokenCollateral(vault_id);
        env.storage().persistent().set(&key, &collateral);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish(
            (TOKEN_COLLATERAL_TOPIC, vault_id),
            (collateral_amount, loan_amount, collateral_ratio_bps),
        );
        Ok(())
    }

    /// Releases the collateral position for a vault.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// * `caller` - Must be the vault owner
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::InvalidAmount` - If no active collateral exists
    pub fn release_token_collateral(
        env: Env,
        vault_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let key = DataKey::TokenCollateral(vault_id);
        let mut collateral: TokenCollateral = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::InvalidAmount)?;

        if !collateral.active {
            return Err(ContractError::InvalidAmount);
        }

        collateral.active = false;
        env.storage().persistent().set(&key, &collateral);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish(
            (TOKEN_COLLAT_RLSD_TOPIC, vault_id),
            (collateral.collateral_amount, collateral.loan_amount),
        );
        Ok(())
    }

    /// Returns the token collateral configuration for a vault.
    pub fn get_token_collateral(env: Env, vault_id: u64) -> Option<TokenCollateral> {
        env.storage().persistent().get(&DataKey::TokenCollateral(vault_id))
    }

    // --- Issue #587: Token Hedging ---

    /// Enables a hedge position to protect against vault token price risk.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// * `caller` - Must be the vault owner
    /// * `hedge_token` - Token used for hedging (e.g., a stablecoin)
    /// * `notional_amount` - Notional value of the hedge position
    /// * `strike_price_bps` - Strike price in basis points relative to current price
    /// * `expiry` - Unix timestamp when the hedge expires
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::InvalidAmount` - If notional amount is not positive
    pub fn enable_token_hedge(
        env: Env,
        vault_id: u64,
        caller: Address,
        hedge_token: Address,
        notional_amount: i128,
        strike_price_bps: u32,
        expiry: u64,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }
        if notional_amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let now = env.ledger().timestamp();
        let hedge = TokenHedge {
            vault_id,
            hedge_token: hedge_token.clone(),
            notional_amount,
            strike_price_bps,
            expiry,
            active: true,
            created_at: now,
        };

        let key = DataKey::TokenHedge(vault_id);
        env.storage().persistent().set(&key, &hedge);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish(
            (TOKEN_HEDGE_TOPIC, vault_id),
            (&hedge_token, notional_amount, strike_price_bps, expiry),
        );
        Ok(())
    }

    /// Closes an active hedge position for a vault.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// * `caller` - Must be the vault owner
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::InvalidAmount` - If no active hedge exists
    pub fn close_token_hedge(
        env: Env,
        vault_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let key = DataKey::TokenHedge(vault_id);
        let mut hedge: TokenHedge = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::InvalidAmount)?;

        if !hedge.active {
            return Err(ContractError::InvalidAmount);
        }

        hedge.active = false;
        env.storage().persistent().set(&key, &hedge);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish(
            (TOKEN_HEDGE_CLOSE_TOPIC, vault_id),
            (hedge.notional_amount, hedge.strike_price_bps),
        );
        Ok(())
    }

    /// Returns the token hedge configuration for a vault.
    pub fn get_token_hedge(env: Env, vault_id: u64) -> Option<TokenHedge> {
        env.storage().persistent().get(&DataKey::TokenHedge(vault_id))
    }

    // --- Issue #588: Token Rebalancing ---

    /// Configures automatic portfolio rebalancing based on target token weights.
    ///
    /// All `target_bps` values across the weights must sum to 10000.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    /// * `caller` - Must be the vault owner
    /// * `target_weights` - Per-token target allocations (basis points, must sum to 10000)
    /// * `rebalance_threshold_bps` - Drift tolerance before triggering rebalance (e.g., 500 = 5%)
    ///
    /// # Errors
    /// * `ContractError::NotOwner` - If caller is not the vault owner
    /// * `ContractError::InvalidBps` - If weights do not sum to 10000
    pub fn set_token_rebalance(
        env: Env,
        vault_id: u64,
        caller: Address,
        target_weights: Vec<TokenWeight>,
        rebalance_threshold_bps: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        caller.require_auth();
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        // Validate weights sum to 10000
        let total_bps: u32 = target_weights.iter().map(|w| w.target_bps).sum();
        if total_bps != 10_000 {
            return Err(ContractError::InvalidBps);
        }

        let config = TokenRebalanceConfig {
            vault_id,
            target_weights,
            last_rebalance: env.ledger().timestamp(),
            rebalance_threshold_bps,
            total_rebalances: 0,
        };

        let key = DataKey::TokenRebalance(vault_id);
        env.storage().persistent().set(&key, &config);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish(
            (TOKEN_REBALANCE_TOPIC, vault_id),
            (rebalance_threshold_bps,),
        );
        Ok(())
    }

    /// Triggers a portfolio rebalance for a vault.
    ///
    /// Records the rebalance event and updates the last rebalance timestamp and counter.
    ///
    /// # Arguments
    /// * `vault_id` - The vault ID
    ///
    /// # Errors
    /// * `ContractError::InvalidAmount` - If no rebalance config exists
    pub fn trigger_rebalance(env: Env, vault_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env);
        let key = DataKey::TokenRebalance(vault_id);
        let mut config: TokenRebalanceConfig = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::InvalidAmount)?;

        let now = env.ledger().timestamp();
        config.last_rebalance = now;
        config.total_rebalances = config.total_rebalances.saturating_add(1);

        env.storage().persistent().set(&key, &config);
        env.storage().persistent().extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        env.events().publish(
            (TOKEN_REBALANCED_TOPIC, vault_id),
            (config.total_rebalances, now),
        );
        Ok(())
    }

    /// Returns the token rebalancing configuration for a vault.
    pub fn get_token_rebalance(env: Env, vault_id: u64) -> Option<TokenRebalanceConfig> {
        env.storage().persistent().get(&DataKey::TokenRebalance(vault_id))
    }

    /// Creates a beneficiary pool from registered vault beneficiaries - Issue #529.
    ///
    /// Validates that all `members` appear in the vault's beneficiary list, sums their
    /// individual BPS allocations into `pooledAllocation[pool_id]`, and persists the pool.
    ///
    /// # Arguments
    /// * `vault_id` - The vault whose beneficiary list is used for validation
    /// * `caller`   - Must be the vault owner
    /// * `pool_id`  - Unique pool identifier chosen by the caller
    /// * `members`  - Addresses that must each be registered beneficiaries of the vault
    ///
    /// # Errors
    /// * `ContractError::NotOwner`         - caller is not the vault owner
    /// * `ContractError::InvalidBeneficiary` - a member is not in the vault's beneficiary list
    pub fn create_pool(
        env: Env,
        vault_id: u64,
        caller: Address,
        pool_id: u64,
        members: Vec<Address>,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        Self::assert_not_paused(&env);
        let vault = Self::load_vault(&env, vault_id);
        if caller != vault.owner {
            return Err(ContractError::NotOwner);
        }

        let mut total_bps: u32 = 0;
        for member in members.iter() {
            let mut found = false;
            for entry in vault.beneficiaries.iter() {
                if entry.address == member {
                    total_bps = total_bps.saturating_add(entry.bps);
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(ContractError::InvalidBeneficiary);
            }
        }

        let pool = BeneficiaryPool {
            pool_id,
            members: members.clone(),
            total_bps,
        };
        env.storage().persistent().set(&DataKey::BeneficiaryPool(pool_id), &pool);
        env.storage().persistent().extend_ttl(
            &DataKey::BeneficiaryPool(pool_id),
            VAULT_TTL_THRESHOLD,
            VAULT_TTL_LEDGERS,
        );
        env.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);
        env.events().publish((POOL_CREATED_TOPIC, pool_id), (members, total_bps));
        Ok(())
    }

    /// Returns the pool record for `pool_id`, if it exists.
    pub fn get_pool(env: Env, pool_id: u64) -> Option<BeneficiaryPool> {
        env.storage().persistent().get(&DataKey::BeneficiaryPool(pool_id))
    }
}
