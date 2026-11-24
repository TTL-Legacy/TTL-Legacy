/// Approximate ledger close time in seconds (Stellar mainnet ~5s).
pub const LEDGER_SECOND: u32 = 5;
/// Number of seconds in one minute.
pub const SECONDS_PER_MINUTE: u64 = 60;
/// Number of seconds in one hour.
pub const SECONDS_PER_HOUR: u64 = 60 * SECONDS_PER_MINUTE;
/// Number of seconds in one day.
pub const SECONDS_PER_DAY: u64 = 24 * SECONDS_PER_HOUR;
/// Number of days in the simplified financial-year calculation.
pub const DAYS_PER_YEAR: u64 = 365;

/// Default maximum vault TTL (10 years, in seconds).
pub const DEFAULT_MAX_TTL_SECONDS: u64 = 10 * DAYS_PER_YEAR * SECONDS_PER_DAY;
/// Soroban maximum persistent entry TTL in ledgers (~180 days at 5s/ledger).
pub const MAX_PERSISTENT_TTL: u32 = 3_110_400;
/// Maximum number of snapshots retained per vault.
pub const MAX_SNAPSHOTS_PER_VAULT: u32 = 1_000;
/// Maximum number of vault IDs accepted by `get_vault_batch_status` in a single call.
pub const MAX_BATCH_STATUS_SIZE: u32 = 100;
/// Maximum number of vault IDs accepted by `get_vault_batch` in a single call.
pub const MAX_VAULT_BATCH_SIZE: u32 = 50;

/// Maximum number of vesting schedules per vault.
pub const MAX_VESTING_SCHEDULES: u32 = 20;

/// Maximum number of beneficiaries allowed per vault.
pub const MAX_BENEFICIARIES: u32 = 20;

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
/// Maximum length, in bytes, of a vault's release memo (Issue #791).
pub const MAX_RELEASE_MEMO_LEN: u32 = 256;

/// Maximum number of withdrawal events kept in the per-vault ring buffer.
///
/// Older entries are silently evicted once this limit is reached, keeping
/// on-chain storage bounded regardless of how many withdrawals a vault
/// accumulates over its lifetime.
pub const MAX_WITHDRAWAL_HISTORY: u32 = 50;

/// Default dispute window before a conflict can be auto-resolved (72 hours)
pub const DEFAULT_CONFLICT_DISPUTE_WINDOW: u64 = 72 * 60 * 60;
/// Minimum dispute window (1 hour)
pub const MIN_CONFLICT_DISPUTE_WINDOW: u64 = 60 * 60;
/// Maximum dispute window (30 days)
pub const MAX_CONFLICT_DISPUTE_WINDOW: u64 = 30 * 24 * 60 * 60;

/// Default minimum interval (seconds) between consecutive check-ins.
pub const DEFAULT_MIN_CHECKIN_COOLDOWN: u64 = 60;

/// Maximum number of seconds a single acceleration call may advance the deadline.
pub const MAX_ACCELERATE_SECONDS: u64 = 2_592_000;

/// Minimum hibernation duration in seconds - Issue #1280.
/// Mirrors `MIN_CHECK_IN_INTERVAL`: a hibernation shorter than the shortest
/// allowed check-in interval provides no meaningful relief from check-ins,
/// so durations below this (including zero, i.e. an instant no-op) are rejected.
pub const MIN_HIBERNATION_SECONDS: u64 = 3600; // 1 hour

/// Maximum hibernation duration in seconds.
/// Soroban's maximum persistent entry TTL is 3_110_400 ledgers at ~5 s/ledger ≈ 180 days.
/// We cap hibernation at that ceiling so storage extension can never fail.
pub const MAX_HIBERNATION_SECONDS: u64 = 15_552_000; // 180 days

/// Minimum check-in interval: 1 hour (3600 seconds)
/// Prevents abuse of TTL extension mechanisms with unreasonably short intervals
pub const MIN_CHECK_IN_INTERVAL: u64 = 3600;

/// Maximum check-in interval: 10 years (315_360_000 seconds)
/// Issue #1165: an unconditional ceiling — independent of the optional
/// admin-configured MaxCheckInInterval — prevents an astronomically large
/// interval (e.g. near u64::MAX) from overflowing TTL arithmetic or making
/// automatic release practically unreachable.
pub const MAX_CHECK_IN_INTERVAL: u64 = 315_360_000;

/// Oracle attestation freshness window in seconds (5 minutes) - Issue #1832.
pub const ORACLE_FRESHNESS_WINDOW: u64 = 300;
