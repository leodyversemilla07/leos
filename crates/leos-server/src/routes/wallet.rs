//! The wallet: profile, balance and transaction history.

use crate::error::ServerError;
use axum::Json;
use axum::extract::{Query, State};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::AuthUser;
use crate::ledger;
use crate::routes::auth::UserDto;
use crate::state::AppState;

/// The currency every Leos wallet holds.
///
/// Leos Credits, which exist only inside this project.
pub const CURRENCY: &str = leos_core::currency::LEOS_CREDITS.code;

/// The profile and balance of the signed in user.
#[derive(Debug, Serialize)]
pub struct MeResponse {
    /// The signed in user.
    pub user: UserDto,
    /// The available balance, as a decimal string.
    pub balance: String,
    /// The balance currency.
    pub currency: &'static str,
}

/// The available balance.
#[derive(Debug, Serialize)]
pub struct BalanceResponse {
    /// The available balance, as a decimal string.
    pub balance: String,
    /// The balance currency.
    pub currency: &'static str,
}

/// A transaction as shown in the app.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionDto {
    /// The transaction identifier.
    pub id: String,
    /// The kind of transaction.
    pub transaction_type: String,
    /// Where the transaction stands.
    pub status: String,
    /// The amount moved, as a decimal string.
    pub amount: String,
    /// The fee charged, as a decimal string.
    pub fee: String,
    /// The reference shown to both parties.
    pub reference: String,
    /// `in` when money arrived, `out` when it left.
    pub direction: String,
    /// The counterparty's phone number.
    pub counterparty_phone: Option<String>,
    /// The counterparty's display name.
    pub counterparty_name: Option<String>,
    /// The note attached to the transaction.
    pub note: Option<String>,
    /// When the transaction happened.
    pub created_at: DateTime<Utc>,
}

/// The transaction history.
#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    /// The transactions, newest first.
    pub transactions: Vec<TransactionDto>,
}

/// Query parameters for the history.
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    /// How many transactions to return, at most 100.
    pub limit: Option<i64>,
}

/// `GET /v1/me`
pub async fn me(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<MeResponse>, ServerError> {
    let profile = crate::routes::auth::load_user(&state, &user.user_id).await?;
    let mut connection = state.pool.acquire().await?;
    let balance = ledger::user_balance(&mut connection, &user.user_id).await?;

    Ok(Json(MeResponse {
        user: profile,
        balance: balance.to_string(),
        currency: CURRENCY,
    }))
}

/// `GET /v1/wallet/balance`
pub async fn balance(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<BalanceResponse>, ServerError> {
    let mut connection = state.pool.acquire().await?;
    let balance = ledger::user_balance(&mut connection, &user.user_id).await?;
    Ok(Json(BalanceResponse {
        balance: balance.to_string(),
        currency: CURRENCY,
    }))
}

/// `GET /v1/transactions`
pub async fn history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
    user: AuthUser,
) -> Result<Json<HistoryResponse>, ServerError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);

    let mut connection = state.pool.acquire().await?;
    let entries = ledger::list_transactions(&mut connection, &user.user_id, limit).await?;

    let transactions = entries
        .into_iter()
        .map(|entry| TransactionDto {
            id: entry.id,
            transaction_type: ledger::tx_type_label(entry.tx_type).to_string(),
            status: ledger::tx_status_label(entry.status).to_string(),
            amount: entry.amount.to_string(),
            fee: entry.fee.to_string(),
            reference: entry.reference,
            direction: if entry.incoming { "in" } else { "out" }.to_string(),
            counterparty_phone: entry.counterparty_phone,
            counterparty_name: entry.counterparty_name,
            note: entry.memo,
            created_at: entry.created_at,
        })
        .collect();

    Ok(Json(HistoryResponse { transactions }))
}
