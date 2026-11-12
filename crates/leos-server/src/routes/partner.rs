//! Partner endpoints: what an agent terminal or a bank can do.
//!
//! These are not part of the consumer app. They are the machine facing side
//! of the payment rails, and they are guarded by a partner key rather than a
//! user session.

use crate::error::ServerError;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use leos_core::domain::{Direction, Posting, TransactionStatus, TransactionType};
use leos_core::error::LeosError;
use leos_core::ids::new_id;
use leos_core::money::Money;
use leos_core::phone::PhoneNumber;

use crate::ledger::{self, PostJournal};
use crate::state::AppState;

/// The header a partner presents.
pub const PARTNER_KEY_HEADER: &str = "x-leos-partner-key";

/// A request from an agent terminal to issue a cash-in voucher.
#[derive(Debug, Deserialize)]
pub struct CashInRequest {
    /// The agent's code.
    pub agent_code: String,
    /// The customer's mobile number.
    pub phone: String,
    /// The amount of cash taken, as a decimal string.
    pub amount: String,
}

/// A newly issued voucher.
#[derive(Debug, Serialize)]
pub struct VoucherResponse {
    /// The code the customer enters in the app.
    pub voucher_code: String,
    /// The amount credited.
    pub amount: String,
    /// The agent that issued it.
    pub agent_name: String,
    /// When it was issued.
    pub created_at: chrono::DateTime<Utc>,
}

/// A request to confirm that cash was handed over.
#[derive(Debug, Deserialize)]
pub struct ConfirmPayoutRequest {
    /// The reference the customer presented.
    pub reference: String,
    /// The agent's code.
    pub agent_code: String,
}

/// The outcome of confirming a payout.
#[derive(Debug, Serialize)]
pub struct PayoutResponse {
    /// The payout identifier.
    pub id: String,
    /// The reference.
    pub reference: String,
    /// Where the payout stands.
    pub status: String,
    /// The amount handed over.
    pub amount: String,
}

/// A payout waiting to be collected.
#[derive(Debug, Serialize)]
pub struct PendingPayout {
    /// The reference the customer presents.
    pub reference: String,
    /// The amount to hand over.
    pub amount: String,
    /// The customer's mobile number.
    pub customer_phone: String,
}

/// Checks the partner key.
pub fn check_partner_key(headers: &HeaderMap) -> Result<(), ServerError> {
    let expected = std::env::var("LEOS_PARTNER_KEY").unwrap_or_else(|_| "dev-partner-key".into());
    let provided = headers
        .get(PARTNER_KEY_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();

    if provided != expected {
        return Err(
            LeosError::Unauthorized("the partner key is missing or wrong".to_string()).into(),
        );
    }
    Ok(())
}

/// `POST /v1/partner/agents/cash-in`
pub async fn issue_cash_in(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CashInRequest>,
) -> Result<Json<VoucherResponse>, ServerError> {
    check_partner_key(&headers)?;

    let phone = PhoneNumber::parse(&request.phone)
        .map_err(|e| LeosError::InvalidPhoneNumber(e.to_string()))?;

    let amount =
        Money::parse(&request.amount).map_err(|e| LeosError::InvalidAmount(e.to_string()))?;
    if !amount.is_positive() {
        return Err(
            LeosError::InvalidAmount("the amount must be greater than zero".to_string()).into(),
        );
    }

    let mut tx = state.pool.begin().await?;

    let agent = sqlx::query(
        "SELECT id, name, cash_balance FROM agents WHERE code = $1 AND active = TRUE FOR UPDATE",
    )
    .bind(request.agent_code.trim().to_uppercase())
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| LeosError::NotFound("unknown agent code".to_string()))?;

    let agent_id: String = agent.get("id");
    let agent_name: String = agent.get("name");
    let cash_balance: i64 = agent.get("cash_balance");

    if cash_balance < amount.centavos() {
        return Err(LeosError::Conflict("the agent does not have enough cash".to_string()).into());
    }

    sqlx::query("UPDATE agents SET cash_balance = cash_balance - $2 WHERE id = $1")
        .bind(&agent_id)
        .bind(amount.centavos())
        .execute(&mut *tx)
        .await?;

    let voucher_code = crate::routes::cash::new_voucher_code();
    sqlx::query(
        "INSERT INTO vouchers (id, agent_id, reference, amount, status)
         VALUES ($1, $2, $3, $4, 'issued')",
    )
    .bind(new_id("vch"))
    .bind(&agent_id)
    .bind(&voucher_code)
    .bind(amount.centavos())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    tracing::info!(
        agent = %agent_name,
        phone = %phone.masked(),
        amount = %amount.to_display_string(),
        "cash-in voucher issued"
    );

    Ok(Json(VoucherResponse {
        voucher_code,
        amount: amount.to_string(),
        agent_name,
        created_at: Utc::now(),
    }))
}

