//! The error type shared by every Leos component.
//!
//! Errors carry a stable machine readable code so the mobile client can react
//! to them without string matching, and an HTTP status so the server can map
//! them without a lookup table.

use serde::{Deserialize, Serialize};

/// A Leos error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "code", content = "message", rename_all = "snake_case")]
pub enum LeosError {
    /// The request was malformed.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// The amount was not a valid peso value.
    #[error("invalid amount: {0}")]
    InvalidAmount(String),
    /// The phone number was not a valid Philippine mobile number.
    #[error("invalid phone number: {0}")]
    InvalidPhoneNumber(String),
    /// The PIN did not satisfy the policy.
    #[error("invalid PIN: {0}")]
    InvalidPin(String),
    /// The one-time passcode did not match.
    #[error("invalid or expired code")]
    InvalidOtp,
    /// The QR payload could not be decoded.
    #[error("invalid QR code: {0}")]
    InvalidQr(String),
    /// Credentials were missing or wrong.
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    /// The account is not allowed to do this.
    #[error("forbidden: {0}")]
    Forbidden(String),
    /// The wallet has too little money.
    #[error("not enough balance")]
    InsufficientFunds,
    /// A transaction limit was hit.
    #[error("limit exceeded: {0}")]
    LimitExceeded(String),
    /// The account is temporarily locked after too many failed attempts.
    #[error("account locked: {0}")]
    Locked(String),
    /// Too many requests in a short window.
    #[error("too many requests: {0}")]
    RateLimited(String),
    /// The referenced object does not exist.
    #[error("not found: {0}")]
    NotFound(String),
    /// The object already exists or the request conflicts with current state.
    #[error("conflict: {0}")]
    Conflict(String),
    /// The provider behind an external rail failed.
    #[error("payment rail unavailable: {0}")]
    RailUnavailable(String),
    /// Something went wrong inside Leos.
    #[error("internal error: {0}")]
    Internal(String),
}

impl LeosError {
    /// A stable, machine readable code.
    pub fn code(&self) -> &'static str {
        match self {
            LeosError::InvalidRequest(_) => "invalid_request",
            LeosError::InvalidAmount(_) => "invalid_amount",
            LeosError::InvalidPhoneNumber(_) => "invalid_phone_number",
            LeosError::InvalidPin(_) => "invalid_pin",
            LeosError::InvalidOtp => "invalid_otp",
            LeosError::InvalidQr(_) => "invalid_qr",
            LeosError::Unauthorized(_) => "unauthorized",
            LeosError::Forbidden(_) => "forbidden",
            LeosError::InsufficientFunds => "insufficient_funds",
            LeosError::LimitExceeded(_) => "limit_exceeded",
            LeosError::Locked(_) => "locked",
            LeosError::RateLimited(_) => "rate_limited",
            LeosError::NotFound(_) => "not_found",
            LeosError::Conflict(_) => "conflict",
            LeosError::RailUnavailable(_) => "rail_unavailable",
            LeosError::Internal(_) => "internal",
        }
    }

    /// The HTTP status this error maps to.
    pub fn http_status(&self) -> u16 {
        match self {
            LeosError::InvalidRequest(_)
            | LeosError::InvalidAmount(_)
            | LeosError::InvalidPhoneNumber(_)
            | LeosError::InvalidPin(_)
            | LeosError::InvalidQr(_) => 400,
            LeosError::Unauthorized(_) => 401,
            LeosError::Forbidden(_) => 403,
            LeosError::NotFound(_) => 404,
            LeosError::Conflict(_) => 409,
            LeosError::InvalidOtp => 401,
            LeosError::InsufficientFunds | LeosError::LimitExceeded(_) | LeosError::Locked(_) => {
                422
            }
            LeosError::RateLimited(_) => 429,
            LeosError::RailUnavailable(_) => 503,
            LeosError::Internal(_) => 500,
        }
    }

    /// Whether retrying the same request could succeed.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            LeosError::RailUnavailable(_) | LeosError::RateLimited(_) | LeosError::Internal(_)
        )
    }
}

/// Result alias used across Leos.
pub type Result<T, E = LeosError> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique() {
        let all = [
            LeosError::InvalidRequest(String::new()),
            LeosError::InvalidAmount(String::new()),
            LeosError::InvalidPhoneNumber(String::new()),
            LeosError::InvalidPin(String::new()),
            LeosError::InvalidOtp,
            LeosError::InvalidQr(String::new()),
            LeosError::Unauthorized(String::new()),
            LeosError::Forbidden(String::new()),
            LeosError::InsufficientFunds,
            LeosError::LimitExceeded(String::new()),
            LeosError::Locked(String::new()),
            LeosError::RateLimited(String::new()),
            LeosError::NotFound(String::new()),
            LeosError::Conflict(String::new()),
            LeosError::RailUnavailable(String::new()),
            LeosError::Internal(String::new()),
        ];
        let mut codes: Vec<_> = all.iter().map(|e| e.code()).collect();
        codes.sort_unstable();
        let count = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), count, "error codes must be unique");
    }

    #[test]
    fn serializes_with_a_code_tag() {
        let json = serde_json::to_value(LeosError::InsufficientFunds).unwrap();
        assert_eq!(json["code"], "insufficient_funds");
    }
}
