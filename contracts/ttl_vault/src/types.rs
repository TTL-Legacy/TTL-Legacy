use soroban_sdk::{contracttype, symbol_short, Address, Bytes, BytesN, String, Symbol, Vec};

pub const RELEASE_TOPIC: Symbol = symbol_short!("release");
pub const VAULT_CREATED_TOPIC: Symbol = symbol_short!("v_created");
pub const PING_EXPIRY_TOPIC: Symbol = symbol_short!("ping_exp");
pub const DEPOSIT_TOPIC: Symbol = symbol_short!("deposit");
pub const WITHDRAW_TOPIC: Symbol = symbol_short!("withdraw");
pub const CHECK_IN_TOPIC: Symbol = symbol_short!("check_in");
pub const CANCEL_TOPIC: Symbol = symbol_short!("cancel");
pub const OWNERSHIP_TOPIC: Symbol = symbol_short!("own_xfer");
pub const OWNERSHIP_INITIATED_TOPIC: Symbol = symbol_short!("own_init");
pub const OWNERSHIP_ACCEPTED_TOPIC: Symbol = symbol_short!("own_acc");
pub const OWNERSHIP_CANCELLED_TOPIC: Symbol = symbol_short!("own_can");
pub const BENEFICIARY_UPDATED_TOPIC: Symbol = symbol_short!("ben_upd");
pub const SET_BENEFICIARIES_TOPIC: Symbol = symbol_short!("set_bens");
pub const UPDATE_INTERVAL_TOPIC: Symbol = symbol_short!("upd_intv");
pub const UPDATE_METADATA_TOPIC: Symbol = symbol_short!("upd_meta");
pub const SET_MIN_INTERVAL_TOPIC: Symbol = symbol_short!("set_min");
pub const SET_MAX_INTERVAL_TOPIC: Symbol = symbol_short!("set_max");
pub const PAUSE_TOPIC: Symbol = symbol_short!("pause");
pub const UNPAUSE_TOPIC: Symbol = symbol_short!("unpause");
pub const SET_VESTING_TOPIC: Symbol = symbol_short!("set_vest");
pub const CLAIM_VEST_TOPIC: Symbol = symbol_short!("clm_vest");
pub const PAUSE_VAULT_TOPIC: Symbol = symbol_short!("v_pause");
pub const RESUME_VAULT_TOPIC: Symbol = symbol_short!("v_resume");
pub const SET_METADATA_TOPIC: Symbol = symbol_short!("set_meta");
pub const INHERITANCE_TOPIC: Symbol = symbol_short!("inherit");
pub const ADD_PASSKEY_TOPIC: Symbol = symbol_short!("add_pk");
pub const REMOVE_PASSKEY_TOPIC: Symbol = symbol_short!("rm_pk");
pub const ROTATE_PASSKEY_TOPIC: Symbol = symbol_short!("rot_pk");
pub const BACKUP_CODE_USED_TOPIC: Symbol = symbol_short!("bk_used");
pub const BACKUP_CODES_GENERATED_TOPIC: Symbol = symbol_short!("bk_gen");
pub const DELEGATE_BENEFICIARY_TOPIC: Symbol = symbol_short!("del_ben");
pub const DISPUTE_FILED_TOPIC: Symbol = symbol_short!("disp_fil");
pub const DISPUTE_RESOLVED_TOPIC: Symbol = symbol_short!("disp_res");
pub const WITHDRAWAL_SCHEDULED_TOPIC: Symbol = symbol_short!("wd_sch");
pub const WITHDRAWAL_EXECUTED_TOPIC: Symbol = symbol_short!("wd_exec");
pub const CONDITIONS_ACCEPTED_TOPIC: Symbol = symbol_short!("cond_acc");
pub const SET_SPENDING_LIMIT_TOPIC: Symbol = symbol_short!("set_slmt");

/// Warning threshold in seconds. If TTL remaining < this value, ping_expiry emits an event.
pub const EXPIRY_WARNING_THRESHOLD: u64 = 86_400; // 24 hours

/// Recovery extension duration in seconds (30 days)
pub const RECOVERY_EXTENSION_DURATION: u64 = 2_592_000;

/// Maximum length for vault metadata string
pub const MAX_METADATA_LEN: u32 = 256;

/// Maximum length for vault name
pub const MAX_NAME_LEN: u32 = 64;

/// Maximum length for vault description
pub const MAX_DESCRIPTION_LEN: u32 = 512;

/// Maximum length for vault notes
pub const MAX_NOTES_LEN: u32 = 1024;

/// Maximum length for custom metadata bytes (2KB) - Issue #378
pub const MAX_CUSTOM_METADATA_LEN: u32 = 2048;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Vault(u64),
    OwnerVaults(Address),
    BeneficiaryVaults(Address),
    VaultCount,
    TokenAddress,
    Admin,
    Paused,
    PendingAdmin,
    MinCheckInInterval,
    MaxCheckInInterval,
    Version,
    VestingSchedule(u64),
    TokenWhitelist(Address),
    VaultMetadata(u64),
    ParentVault(u64),
    VaultPasskeys(u64),
    BackupCodes(u64),
    BeneficiaryDelegate(u64),
    WithdrawalSchedule(u64),
    DisputeStatus(u64),
    ConditionalAcceptance(u64),
    ArchivedVault(u64),
}

