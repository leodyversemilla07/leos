//! Cash in and cash out through partner agents.
//!
//! Cash in works the way it does in the Philippines: the customer pays an
//! agent in cash and receives a voucher code, which they then redeem in the
//! app. Cash out is the mirror image: the app reserves the money and hands
//! the customer a reference to collect at an agent.

use crate::error::ServerError;
use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use leos_core::domain::{Direction, Posting, TransactionStatus, TransactionType};
use leos_core::error::LeosError;
use leos_core::ids::{new_id, new_reference, random_code};
use leos_core::money::Money;
use leos_core::policy::WalletPolicy;

use crate::auth::AuthUser;
use crate::idempotency::{self, Claim};
use crate::ledger::{self, PostJournal};
use crate::routes::auth::verify_pin;
use crate::routes::wallet::TransactionDto;
use crate::state::AppState;

/// The endpoint identifier used for idempotency keys.
const ENDPOINT: &str = "cash_out";

/// A partner agent.
#[derive(Debug, Serialize)]
pub struct AgentDto {
    /// The agent identifier.
    pub id: String,
    /// The code the agent is known by.
    pub code: String,
    /// The trading name.
    pub name: String,
    /// Where the agent is.
    pub address: String,
    /// Whether the agent is accepting transactions.
    pub active: bool,
}

/// The list of agents.
#[derive(Debug, Serialize)]
pub struct AgentList {
    /// The agents.
    pub agents: Vec<AgentDto>,
}

/// A request to withdraw cash.
#[derive(Debug, Deserialize)]
pub struct CashOutRequest {
    /// The amount to withdraw, as a decimal string.
    pub amount: String,
    /// A key that makes a retried request safe.
    pub idempotency_key: String,
    /// The PIN secret derived on the device.
    pub pin_secret: String,
}

/// The result of a cash out request.
#[derive(Debug, Serialize, Deserialize)]
pub struct CashOutResponse {
    /// The payout identifier.
    pub id: String,
    /// The reference to show the agent.
    pub reference: String,
    /// The amount that will be handed over.
    pub amount: String,
    /// The fee charged.
    pub fee: String,
    /// Where to collect the money.
    pub instructions: String,
    /// When the request was made.
    pub created_at: DateTime<Utc>,
}

/// A request to redeem a cash-in voucher.
#[derive(Debug, Deserialize)]
pub struct RedeemRequest {
    /// The voucher code the agent gave the customer.
    pub voucher_code: String,
}

/// The result of redeeming a voucher.
#[derive(Debug, Serialize, Deserialize)]
pub struct RedeemResponse {
    /// The transaction that credited the wallet.
    pub transaction: TransactionDto,
    /// The amount credited.
    pub amount: String,
}

/// `GET /v1/agents`
pub async fn list_agents(State(state): State<AppState>) -> Result<Json<AgentList>, ServerError> {
    let rows = sqlx::query(
        "SELECT id, code, name, address, active FROM agents WHERE active = TRUE ORDER BY name",
    )
    .fetch_all(&state.pool)
    .await?;

    let agents = rows
        .into_iter()
        .map(|row| AgentDto {
            id: row.get("id"),
            code: row.get("code"),
            name: row.get("name"),
            address: row.get("address"),
            active: row.get("active"),
        })
        .collect();

    Ok(Json(AgentList { agents }))
}

