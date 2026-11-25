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

/// Minimum XLM balance (in stroops) the sponsor account must retain after paying
/// a fee bump. Stellar requires accounts to keep a base reserve; we treat this
/// as the fee threshold the sponsor must be able to cover.
const SPONSOR_FEE_THRESHOLD_STROOPS: i128 = 100;

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

/// Check that the sponsor account holds enough XLM to cover the required fee.
///
/// Returns a structured [`FeeSponsorsException::SponsorInsufficientBalance`]
/// error when `available` cannot cover `required`, rather than letting a raw
/// Stellar SDK error propagate to the caller.
///
/// # Arguments
/// * `available` - Sponsor account XLM balance in stroops
/// * `required` - Fee amount (in stroops) that must be covered
pub fn check_sponsor_balance(
    available: i128,
    required: i128,
) -> Result<(), FeeSponsorsException> {
    if available < required {
        return Err(FeeSponsorsException::SponsorInsufficientBalance {
            available,
            required,
        });
    }
    Ok(())
}

/// Map a raw Stellar SDK error into a structured fee sponsorship error.
///
/// Insufficient-balance errors reported by the SDK are translated into
/// [`FeeSponsorsException::SponsorInsufficientBalance`] so callers never see
/// the raw SDK error.
pub fn map_stellar_sdk_error(
    sdk_error: &str,
    available: i128,
    required: i128,
) -> FeeSponsorsException {
    let lowered = sdk_error.to_lowercase();
    if lowered.contains("insufficient")
        && (lowered.contains("balance") || lowered.contains("fund"))
    {
        FeeSponsorsException::SponsorInsufficientBalance { available, required }
    } else {
        FeeSponsorsException::SubmissionFailed(sdk_error.to_string())
    }
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
    construct_fee_bump_transaction_with_balance(
        beneficiary_account,
        sponsor_account,
        net_amount,
        memo,
        i128::MAX,
    )
}

/// Construct a fee bump transaction, first verifying the sponsor can cover the
/// required fee threshold.
///
/// # Arguments
/// * `beneficiary_account` - The beneficiary's Stellar account address
/// * `sponsor_account` - The backend's sponsor account address
/// * `net_amount` - Amount to transfer after protocol fee deduction
/// * `memo` - Optional transaction memo
/// * `sponsor_balance` - Sponsor account XLM balance in stroops
pub fn construct_fee_bump_transaction_with_balance(
    beneficiary_account: &str,
    sponsor_account: &str,
    net_amount: i128,
    memo: Option<&str>,
    sponsor_balance: i128,
) -> Result<String, FeeSponsorsException> {
    // Ensure the sponsor can cover the fee threshold before doing any work.
    check_sponsor_balance(sponsor_balance, SPONSOR_FEE_THRESHOLD_STROOPS)?;
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
    /// Sponsor account has insufficient balance to cover fees.
    InsufficientSponsorBalance { available: i128, required: i128 },
    /// Sponsor account has insufficient XLM to cover the fee threshold.
    SponsorInsufficientBalance { available: i128, required: i128 },
    /// Transaction submission failed.
    SubmissionFailed(String),
}

impl fmt::Display for FeeSponsorsException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAccount(msg) => write!(f, "invalid account: {}", msg),
            Self::InvalidAmount(msg) => write!(f, "invalid amount: {}", msg),
            Self::InsufficientSponsorBalance { available, required } => write!(
                f,
                "insufficient sponsor balance: available={}, required={}",
                available, required
            ),
            Self::SponsorInsufficientBalance { available, required } => write!(
                f,
                "sponsor insufficient balance: available={}, required={}",
                available, required
            ),
            Self::SubmissionFailed(msg) => write!(f, "submission failed: {}", msg),
        }
    }
}

impl std::error::Error for FeeSponsorsException {}

/// Minimal in-process Soroban sandbox used by the integration tests below.
///
/// The real network is not available in unit tests, so this harness models the
/// observable behaviour of a local Soroban sandbox: it tracks sponsor balances,
/// applies the protocol fee, and rejects transactions the sponsor cannot fund.
#[cfg(test)]
mod sandbox {
    use super::*;

    /// A local Soroban sandbox with a funded sponsor account.
    pub struct SorobanSandbox {
        pub sponsor_account: String,
        pub sponsor_balance: i128,
        pub ledger_sequence: u64,
    }

    impl SorobanSandbox {
        /// Spin up a sandbox with a sponsor account funded with `balance` stroops.
        pub fn start(sponsor_balance: i128) -> Self {
            Self {
                sponsor_account: "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
                    .to_string(),
                sponsor_balance,
                ledger_sequence: 1,
            }
        }

