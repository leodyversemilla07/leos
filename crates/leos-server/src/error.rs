//! Turning domain errors into HTTP responses.
//!
//! The server owns a small error type of its own. `LeosError` lives in
//! `leos-core`, which has no opinion about HTTP, so the wrapping type is
//! what carries the status code.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use leos_core::error::LeosError;

/// The error type every handler returns.
#[derive(Debug)]
pub struct ServerError(LeosError);

impl ServerError {
    /// Unwraps the underlying domain error.
    pub fn into_inner(self) -> LeosError {
        self.0
    }
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ServerError {}

impl From<LeosError> for ServerError {
    fn from(error: LeosError) -> Self {
        Self(error)
    }
}

impl From<sqlx::Error> for ServerError {
    fn from(error: sqlx::Error) -> Self {
        // Constraint violations are the wallet's own guard rails firing, not
        // bugs, so they get a meaningful message instead of a 500.
        if let sqlx::Error::Database(db) = &error {
            if let Some(constraint) = db.constraint() {
                return Self(match constraint {
                    "account_balances_balance_check" => LeosError::InsufficientFunds,
                    other => LeosError::Conflict(format!("database constraint {other} violated")),
                });
            }
        }
        tracing::error!(error = %error, "database error");
        Self(LeosError::Internal("database error".to_string()))
    }
}

impl From<sqlx::migrate::MigrateError> for ServerError {
    fn from(error: sqlx::migrate::MigrateError) -> Self {
        tracing::error!(error = %error, "migration error");
        Self(LeosError::Internal("migration failed".to_string()))
    }
}

/// The body returned for any failed request.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    /// The failing part of the request.
    pub error: ErrorDetail,
}

/// The detail of a failure.
#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    /// A stable, machine readable code.
    pub code: &'static str,
    /// A human readable message.
    pub message: String,
    /// Whether the client should try again.
    pub retryable: bool,
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.0.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = ErrorBody {
            error: ErrorDetail {
                code: self.0.code(),
                message: self.0.to_string(),
                retryable: self.0.is_retryable(),
            },
        };
        (status, Json(body)).into_response()
    }
}

/// A result carrying a [`ServerError`].
pub type ServerResult<T> = Result<T, ServerError>;
