//! Idempotency: making a retried request safe.
//!
//! Mobile networks retry. If a transfer request is sent twice because the
//! first response was lost, the wallet must not move the money twice. The
//! first request to claim a key stores its response; later requests with the
//! same key replay that response instead of doing the work again.

use crate::error::ServerError;
use sqlx::Row;
use sqlx::{PgConnection, Postgres, Transaction};

use leos_core::error::LeosError;

/// The outcome of trying to claim an idempotency key.
pub enum Claim {
    /// The caller owns the key and should carry on.
    Fresh,
    /// The key was already used and the stored response should be replayed.
    Replay(serde_json::Value),
}

/// Claims a key for a request.
///
/// Returns [`Claim::Replay`] when the key has already been used, in which
/// case the caller must return the stored response untouched.
pub async fn claim(
    executor: &mut PgConnection,
    user_id: &str,
    endpoint: &str,
    key: &str,
) -> Result<Claim, ServerError> {
    let inserted = sqlx::query(
        "INSERT INTO idempotency_keys (key, user_id, endpoint) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, endpoint, key) DO NOTHING",
    )
    .bind(key)
    .bind(user_id)
    .bind(endpoint)
    .execute(&mut *executor)
    .await?;

    if inserted.rows_affected() == 1 {
        return Ok(Claim::Fresh);
    }

    let row = sqlx::query(
        "SELECT response FROM idempotency_keys WHERE user_id = $1 AND endpoint = $2 AND key = $3",
    )
    .bind(user_id)
    .bind(endpoint)
    .bind(key)
    .fetch_optional(&mut *executor)
    .await?;

    match row.and_then(|row| row.get::<Option<serde_json::Value>, _>("response")) {
        Some(response) => Ok(Claim::Replay(response)),
        // The first request is still running.
        None => {
            Err(LeosError::Conflict("an identical request is still in flight".to_string()).into())
        }
    }
}

/// Stores the response for a claimed key.
pub async fn store(
    executor: &mut PgConnection,
    user_id: &str,
    endpoint: &str,
    key: &str,
    response: &serde_json::Value,
) -> Result<(), ServerError> {
    sqlx::query(
        "UPDATE idempotency_keys SET response = $4
         WHERE user_id = $1 AND endpoint = $2 AND key = $3",
    )
    .bind(user_id)
    .bind(endpoint)
    .bind(key)
    .bind(response)
    .execute(&mut *executor)
    .await?;
    Ok(())
}

/// Stores the response for a claimed key inside an existing transaction.
pub async fn store_in_tx(
    executor: &mut Transaction<'_, Postgres>,
    user_id: &str,
    endpoint: &str,
    key: &str,
    response: &serde_json::Value,
) -> Result<(), ServerError> {
    sqlx::query(
        "UPDATE idempotency_keys SET response = $4
         WHERE user_id = $1 AND endpoint = $2 AND key = $3",
    )
    .bind(user_id)
    .bind(endpoint)
    .bind(key)
    .bind(response)
    .execute(&mut **executor)
    .await?;
    Ok(())
}

/// Checks that an idempotency key is present and sane.
pub fn validate_key(key: &str) -> Result<(), ServerError> {
    if key.trim().is_empty() {
        return Err(LeosError::InvalidRequest("an idempotency key is required".to_string()).into());
    }
    if key.len() > 128 {
        return Err(
            LeosError::InvalidRequest("the idempotency key is too long".to_string()).into(),
        );
    }
    Ok(())
}
