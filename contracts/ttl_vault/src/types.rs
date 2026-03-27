use soroban_sdk::{contracttype, symbol_short, Address, String, Symbol, Vec};

pub const RELEASE_TOPIC: Symbol = symbol_short!("release");
pub const VAULT_CREATED_TOPIC: Symbol = symbol_short!("v_created");
pub const PING_EXPIRY_TOPIC: Symbol = symbol_short!("ping_exp");

/// Warning threshold in seconds. If TTL remaining < this value, ping_expiry emits an event.
pub const EXPIRY_WARNING_THRESHOLD: u64 = 86_400; // 24 hours

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

/// A single beneficiary entry: (address, basis_points).
/// All entries in a vault's beneficiaries must sum to 10_000 bps (100%).
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryEntry {
    pub address: Address,
    pub bps: u32,
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
    pub status: ReleaseStatus,
    /// Multi-beneficiary split. Empty means use `beneficiary` (100%).
    pub beneficiaries: Vec<BeneficiaryEntry>,
    /// Optional short metadata string (label or IPFS hash).
    pub metadata: String,
}
