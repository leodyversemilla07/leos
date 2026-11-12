//! Money: Philippine Peso amounts represented as integer centavos.
//!
//! Floating point is never used for balances. Every amount crossing a
//! boundary (DB, FFI, API) is transported as an integer number of centavos.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// One peso expressed in centavos.
pub const CENTAVOS_PER_PESO: i64 = 100;

/// Largest amount the wallet will ever represent: one billion pesos.
pub const MAX_CENTAVOS: i64 = 100_000_000 * CENTAVOS_PER_PESO;

/// A non-negative peso amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Money {
    centavos: i64,
}

impl Money {
    /// Zero pesos.
    pub const fn zero() -> Self {
        Self { centavos: 0 }
    }

    /// Builds an amount from an exact number of centavos.
    pub const fn from_centavos(centavos: i64) -> Self {
        Self { centavos }
    }

    /// Builds an amount from peso and centavos parts.
    ///
    /// Returns `None` when `centavos` is negative or above 99.
    pub const fn from_parts(pesos: i64, centavos: i64) -> Option<Self> {
        if centavos < 0 || centavos >= CENTAVOS_PER_PESO {
            return None;
        }
        Some(Self {
            centavos: pesos * CENTAVOS_PER_PESO + centavos,
        })
    }

    /// The exact amount in centavos.
    pub const fn centavos(self) -> i64 {
        self.centavos
    }

    /// The whole peso component.
    pub const fn pesos(self) -> i64 {
        self.centavos / CENTAVOS_PER_PESO
    }

    /// The sub-peso component, always in the range `0..100`.
    pub const fn fractional_centavos(self) -> i64 {
        self.centavos % CENTAVOS_PER_PESO
    }

    /// Whether the amount is exactly zero.
    pub const fn is_zero(self) -> bool {
        self.centavos == 0
    }

    /// Whether the amount is greater than zero.
    pub const fn is_positive(self) -> bool {
        self.centavos > 0
    }

    /// Adds two amounts, saturating instead of overflowing.
    pub fn saturating_add(self, other: Self) -> Self {
        Self {
            centavos: self.centavos.saturating_add(other.centavos),
        }
    }

    /// Adds two amounts, returning `None` on overflow.
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.centavos
            .checked_add(other.centavos)
            .map(|centavos| Self { centavos })
    }

    /// Subtracts two amounts, returning `None` when the result would be negative.
    ///
    /// A wallet balance must never go below zero, so underflow is an error
    /// rather than a saturating clamp.
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.centavos
            .checked_sub(other.centavos)
            .filter(|c| *c >= 0)
            .map(|centavos| Self { centavos })
    }

    /// Splits an amount into a net part and a fee part that sum back to it.
    ///
    /// Used when a fee is charged on top of a transfer: the sender is debited
    /// the full amount, the recipient is credited the net amount, and the fee
    /// is credited to the fee account.
    pub fn split_fee(self, fee: Self) -> Option<(Self, Self)> {
        if fee.centavos > self.centavos {
            return None;
        }
        Some((self.checked_sub(fee)?, fee))
    }

    /// Parses a user-entered peso string.
    ///
    /// Accepts an optional currency prefix, thousands separators, and one or two
    /// decimal places. Rejects anything with more precision than centavos.
    pub fn parse(input: &str) -> Result<Self, MoneyParseError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(MoneyParseError::Empty);
        }

        // An optional currency prefix, in either spelling.
        let trimmed = if trimmed.len() >= 3 && trimmed[..3].eq_ignore_ascii_case("php") {
            &trimmed[3..]
        } else {
            trimmed
        };

        let mut digits = String::with_capacity(trimmed.len());
        let mut decimals: Option<u32> = None;
        let mut seen_digit = false;

        for ch in trimmed.chars() {
            match ch {
                '₱' => {
                    // A currency marker may only lead the input.
                    if seen_digit {
                        return Err(MoneyParseError::Malformed);
                    }
                }
                ',' | '_' | ' ' => {}
                '.' => {
                    if decimals.is_some() {
                        return Err(MoneyParseError::Malformed);
                    }
                    decimals = Some(0);
                }
                '0'..='9' => {
                    seen_digit = true;
                    digits.push(ch);
                    if let Some(count) = decimals.as_mut() {
                        *count += 1;
                        if *count > 2 {
                            return Err(MoneyParseError::TooPrecise);
                        }
                    }
                }
                _ => return Err(MoneyParseError::Malformed),
            }
        }

        if !seen_digit {
            return Err(MoneyParseError::Empty);
        }

        let (whole, frac) = match decimals {
            Some(count) => {
                let split = digits.len() - count as usize;
                let (whole, frac) = digits.split_at(split);
                let mut frac_padded = frac.to_string();
                while frac_padded.len() < 2 {
                    frac_padded.push('0');
                }
                (
                    whole.to_string(),
                    frac_padded
                        .parse::<i64>()
                        .map_err(|_| MoneyParseError::Malformed)?,
                )
            }
            None => (digits.clone(), 0),
        };

        let whole: i64 = if whole.is_empty() {
            0
        } else {
            whole
                .parse::<i64>()
                .map_err(|_| MoneyParseError::Malformed)?
        };

        if whole > MAX_CENTAVOS / CENTAVOS_PER_PESO {
            return Err(MoneyParseError::TooLarge);
        }

        let centavos = whole
            .checked_mul(CENTAVOS_PER_PESO)
            .and_then(|c| c.checked_add(frac))
            .ok_or(MoneyParseError::TooLarge)?;

        if centavos > MAX_CENTAVOS {
            return Err(MoneyParseError::TooLarge);
        }

        Ok(Self { centavos })
    }
}