/// `GET /v1/partner/agents/payouts`
pub async fn list_pending_payouts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PendingPayout>>, ServerError> {
    check_partner_key(&headers)?;

    let rows = sqlx::query(
        "SELECT p.reference, p.amount, u.phone
         FROM payouts p
         JOIN users u ON u.id = p.user_id
         WHERE p.status = 'pending'
         ORDER BY p.created_at ASC",
    )
    .fetch_all(&state.pool)
    .await?;

    let payouts = rows
        .into_iter()
        .map(|row| PendingPayout {
            reference: row.get("reference"),
            amount: Money::from_centavos(row.get::<i64, _>("amount")).to_string(),
            customer_phone: row.get("phone"),
        })
        .collect();

    Ok(Json(payouts))
}

/// `POST /v1/partner/agents/payouts/confirm`
pub async fn confirm_payout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ConfirmPayoutRequest>,
) -> Result<Json<PayoutResponse>, ServerError> {
    check_partner_key(&headers)?;

    let mut tx = state.pool.begin().await?;

    let row = sqlx::query(
        "SELECT id, user_id, amount, fee, status FROM payouts WHERE reference = $1 FOR UPDATE",
    )
    .bind(request.reference.trim())
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        return Err(LeosError::NotFound("no payout with that reference".to_string()).into());
    };

    let payout_id: String = row.get("id");
    let amount: i64 = row.get("amount");
    let fee: i64 = row.get("fee");
    let status: String = row.get("status");

    if status == "completed" {
        return Ok(Json(PayoutResponse {
            id: payout_id,
            reference: request.reference.trim().to_string(),
            status,
            amount: Money::from_centavos(amount - fee).to_string(),
        }));
    }

    let agent = sqlx::query(
        "SELECT id, name, cash_balance FROM agents WHERE code = $1 AND active = TRUE FOR UPDATE",
    )
    .bind(request.agent_code.trim().to_uppercase())
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| LeosError::NotFound("unknown agent code".to_string()))?;

    let agent_id: String = agent.get("id");
    let agent_name: String = agent.get("name");
    let cash_balance: i64 = agent.get("cash_balance");

    let net = amount - fee;
    if cash_balance < net {
        return Err(LeosError::Conflict("the agent does not have enough cash".to_string()).into());
    }

    // The money moves out of suspense and into the agent's pooled cash.
    ledger::post_journal(
        &mut tx,
        &PostJournal {
            id: new_id("txn"),
            tx_type: TransactionType::CashOut,
            status: TransactionStatus::Completed,
            reference: format!(
                "PAYOUT-{}",
                &request.reference[..request.reference.len().min(24)]
            ),
            memo: Some(format!("Cash out to {agent_name}")),
            metadata: serde_json::json!({ "payout": payout_id }),
            postings: vec![
                Posting {
                    account: "suspense:cash_out".to_string(),
                    direction: Direction::Debit,
                    amount: Money::from_centavos(net),
                },
                Posting {
                    account: "cash:agents".to_string(),
                    direction: Direction::Credit,
                    amount: Money::from_centavos(net),
                },
            ],
        },
    )
    .await?;

    sqlx::query(
        "UPDATE payouts SET status = 'completed', completed_at = now(), agent_id = $2 WHERE id = $1",
    )
    .bind(&payout_id)
    .bind(&agent_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE agents SET cash_balance = cash_balance + $2 WHERE id = $1")
        .bind(&agent_id)
        .bind(net)
        .execute(&mut *tx)
        .await?;

    // The original reservation journal is now settled.
    sqlx::query("UPDATE journals SET status = 'completed' WHERE reference = $1")
        .bind(request.reference.trim())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(Json(PayoutResponse {
        id: payout_id,
        reference: request.reference.trim().to_string(),
        status: "completed".to_string(),
        amount: Money::from_centavos(net).to_string(),
    }))
}

/// `POST /v1/partner/bank/topups/{reference}/confirm`
pub async fn confirm_bank_topup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reference): Path<String>,
) -> Result<Json<crate::routes::topup::TopupDto>, ServerError> {
    check_partner_key(&headers)?;
    crate::routes::topup::confirm_topup(State(state), Path(reference)).await
}