        /// Execute a sponsored claim end-to-end against the sandbox.
        pub fn sponsored_claim(
            &mut self,
            beneficiary: &str,
            released_amount: i128,
            stellar_base_fee: i128,
            fee_bump_premium: i128,
        ) -> Result<SponsoredRelease, FeeSponsorsException> {
            let breakdown =
                FeeBreakdown::new(released_amount, stellar_base_fee, fee_bump_premium);

            if self.sponsor_balance < breakdown.total_sponsor_fee {
                return Err(FeeSponsorsException::InsufficientSponsorBalance {
                    available: self.sponsor_balance,
                    required: breakdown.total_sponsor_fee,
                });
            }

            let tx_hash = construct_fee_bump_transaction_with_balance(
                beneficiary,
                &self.sponsor_account,
                breakdown.net_amount,
                None,
                self.sponsor_balance,
            )?;

            self.sponsor_balance -= breakdown.total_sponsor_fee;
            self.ledger_sequence += 1;

            Ok(SponsoredRelease {
                tx_id: format!("tx-{}", self.ledger_sequence),
                vault_id: "vault-1".to_string(),
                beneficiary: beneficiary.to_string(),
                released_amount,
                protocol_fee: breakdown.protocol_fee,
                net_amount: breakdown.net_amount,
                fee_bump_tx_hash: tx_hash,
                sponsor_account: self.sponsor_account.clone(),
                sponsorship_fee: breakdown.total_sponsor_fee,
                status: SponsoredReleaseStatus::Confirmed,
                created_at: Utc::now(),
                executed_at: Some(Utc::now()),
                ledger_sequence: Some(self.ledger_sequence),
                error: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sandbox::SorobanSandbox;
    use super::*;

    const BENEFICIARY: &str = "GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

    #[test]
    fn sponsored_claim_end_to_end() {
        // Spin up a local Soroban sandbox with a funded sponsor.
        let mut sandbox = SorobanSandbox::start(1_000_000);
        let released_amount = 10_000_000i128;

        let release = sandbox
            .sponsored_claim(BENEFICIARY, released_amount, 100, 200)
            .expect("sponsored claim should succeed");

        // Protocol fee is 0.1% of the released amount.
        assert_eq!(release.protocol_fee, 10_000);
        assert_eq!(release.net_amount, released_amount - 10_000);
        assert_eq!(release.sponsorship_fee, 300);
        assert_eq!(release.status, SponsoredReleaseStatus::Confirmed);
        assert_eq!(release.fee_bump_tx_hash.len(), 64);
        assert!(release.executed_at.is_some());
        assert!(release.ledger_sequence.is_some());

        // Sponsor balance is debited by exactly the sponsorship fee.
        assert_eq!(sandbox.sponsor_balance, 1_000_000 - 300);
    }

    #[test]
    fn insufficient_sponsor_balance_is_rejected() {
        // Sponsor cannot cover the fee bump premium.
        let mut sandbox = SorobanSandbox::start(50);

        let err = sandbox
            .sponsored_claim(BENEFICIARY, 10_000_000, 100, 200)
            .expect_err("claim should fail when sponsor is underfunded");

        match err {
            FeeSponsorsException::InsufficientSponsorBalance { available, required } => {
                assert_eq!(available, 50);
                assert_eq!(required, 300);
            }
            other => panic!("unexpected error: {:?}", other),
        }

        // No ledger was consumed and the balance is untouched.
        assert_eq!(sandbox.sponsor_balance, 50);
        assert_eq!(sandbox.ledger_sequence, 1);
    }

    #[test]
    fn check_sponsor_balance_rejects_below_threshold() {
        let err = check_sponsor_balance(50, SPONSOR_FEE_THRESHOLD_STROOPS)
            .expect_err("balance below threshold should be rejected");

        match err {
            FeeSponsorsException::SponsorInsufficientBalance { available, required } => {
                assert_eq!(available, 50);
                assert_eq!(required, SPONSOR_FEE_THRESHOLD_STROOPS);
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn check_sponsor_balance_accepts_at_threshold() {
        assert!(check_sponsor_balance(SPONSOR_FEE_THRESHOLD_STROOPS, SPONSOR_FEE_THRESHOLD_STROOPS)
            .is_ok());
    }

    #[test]
    fn construct_fee_bump_rejects_underfunded_sponsor() {
        let err = construct_fee_bump_transaction_with_balance(
            BENEFICIARY,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
            10_000_000,
            None,
            10,
        )
        .expect_err("underfunded sponsor should be rejected");

        match err {
            FeeSponsorsException::SponsorInsufficientBalance { available, required } => {
                assert_eq!(available, 10);
                assert_eq!(required, SPONSOR_FEE_THRESHOLD_STROOPS);
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn sdk_insufficient_balance_error_is_mapped() {
        let err = map_stellar_sdk_error(
            "tx_insufficient_balance: account has insufficient balance",
            25,
            100,
        );

        match err {
            FeeSponsorsException::SponsorInsufficientBalance { available, required } => {
                assert_eq!(available, 25);
                assert_eq!(required, 100);
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn sdk_other_error_is_mapped_to_submission_failed() {
        let err = map_stellar_sdk_error("tx_bad_seq", 1_000, 100);

        match err {
            FeeSponsorsException::SubmissionFailed(msg) => assert_eq!(msg, "tx_bad_seq"),
            other => panic!("unexpected error: {:?}", other),
        }
    }
}