/// Formats as a plain decimal string, e.g. `1234.56`.
impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02}", self.pesos(), self.fractional_centavos())
    }
}

/// On the wire a peso amount is a decimal string such as `"1234.56"`.
///
/// Strings keep the value exact across JSON, the FFI boundary and the
/// database, which matters more than readability here: a peso balance that
/// drifts by a centavo is a support ticket.
impl Serialize for Money {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// Deserializes from a decimal string, or from an integer number of centavos.
impl<'de> Deserialize<'de> for Money {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Decimal(String),
            Centavos(i64),
        }

        match Raw::deserialize(deserializer)? {
            Raw::Decimal(text) => Money::parse(&text).map_err(serde::de::Error::custom),
            Raw::Centavos(centavos) => Ok(Money::from_centavos(centavos)),
        }
    }
}

impl Money {
    /// Formats for display, e.g. `LC 1,234.56`.
    pub fn to_display_string(self) -> String {
        crate::currency::format(self)
    }

    /// Formats with thousands separators and no currency symbol, e.g. `1,234.56`.
    pub fn with_thousands_separator(self) -> String {
        let whole = self.pesos().abs().to_string();
        let mut grouped = String::with_capacity(whole.len() + whole.len() / 3);
        for (index, ch) in whole.chars().enumerate() {
            if index > 0 && (whole.len() - index) % 3 == 0 {
                grouped.push(',');
            }
            grouped.push(ch);
        }
        let sign = if self.centavos < 0 { "-" } else { "" };
        format!("{sign}{grouped}.{:02}", self.fractional_centavos())
    }
}

/// Failure modes for [`Money::parse`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MoneyParseError {
    /// The input was empty or contained no digits.
    #[error("amount is empty")]
    Empty,
    /// The input contained characters that cannot appear in a peso amount.
    #[error("amount contains characters that are not valid")]
    Malformed,
    /// The input carried more precision than a centavo.
    #[error("amount has more than two decimal places")]
    TooPrecise,
    /// The amount exceeds the largest value the wallet can represent.
    #[error("amount is too large")]
    TooLarge,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_plain_decimal() {
        assert_eq!(
            Money::parse("1234.56").unwrap(),
            Money::from_centavos(123_456)
        );
        assert_eq!(Money::parse("0.99").unwrap(), Money::from_centavos(99));
        assert_eq!(Money::parse("100").unwrap(), Money::from_centavos(10_000));
    }

    #[test]
    fn parses_with_currency_symbol_and_separators() {
        assert_eq!(
            Money::parse("₱1,234.56").unwrap(),
            Money::from_centavos(123_456)
        );
        assert_eq!(
            Money::parse("  1 234.5 ").unwrap(),
            Money::from_centavos(123_450)
        );
        assert_eq!(
            Money::parse("PHP 500.00").unwrap(),
            Money::from_centavos(50_000)
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(Money::parse(""), Err(MoneyParseError::Empty));
        assert_eq!(Money::parse("abc"), Err(MoneyParseError::Malformed));
        assert_eq!(Money::parse("1.234"), Err(MoneyParseError::TooPrecise));
        assert_eq!(Money::parse("1.2.3"), Err(MoneyParseError::Malformed));
        assert_eq!(Money::parse("-5"), Err(MoneyParseError::Malformed));
        assert_eq!(
            Money::parse("999999999999").unwrap_err(),
            MoneyParseError::TooLarge
        );
    }

    #[test]
    fn formats_with_separators() {
        assert_eq!(
            Money::from_centavos(123_456).to_display_string(),
            "LC 1,234.56"
        );
        assert_eq!(Money::from_centavos(100).to_display_string(), "LC 1.00");
        assert_eq!(
            Money::from_centavos(10_000_000).to_display_string(),
            "LC 100,000.00"
        );
        assert_eq!(
            Money::from_centavos(100_000_000).to_display_string(),
            "LC 1,000,000.00"
        );
        assert_eq!(Money::zero().to_display_string(), "LC 0.00");
    }

    #[test]
    fn subtraction_never_goes_negative() {
        let balance = Money::from_centavos(500);
        let spend = Money::from_centavos(700);
        assert_eq!(balance.checked_sub(spend), None);
        assert_eq!(
            balance.checked_sub(Money::from_centavos(200)).unwrap(),
            Money::from_centavos(300)
        );
    }

    #[test]
    fn splits_fee() {
        let total = Money::from_centavos(10_000);
        let fee = Money::from_centavos(50);
        let (net, taken) = total.split_fee(fee).unwrap();
        assert_eq!(net, Money::from_centavos(9_950));
        assert_eq!(taken, fee);
        assert_eq!(net.checked_add(taken).unwrap(), total);
        assert!(total.split_fee(Money::from_centavos(10_001)).is_none());
    }

    #[test]
    fn from_parts_validates_range() {
        assert_eq!(
            Money::from_parts(10, 5).unwrap(),
            Money::from_centavos(1_005)
        );
        assert_eq!(Money::from_parts(10, 100), None);
        assert_eq!(Money::from_parts(10, -1), None);
    }
}
