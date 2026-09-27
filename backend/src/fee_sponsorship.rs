use chrono::{DateTime, Utc};
/// Fee sponsorship module for sponsored vault release transactions.
///
/// Implements fee bump transaction construction and protocol fee handling
/// to allow beneficiaries without XLM to claim released vaults.
///
/// Issue #1122: Implement Fee Sponsorship for Beneficiary Release Transactions
use serde::{Deserialize, Serialize};
use std::fmt;

/// Protocol fee charged for sponsored release transactions (0.1% of released amount).
const PROTOCOL_FEE_BASIS_POINTS: u64 = 10; // 10 bps = 0.1%

/// Represents a sponsored release transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SponsoredRelease {
    /// Unique transaction ID.
    pub tx_id: String,
    /// Vault ID being released.
    pub vault_id: String,
    /// Beneficiary address claiming the funds.
    pub beneficiary: String,
    /// Released amount in stroops (smallest XLM unit).
    pub released_amount: i128,
    /// Protocol fee deducted (0.1% of released_amount).
    pub protocol_fee: i128,
    /// Amount actually transferred to beneficiary.
    pub net_amount: i128,
    /// Stellar fee bump transaction hash.
    pub fee_bump_tx_hash: String,
    /// Backend sponsor account address.
    pub sponsor_account: String,
    /// Transaction fee paid by sponsor (in stroops).
    pub sponsorship_fee: i128,
    /// Status of the sponsored transaction.
    pub status: SponsoredReleaseStatus,
    /// When this sponsorship was created.
    pub created_at: DateTime<Utc>,
    /// When this transaction was executed (if successful).
    pub executed_at: Option<DateTime<Utc>>,
    /// Ledger sequence of the executed transaction.
    pub ledger_sequence: Option<u64>,
    /// Optional error message if execution failed.
    pub error: Option<String>,
}

/// Status of a sponsored release transaction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SponsoredReleaseStatus {
    /// Transaction constructed, awaiting submission.
    Pending,
    /// Successfully submitted to Stellar network.
    Submitted,
    /// Transaction included in ledger and confirmed.
    Confirmed,
    /// Transaction failed to execute.
    Failed,
    /// Transaction cancelled or expired.
    Cancelled,
}

impl fmt::Display for SponsoredReleaseStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Submitted => write!(f, "submitted"),
            Self::Confirmed => write!(f, "confirmed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Request body for POST /api/vaults/{id}/sponsored-release.
#[derive(Debug, Deserialize)]
pub struct SponsoredReleaseRequest {
    /// Beneficiary's Stellar account address.
    pub beneficiary_account: String,
    /// Optional reference/memo for logging.
    pub memo: Option<String>,
}

/// Response for sponsored release creation.
#[derive(Debug, Serialize)]
pub struct SponsoredReleaseResponse {
    /// The sponsored transaction details.
    pub transaction: SponsoredRelease,
    /// Fee breakdown for transparency.
    pub fee_breakdown: FeeBreakdown,
}

/// Transparent fee breakdown.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeeBreakdown {
    /// Released vault amount.
    pub gross_amount: i128,
    /// Protocol fee (0.1% of released_amount).
    pub protocol_fee: i128,
    /// Stellar base transaction fee.
    pub stellar_base_fee: i128,
    /// Additional fee for fee bump transaction.
    pub fee_bump_premium: i128,
    /// Total fee paid by sponsor.
    pub total_sponsor_fee: i128,
    /// Net amount received by beneficiary.
    pub net_amount: i128,
}

impl FeeBreakdown {
    /// Create a new fee breakdown for a released amount.
    pub fn new(released_amount: i128, stellar_base_fee: i128, fee_bump_premium: i128) -> Self {
        let protocol_fee = calculate_protocol_fee(released_amount);
        let total_sponsor_fee = stellar_base_fee + fee_bump_premium;
        let net_amount = released_amount - protocol_fee;

        Self {
            gross_amount: released_amount,
            protocol_fee,
            stellar_base_fee,
            fee_bump_premium,
            total_sponsor_fee,
            net_amount,
        }
    }
}

