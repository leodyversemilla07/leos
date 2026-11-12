//! Shared application state.

use crate::error::ServerError;
use sqlx::Row;
use std::sync::Arc;

use sqlx::PgPool;

use leos_core::error::LeosError;
use leos_core::phone::PhoneNumber;

use crate::auth::{AuthUser, hash_token};
use crate::config::Config;

/// Everything a request handler needs.
#[derive(Clone)]
pub struct AppState {
    /// The database pool.
    pub pool: PgPool,
    /// The server configuration.
    pub config: Arc<Config>,
}

impl AppState {
    /// Resolves a bearer token into the caller it belongs to.
    pub async fn authenticate(&self, token: &str) -> Result<AuthUser, ServerError> {
        let session_id = hash_token(token);
        let now = chrono::Utc::now();

        let row = sqlx::query(
            "SELECT s.id AS session_id, s.user_id, s.expires_at, s.revoked_at,
                    u.phone, u.status
             FROM sessions s
             JOIN users u ON u.id = s.user_id
             WHERE s.id = $1",
        )
        .bind(&session_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Err(LeosError::Unauthorized("session not found".to_string()).into());
        };

        let revoked_at: Option<chrono::DateTime<chrono::Utc>> = row.get("revoked_at");
        if revoked_at.is_some() {
            return Err(LeosError::Unauthorized("session has been revoked".to_string()).into());
        }

        let expires_at: chrono::DateTime<chrono::Utc> = row.get("expires_at");
        if expires_at <= now {
            return Err(LeosError::Unauthorized("session has expired".to_string()).into());
        }

        let status: String = row.get("status");
        if status != "active" {
            return Err(LeosError::Forbidden(format!("account is {status}")).into());
        }

        let phone_raw: String = row.get("phone");
        let phone = PhoneNumber::parse(&phone_raw)
            .map_err(|e| LeosError::Internal(format!("stored phone number is invalid: {e}")))?;

        Ok(AuthUser {
            user_id: row.get("user_id"),
            phone,
            session_id: row.get("session_id"),
        })
    }
}
