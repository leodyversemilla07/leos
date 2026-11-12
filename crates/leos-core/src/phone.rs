//! Philippine mobile number normalization.
//!
//! Leos identifies users by mobile number in E.164 form (`+639171234567`).
//! Users type their number many ways, so every inbound string is normalized
//! to a canonical form before it reaches the database.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The country code for the Philippines.
pub const PH_COUNTRY_CODE: &str = "63";

/// A Philippine phone number in E.164 form.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PhoneNumber {
    /// E.164 form, e.g. `+639171234567`.
    e164: String,
    /// Whether this is a mobile or landline number.
    kind: PhoneKind,
}

impl PhoneNumber {
    /// Parses a user-entered number into E.164 form.
    ///
    /// Accepts `+63...`, `63...`, `0...` and bare national forms, with spaces,
    /// dashes, dots or parentheses as separators.
    ///
    /// Philippine national numbers are either ten digits starting with `9`
    /// (mobile) or nine digits (landline, where the first digits are the area
    /// code).
    pub fn parse(input: &str) -> Result<Self, PhoneError> {
        let raw = input.trim();
        if raw.is_empty() {
            return Err(PhoneError::Empty);
        }

        let mut digits = String::with_capacity(raw.len());
        for ch in raw.chars() {
            match ch {
                '0'..='9' => digits.push(ch),
                '+' | ' ' | '-' | '.' | '(' | ')' => {}
                _ => return Err(PhoneError::NotNumeric),
            }
        }

        if digits.is_empty() {
            return Err(PhoneError::Empty);
        }

        // Strip the country code in any of its spellings, or the national
        // trunk prefix.
        let national = if let Some(rest) = digits.strip_prefix("63") {
            rest
        } else if let Some(rest) = digits.strip_prefix('0') {
            rest
        } else {
            digits.as_str()
        };

        let kind = match national.len() {
            10 if national.starts_with('9') => PhoneKind::Mobile,
            9 => PhoneKind::Landline,
            10 => return Err(PhoneError::UnknownPrefix),
            _ => return Err(PhoneError::BadLength),
        };

        Ok(Self {
            e164: format!("+{PH_COUNTRY_CODE}{national}"),
            kind,
        })
    }

    /// The canonical E.164 representation.
    pub fn e164(&self) -> &str {
        &self.e164
    }

    /// The national significant number without the country code.
    pub fn national(&self) -> &str {
        &self.e164[3..]
    }

    /// Whether the number is a mobile number (the only kind Leos can register).
    pub fn is_mobile(&self) -> bool {
        self.kind == PhoneKind::Mobile
    }

    /// The kind of Philippine number this is.
    pub fn kind(&self) -> PhoneKind {
        self.kind
    }

    /// A masked form safe for logs and screenshots, e.g. `+63 917 *** 4567`.
    pub fn masked(&self) -> String {
        let national = self.national();
        let tail = &national[national.len() - 4..];
        format!("+{} {} *** {}", PH_COUNTRY_CODE, &national[..3], tail)
    }
}

impl fmt::Display for PhoneNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.e164)
    }
}

impl TryFrom<String> for PhoneNumber {
    type Error = PhoneError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        PhoneNumber::parse(&value)
    }
}

impl From<PhoneNumber> for String {
    fn from(value: PhoneNumber) -> Self {
        value.e164
    }
}

/// Whether a Philippine number is a mobile or landline number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhoneKind {
    /// A mobile number, starting with 9 after the country code.
    Mobile,
    /// A landline number, whose first digits are the area code.
    Landline,
}

/// Failure modes for [`PhoneNumber::parse`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PhoneError {
    /// The input was empty.
    #[error("phone number is empty")]
    Empty,
    /// The input contained letters or unsupported symbols.
    #[error("phone number contains letters or symbols")]
    NotNumeric,
    /// The number of digits does not match any Philippine number form.
    #[error("phone number is not a valid Philippine number")]
    BadLength,
    /// The number has the right length but an invalid prefix.
    #[error("phone number has an unknown prefix")]
    UnknownPrefix,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn normalizes_every_spelling() {
        let expected = "+639171234567";
        for input in [
            "+639171234567",
            "639171234567",
            "09171234567",
            "9171234567",
            "+63 917 123 4567",
            "0917-123-4567",
            "(0917) 123 4567",
            "  +63.917.123.4567  ",
        ] {
            assert_eq!(
                PhoneNumber::parse(input).unwrap().e164(),
                expected,
                "{input}"
            );
        }
    }

    #[test]
    fn identifies_mobile_and_landline() {
        assert!(PhoneNumber::parse("09171234567").unwrap().is_mobile());
        assert!(!PhoneNumber::parse("0281234567").unwrap().is_mobile());
        assert_eq!(
            PhoneNumber::parse("0281234567").unwrap().kind(),
            PhoneKind::Landline
        );
        assert_eq!(
            PhoneNumber::parse("+63 917 123 4567").unwrap().national(),
            "9171234567"
        );
    }

    #[test]
    fn masks_for_display() {
        assert_eq!(
            PhoneNumber::parse("09171234567").unwrap().masked(),
            "+63 917 *** 4567"
        );
        assert_eq!(
            PhoneNumber::parse("0281234567").unwrap().masked(),
            "+63 281 *** 4567"
        );
    }

    #[test]
    fn rejects_invalid_numbers() {
        assert_eq!(PhoneNumber::parse("").unwrap_err(), PhoneError::Empty);
        assert_eq!(
            PhoneNumber::parse("0917ABC4567").unwrap_err(),
            PhoneError::NotNumeric
        );
        assert_eq!(
            PhoneNumber::parse("091712345").unwrap_err(),
            PhoneError::BadLength
        );
        assert_eq!(
            PhoneNumber::parse("8123456789").unwrap_err(),
            PhoneError::UnknownPrefix
        );
    }
}