/// Calculate the protocol fee (0.1% of amount).
pub fn calculate_protocol_fee(amount: i128) -> i128 {
    (amount * PROTOCOL_FEE_BASIS_POINTS as i128) / 10_000
}

/// Construct a fee bump transaction wrapping the beneficiary's release operation.
///
/// # Arguments
/// * `beneficiary_account` - The beneficiary's Stellar account address
/// * `sponsor_account` - The backend's sponsor account address
/// * `net_amount` - Amount to transfer after protocol fee deduction
/// * `memo` - Optional transaction memo
///
/// # Returns
/// A fee bump transaction hash (in production, would return XDR-encoded transaction)
pub fn construct_fee_bump_transaction(
    beneficiary_account: &str,
    sponsor_account: &str,
    net_amount: i128,
    memo: Option<&str>,
) -> Result<String, FeeSponsorsException> {
    // Validate accounts
    if beneficiary_account.is_empty() || !is_valid_stellar_account(beneficiary_account) {
        return Err(FeeSponsorsException::InvalidAccount(format!(
            "Invalid beneficiary account: {}",
            beneficiary_account
        )));
    }

    if sponsor_account.is_empty() || !is_valid_stellar_account(sponsor_account) {
        return Err(FeeSponsorsException::InvalidAccount(format!(
            "Invalid sponsor account: {}",
            sponsor_account
        )));
    }

    // Validate amount
    if net_amount <= 0 {
        return Err(FeeSponsorsException::InvalidAmount(
            "Release amount must be positive".to_string(),
        ));
    }

    // In production, this would:
    // 1. Create a payment operation from vault account to beneficiary
    // 2. Wrap it in a fee bump transaction with sponsor account
    // 3. Sign the inner transaction with vault's signing key
    // 4. Sign the fee bump with sponsor's key
    // 5. Return the XDR-encoded transaction
    //
    // For now, we generate a mock transaction hash for testing/demonstration
    let tx_hash = generate_mock_tx_hash(beneficiary_account, net_amount, memo);

    tracing::info!(
        beneficiary = beneficiary_account,
        sponsor = sponsor_account,
        amount = net_amount,
        memo = ?memo,
        tx_hash = %tx_hash,
        "fee bump transaction constructed"
    );

    Ok(tx_hash)
}

/// Check if a string is a valid Stellar account address.
/// Stellar accounts are 56-character base32-encoded strings starting with 'G'.
fn is_valid_stellar_account(account: &str) -> bool {
    if account.len() != 56 {
        return false;
    }

    if !account.starts_with('G') {
        return false;
    }

    // Check if all characters are valid base32
    account[1..]
        .chars()
        .all(|c| "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".contains(c))
}

/// Generate a mock transaction hash for testing.
///
/// This helper is only compiled in test builds. Production code must never
/// fabricate a transaction hash; it must submit a real fee-bump transaction
/// via Soroban RPC and return the hash reported by the network.
#[cfg(test)]
fn generate_mock_tx_hash(beneficiary: &str, amount: i128, memo: Option<&str>) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    beneficiary.hash(&mut hasher);
    amount.hash(&mut hasher);
    if let Some(m) = memo {
        m.hash(&mut hasher);
    }
    let hash = hasher.finish();

    format!("{:064x}", hash)
}

/// Exception types for fee sponsorship operations.
#[derive(Debug, Clone)]
pub enum FeeSponsorsException {
    /// Invalid Stellar account address.
    InvalidAccount(String),
    /// Invalid release amount.
    InvalidAmount(String),
    /// Vault not found or already released.
    VaultNotFound(String),
    /// Insufficient balance to deduct protocol fee.
    InsufficientBalance(String),
    /// Fee bump transaction construction failed.
    TransactionConstr

/* … truncated 5501 chars — edit only what you need near the top … */