/// A vesting schedule attached to a vault.
/// Funds are released in `num_installments` equal tranches, each separated by `interval` seconds.
/// The first installment becomes claimable at `start_time`.
#[contracttype]
#[derive(Clone)]
pub struct VestingSchedule {
    /// Unix timestamp when the first installment becomes claimable.
    pub start_time: u64,
    /// Seconds between consecutive installments.
    pub interval: u64,
    /// Total number of installments.
    pub num_installments: u32,
    /// Number of installments already claimed.
    pub claimed_installments: u32,
    /// Total amount to vest (in stroops). Each installment = total_amount / num_installments,
    /// with the last installment absorbing any remainder.
    pub total_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ReleaseStatus {
    Locked,
    Released,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ReleaseCondition {
    OnExpiry,
    OnProof(u32),
    Tranche(Vec<(u64, u32)>),
}

#[contracttype]
#[derive(Clone)]
pub struct ReleaseEvent {
    pub vault_id: u64,
    pub beneficiary: Address,
    pub amount: i128,
}

/// A single beneficiary entry: (address, basis_points).
/// All entries in a vault's beneficiaries must sum to 10_000 bps (100%).
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryEntry {
    pub address: Address,
    pub bps: u32,
}

/// Bridge configuration for cross-chain support.
#[contracttype]
#[derive(Clone)]
pub struct BridgeConfig {
    pub chain_id: u32,
    pub bridge_address: Address,
    pub is_active: bool,
}

/// Passkey hash for multi-passkey support - Issue #394
#[contracttype]
#[derive(Clone)]
pub struct PasskeyHash {
    pub hash: BytesN<32>,
    pub added_at: u64,
}

/// Backup code entry - Issue #393
#[contracttype]
#[derive(Clone)]
pub struct BackupCode {
    pub code: String,
    pub used: bool,
}

/// Withdrawal approval request - Issue #404
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalRequest {
    pub request_id: u64,
    pub amount: i128,
    pub requested_at: u64,
    pub approved: bool,
}

/// Deposit proof - Issue #405
#[contracttype]
#[derive(Clone)]
pub struct DepositProof {
    pub vault_id: u64,
    pub amount: i128,
    pub timestamp: u64,
    pub proof_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone)]
pub struct Vault {
    pub owner: Address,
    /// Primary beneficiary kept for backwards-compatible single-beneficiary reads.
    /// When beneficiaries is non-empty, this field is ignored during trigger_release.
    pub beneficiary: Address,
    pub balance: i128,
    pub check_in_interval: u64, // seconds
    pub last_check_in: u64,     // ledger timestamp
    pub created_at: u64,        // vault creation timestamp
    pub status: ReleaseStatus,
    /// Multi-beneficiary split. Empty means use `beneficiary` (100%).
    pub beneficiaries: Vec<BeneficiaryEntry>,
    /// Optional short metadata string (label or IPFS hash).
    pub metadata: String,
    /// Token contract address for this vault. Uses default XLM token if not specified.
    pub token_address: Address,
    /// Custom metadata as bytes (max 2KB) - Issue #378
    pub custom_metadata: Bytes,
    /// Whether the vault is paused - Issue #380
    pub is_paused: bool,
    /// Release condition for the vault - Issue #379
    pub release_condition: ReleaseCondition,
    /// Parent vault ID for inheritance chain - Issue #381
    pub parent_vault_id: Option<u64>,
    /// Primary passkey hash for backwards compatibility - Issue #392, #394
    pub passkey_hash: Option<BytesN<32>>,
    /// Maximum deposit amount - Issue #403
    pub max_deposit_amount: Option<i128>,
    /// Withdrawal approval threshold - Issue #404
    pub withdrawal_approval_threshold: Option<i128>,
    /// Maximum amount releasable per trigger_release call - Issue #382
    pub spending_limit: Option<i128>,
}

/// Passkey usage entry for tracking check-ins - Issue #395
#[contracttype]
#[derive(Clone)]
pub struct PasskeyUsageEntry {
    pub passkey_hash: BytesN<32>,
    pub timestamp: u64,
}

/// Beneficiary status enum - Issue #397
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum BeneficiaryStatus {
    Pending,
    Accepted,
    Declined,
}

/// Dispute status enum - Issue #399
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DisputeStatus {
    None,
    Filed,
    Resolved,
}

/// Withdrawal schedule entry - Issue #402
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalScheduleEntry {
    pub timestamp: u64,
    pub amount: i128,
}

/// Conditional acceptance entry - Issue #400
#[contracttype]
#[derive(Clone)]
pub struct ConditionalAcceptanceEntry {
    pub conditions: String,
    pub approved_by_owner: bool,
}

/// Archived vault info for restoration - Issue #443
#[contracttype]
#[derive(Clone)]
pub struct ArchivedVaultInfo(pub Vault);
