//! Wallet policy: PIN rules, limits and fees.
//!
//! These are the rules the wallet enforces on the server and mirrors on the
//! client so the UI can validate before a round trip.

use crate::money::Money;

/// Minimum length of a wallet PIN.
pub const PIN_MIN_LEN: usize = 4;
/// Maximum length of a wallet PIN.
pub const PIN_MAX_LEN: usize = 6;
/// Number of digits in a one-time passcode.
pub const OTP_LEN: usize = 6;

/// Checks that a PIN is acceptable, returning a human readable reason if not.
pub fn validate_pin(pin: &str) -> Result<(), PinViolation> {
    if pin.len() < PIN_MIN_LEN || pin.len() > PIN_MAX_LEN {
        return Err(PinViolation::Length);
    }
    if !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(PinViolation::NotNumeric);
    }

    let digits: Vec<u32> = pin.chars().map(|c| c.to_digit(10).unwrap()).collect();

    if digits.iter().all(|d| *d == digits[0]) {
        return Err(PinViolation::RepeatedDigit);
    }

    // Reject simple runs: 1234, 4321, 1112 style sequences.
    let ascending = digits.windows(2).all(|w| w[1] == w[0] + 1);
    let descending = digits.windows(2).all(|w| w[1] == w[0].saturating_sub(1));
    if ascending || descending {
        return Err(PinViolation::Sequential);
    }

    // Reject a repeated pair such as 1212.
    if digits.len() >= 4 && digits.chunks(2).all(|c| c[0] == c[1]) {
        return Err(PinViolation::RepeatedDigit);
    }

    Ok(())
}

/// Why a PIN was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PinViolation {
    /// The PIN was the wrong length.
    #[error("PIN must be {PIN_MIN_LEN} to {PIN_MAX_LEN} digits")]
    Length,
    /// The PIN contained something other than digits.
    #[error("PIN must contain only digits")]
    NotNumeric,
    /// The PIN repeated a digit or a digit pair too obviously.
    #[error("PIN cannot repeat the same digit")]
    RepeatedDigit,
    /// The PIN was a simple ascending or descending run.
    #[error("PIN cannot be a simple sequence")]
    Sequential,
}

/// Limits and fees applied to a wallet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalletPolicy {
    /// Largest single transfer.
    pub max_transfer: Money,
    /// Largest total transferred in a day.
    pub max_daily_transfer: Money,
    /// Largest total transferred in a calendar month.
    pub max_monthly_transfer: Money,
    /// Largest single cash-in.
    pub max_cash_in: Money,
    /// Largest single cash-out.
    pub max_cash_out: Money,
    /// Largest single top-up from a bank transfer.
    pub max_top_up: Money,
    /// Fee charged on a person-to-person transfer.
    pub transfer_fee: Money,
    /// Fee charged on a cash-out.
    pub cash_out_fee: Money,
    /// Fee charged on a merchant QR payment.
    pub merchant_pay_fee: Money,
}

impl Default for WalletPolicy {
    fn default() -> Self {
        Self {
            max_transfer: Money::from_centavos(5_000_000),
            max_daily_transfer: Money::from_centavos(10_000_000),
            max_monthly_transfer: Money::from_centavos(50_000_000),
            max_cash_in: Money::from_centavos(5_000_000),
            max_cash_out: Money::from_centavos(5_000_000),
            max_top_up: Money::from_centavos(10_000_000),
            transfer_fee: Money::zero(),
            cash_out_fee: Money::from_centavos(1_500),
            merchant_pay_fee: Money::zero(),
        }
    }
}

/// Failure modes for PIN attempts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PinAttemptError {
    /// Too many incorrect PIN attempts in a row.
    #[error("too many incorrect PIN attempts")]
    Locked,
    /// The PIN did not match.
    #[error("incorrect PIN")]
    Incorrect,
}