/// `POST /v1/agents/cash-out`
pub async fn cash_out(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<CashOutRequest>,
) -> Result<Json<CashOutResponse>, ServerError> {
    idempotency::validate_key(&request.idempotency_key)?;

    let mut connection = state.pool.acquire().await?;
    match idempotency::claim(
        &mut connection,
        &user.user_id,
        ENDPOINT,
        &request.idempotency_key,
    )
    .await?
    {
        Claim::Replay(response) => {
            return serde_json::from_value(response).map(Json).map_err(|e| {
                LeosError::Internal(format!("stored response is unusable: {e}")).into()
            });
        }
        Claim::Fresh => {}
    }

    verify_pin(&state, &user.user_id, &request.pin_secret).await?;

    let amount =
        Money::parse(&request.amount).map_err(|e| LeosError::InvalidAmount(e.to_string()))?;
    if !amount.is_positive() {
        return Err(
            LeosError::InvalidAmount("the amount must be greater than zero".to_string()).into(),
        );
    }

    let policy = WalletPolicy::default();
    if amount > policy.max_cash_out {
        return Err(LeosError::LimitExceeded(format!(
            "a single cash out cannot be more than {}",
            policy.max_cash_out.to_display_string()
        ))
        .into());
    }

    let fee = policy.cash_out_fee;
    let (net, fee) = amount.split_fee(fee).ok_or_else(|| {
        LeosError::InvalidAmount("the amount is too small for the fee".to_string())
    })?;

    let payout_id = new_id("pay");
    let reference = new_reference();
    let user_account = ledger::user_account(&user.user_id);

    let postings = vec![
        Posting {
            account: user_account.clone(),
            direction: Direction::Debit,
            amount,
        },
        Posting {
            account: "suspense:cash_out".to_string(),
            direction: Direction::Credit,
            amount: net,
        },
        Posting {
            account: "fee:cash_out".to_string(),
            direction: Direction::Credit,
            amount: fee,
        },
    ];

    let mut tx = state.pool.begin().await?;
    ledger::post_journal(
        &mut tx,
        &PostJournal {
            id: payout_id.clone(),
            tx_type: TransactionType::CashOut,
            status: TransactionStatus::Pending,
            reference: reference.clone(),
            memo: Some("Cash out".to_string()),
            metadata: serde_json::json!({ "fee": fee.centavos() }),
            postings,
        },
    )
    .await?;

    sqlx::query(
        "INSERT INTO payouts (id, user_id, reference, amount, fee, status)
         VALUES ($1, $2, $3, $4, $5, 'pending')",
    )
    .bind(&payout_id)
    .bind(&user.user_id)
    .bind(&reference)
    .bind(amount.centavos())
    .bind(fee.centavos())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let response = CashOutResponse {
        id: payout_id,
        reference: reference.clone(),
        amount: net.to_string(),
        fee: fee.to_string(),
        instructions: format!(
            "Present reference {reference} at any Leos partner agent to collect {}.",
            net.to_display_string()
        ),
        created_at: Utc::now(),
    };

    let payload = serde_json::to_value(&response)
        .map_err(|e| LeosError::Internal(format!("could not encode response: {e}")))?;
    idempotency::store(
        &mut connection,
        &user.user_id,
        ENDPOINT,
        &request.idempotency_key,
        &payload,
    )
    .await?;

    Ok(Json(response))
}

/// `POST /v1/agents/redeem`
pub async fn redeem_voucher(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<RedeemRequest>,
) -> Result<Json<RedeemResponse>, ServerError> {
    let code = request.voucher_code.trim().to_uppercase();
    if code.is_empty() {
        return Err(LeosError::InvalidRequest("the voucher code is empty".to_string()).into());
    }

    let mut tx = state.pool.begin().await?;

    let row = sqlx::query(
        "SELECT id, agent_id, amount, status FROM vouchers WHERE reference = $1 FOR UPDATE",
    )
    .bind(&code)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        return Err(LeosError::NotFound("that voucher code was not found".to_string()).into());
    };

    let status: String = row.get("status");
    if status != "issued" {
        return Err(LeosError::Conflict("that voucher has already been used".to_string()).into());
    }

    let amount = Money::from_centavos(row.get::<i64, _>("amount"));
    let voucher_id: String = row.get("id");

    let journal_id = new_id("txn");
    let reference = new_reference();

    ledger::post_journal(
        &mut tx,
        &PostJournal {
            id: journal_id.clone(),
            tx_type: TransactionType::CashIn,
            status: TransactionStatus::Completed,
            reference: reference.clone(),
            memo: Some(format!("Cash in via agent {code}")),
            metadata: serde_json::json!({ "voucher": voucher_id }),
            postings: vec![
                Posting {
                    account: "cash:agents".to_string(),
                    direction: Direction::Debit,
                    amount,
                },
                Posting {
                    account: ledger::user_account(&user.user_id),
                    direction: Direction::Credit,
                    amount,
                },
            ],
        },
    )
    .await?;

    sqlx::query(
        "UPDATE vouchers SET status = 'redeemed', redeemed_at = now(), user_id = $2 WHERE id = $1",
    )
    .bind(&voucher_id)
    .bind(&user.user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Json(RedeemResponse {
        transaction: TransactionDto {
            id: journal_id,
            transaction_type: "cash_in".to_string(),
            status: "completed".to_string(),
            amount: amount.to_string(),
            fee: Money::zero().to_string(),
            reference,
            direction: "in".to_string(),
            counterparty_phone: None,
            counterparty_name: Some("Cash agent".to_string()),
            note: Some(format!("Cash in via agent {code}")),
            created_at: Utc::now(),
        },
        amount: amount.to_string(),
    }))
}

/// Generates the voucher code an agent terminal uses.
pub fn new_voucher_code() -> String {
    random_code(8)
}
