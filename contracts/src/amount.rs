//! Amount and BPS arithmetic helpers.
//!
//! All arithmetic in this module uses checked operations so that overflow and
//! underflow are surfaced as explicit errors instead of wrapping or panicking.
//! The crate enables `clippy::arithmetic_side_effects` (see `lib.rs`), so any
//! unchecked `+`, `-`, `*`, `/` on amounts or BPS will fail the lint.

use thiserror::Error;

/// Errors returned by amount and BPS arithmetic.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AmountError {
    /// Addition overflowed the underlying integer type.
    #[error("amount addition overflow")]
    AddOverflow,
    /// Subtraction underflowed the underlying integer type.
    #[error("amount subtraction underflow")]
    SubOverflow,
    /// Multiplication overflowed the underlying integer type.
    #[error("amount multiplication overflow")]
    MulOverflow,
    /// Division by zero was attempted.
    #[error("amount division by zero")]
    DivByZero,
    /// BPS value exceeded the maximum allowed (10_000 = 100%).
    #[error("bps value out of range")]
    BpsOutOfRange,
}

/// Basis points denominator: 10_000 BPS == 100%.
pub const BPS_DENOMINATOR: u128 = 10_000;

/// Checked addition of two amounts.
pub fn checked_add(a: u128, b: u128) -> Result<u128, AmountError> {
    a.checked_add(b).ok_or(AmountError::AddOverflow)
}

/// Checked subtraction of two amounts.
pub fn checked_sub(a: u128, b: u128) -> Result<u128, AmountError> {
    a.checked_sub(b).ok_or(AmountError::SubOverflow)
}

/// Checked multiplication of two amounts.
pub fn checked_mul(a: u128, b: u128) -> Result<u128, AmountError> {
    a.checked_mul(b).ok_or(AmountError::MulOverflow)
}

/// Checked division of two amounts.
pub fn checked_div(a: u128, b: u128) -> Result<u128, AmountError> {
    a.checked_div(b).ok_or(AmountError::DivByZero)
}

/// Apply a BPS rate to an amount using checked arithmetic.
///
/// Returns `amount * bps / BPS_DENOMINATOR`, rejecting BPS values above 100%.
pub fn apply_bps(amount: u128, bps: u128) -> Result<u128, AmountError> {
    if bps > BPS_DENOMINATOR {
        return Err(AmountError::BpsOutOfRange);
    }
    let scaled = checked_mul(amount, bps)?;
    checked_div(scaled, BPS_DENOMINATOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_ok_and_overflow() {
        assert_eq!(checked_add(1, 2), Ok(3));
        assert_eq!(checked_add(u128::MAX, 1), Err(AmountError::AddOverflow));
    }

    #[test]
    fn sub_ok_and_underflow() {
        assert_eq!(checked_sub(3, 2), Ok(1));
        assert_eq!(checked_sub(0, 1), Err(AmountError::SubOverflow));
    }

    #[test]
    fn mul_ok_and_overflow() {
        assert_eq!(checked_mul(3, 4), Ok(12));
        assert_eq!(checked_mul(u128::MAX, 2), Err(AmountError::MulOverflow));
    }

    #[test]
    fn div_ok_and_by_zero() {
        assert_eq!(checked_div(10, 2), Ok(5));
        assert_eq!(checked_div(1, 0), Err(AmountError::DivByZero));
    }

    #[test]
    fn apply_bps_boundaries() {
        assert_eq!(apply_bps(1_000, 0), Ok(0));
        assert_eq!(apply_bps(1_000, BPS_DENOMINATOR), Ok(1_000));
        assert_eq!(apply_bps(1_000, 5_000), Ok(500));
        assert_eq!(apply_bps(1_000, BPS_DENOMINATOR + 1), Err(AmountError::BpsOutOfRange));
    }

    #[test]
    fn apply_bps_overflow_is_reported() {
        assert_eq!(apply_bps(u128::MAX, 2), Err(AmountError::MulOverflow));
    }
}
