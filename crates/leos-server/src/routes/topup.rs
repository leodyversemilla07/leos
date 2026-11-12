//! Top-ups from a bank transfer.
//!
//! A customer asks their bank to transfer money to Leos and quotes the
//! reference Leos gives them. The bank confirms the transfer, and the wallet
//! is credited. Until then the money is in flight and the wallet is
//! unchanged.

use crate::error::ServerError;
use axum::Json;
use axum::extract::{Path, State};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use leos_core::domain::{Direction, Posting, TransactionStatus, TransactionType};
use leos_core::error::LeosError;
use leos_core::ids::{new_id, new_reference};
use leos_core::money::Money;
use leos_core::policy::WalletPolicy;

use crate::auth::AuthUser;
use crate::ledger::{self, PostJournal};
use crate::state::AppState;

/// The bank account customers are told to transfer to.
pub const BANK_CODE: &str = "LEOS-001";

/// The account number customers are told to transfer to.
pub const BANK_ACCOUNT: &str = "00001234567890";

/// How long a top-up reference stays open.
const TOPUP_TTL: Duration = Duration::hours(24);

/// A request to start a top-up.
#[derive(Debug, Deserialize)]
pub struct TopupRequest {
    /// The amount to top up, as a decimal string.
    pub amount: String,
}

/// A pending top-up.
#[derive(Debug, Serialize)]
pub struct TopupDto {
    /// The top-up identifier.
    pub id: String,
    /// The reference to quote to the bank.
    pub reference: String,
    /// The amount requested.
    pub amount: String,
    /// Where the top-up stands.
    pub status: String,
    /// The bank to transfer from.
    pub bank_code: String,
    /// The account to transfer to.
    pub bank_account: String,
    /// When the reference expires.
    pub expires_at: DateTime<Utc>,
    /// When the top-up was created.
    pub created_at: DateTime<Utc>,
}

/// The list of a user's top-ups.
#[derive(Debug, Serialize)]
pub struct TopupList {
    /// The top-ups, newest first.
    pub topups: Vec<TopupDto>,
}

/// `POST /v1/topups`
pub async fn create_topup(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<TopupRequest>,
) -> Result<Json<TopupDto>, ServerError> {
    let amount =
        Money::parse(&request.amount).map_err(|e| LeosError::InvalidAmount(e.to_string()))?;
    if !amount.is_positive() {
        return Err(
            LeosError::InvalidAmount("the amount must be greater than zero".to_string()).into(),
        );
    }

    let policy = WalletPolicy::default();
    if amount > policy.max_top_up {
        return Err(LeosError::LimitExceeded(format!(
            "a single top up cannot be more than {}",
            policy.max_top_up.to_display_string()
        ))
        .into());
    }

    let id = new_id("top");
    let reference = new_reference();
    let expires_at = Utc::now() + TOPUP_TTL;

    sqlx::query(
        "INSERT INTO topups (id, user_id, reference, amount, status, bank_code, expires_at)
         VALUES ($1, $2, $3, $4, 'pending', $5, $6)",
    )
    .bind(&id)
    .bind(&user.user_id)
    .bind(&reference)
    .bind(amount.centavos())
    .bind(BANK_CODE)
    .bind(expires_at)
    .execute(&state.pool)
    .await?;

    Ok(Json(TopupDto {
        id,
        reference: reference.clone(),
        amount: amount.to_string(),
        status: "pending".to_string(),
        bank_code: BANK_CODE.to_string(),
        bank_account: BANK_ACCOUNT.to_string(),
        expires_at,
        created_at: Utc::now(),
    }))
}

/// `GET /v1/topups`
pub async fn list_topups(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<TopupList>, ServerError> {
    let rows = sqlx::query(
        "SELECT id, reference, amount, status, bank_code, expires_at, created_at
         FROM topups WHERE user_id = $1 ORDER BY created_at DESC LIMIT 50",
    )
    .bind(&user.user_id)
    .fetch_all(&state.pool)
    .await?;

    let topups = rows
        .into_iter()
        .map(|row| TopupDto {
            id: row.get("id"),
            reference: row.get("reference"),
            amount: Money::from_centavos(row.get::<i64, _>("amount")).to_string(),
            status: row.get("status"),
            bank_code: row.get("bank_code"),
            bank_account: BANK_ACCOUNT.to_string(),
            expires_at: row.get("expires_at"),
            created_at: row.get("created_at"),
        })
        .collect();

    Ok(Json(TopupList { topups }))
}

/// `POST /v1/topups/{reference}/confirm`
///
/// Called by the bank once the transfer has settled.
pub async fn confirm_topup(
    State(state): State<AppState>,
    Path(reference): Path<String>,
) -> Result<Json<TopupDto>, ServerError> {
    let mut tx = state.pool.begin().await?;

    let row = sqlx::query(
        "SELECT id, user_id, amount, status, bank_code, expires_at, created_at
         FROM topups WHERE reference = $1 FOR UPDATE",
    )
    .bind(&reference)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        return Err(LeosError::NotFound("no top up with that reference".to_string()).into());
    };

    let status: String = row.get("status");
    if status == "credited" {
        // Confirming twice is not an error: the money is only credited once.
        return Ok(Json(TopupDto {
            id: row.get("id"),
            reference: reference.clone(),
            amount: Money::from_centavos(row.get::<i64, _>("amount")).to_string(),
            status,
            bank_code: row.get("bank_code"),
            bank_account: BANK_ACCOUNT.to_string(),
            expires_at: row.get("expires_at"),
            created_at: row.get("created_at"),
        }));
    }
    if status != "pending" {
        return Err(LeosError::Conflict(format!("the top up is {status}")).into());
    }

    let expires_at: DateTime<Utc> = row.get("expires_at");
    if expires_at <= Utc::now() {
        return Err(LeosError::Conflict("the top up reference has expired".to_string()).into());
    }

    let user_id: String = row.get("user_id");
    let amount = Money::from_centavos(row.get::<i64, _>("amount"));
    let journal_id = new_id("txn");
    let journal_reference = new_reference();

    ledger::post_journal(
        &mut tx,
        &PostJournal {
            id: journal_id,
            tx_type: TransactionType::TopUp,
            status: TransactionStatus::Completed,
            reference: journal_reference,
            memo: Some(format!("Top up {reference}")),
            metadata: serde_json::json!({ "topup": row.get::<String, _>("id") }),
            postings: vec![
                Posting {
                    account: "cash:bank".to_string(),
                    direction: Direction::Debit,
                    amount,
                },
                Posting {
                    account: ledger::user_account(&user_id),
                    direction: Direction::Credit,
                    amount,
                },
            ],
        },
    )
    .await?;

    sqlx::query("UPDATE topups SET status = 'credited', credited_at = now() WHERE id = $1")
        .bind(row.get::<String, _>("id"))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(Json(TopupDto {
        id: row.get("id"),
        reference,
        amount: amount.to_string(),
        status: "credited".to_string(),
        bank_code: row.get("bank_code"),
        bank_account: BANK_ACCOUNT.to_string(),
        expires_at,
        created_at: row.get("created_at"),
    }))
}
