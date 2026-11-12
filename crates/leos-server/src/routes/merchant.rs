//! QR pay: paying a merchant by scanning their QR code.

use crate::error::ServerError;
use axum::Json;
use axum::extract::State;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use leos_core::domain::{Direction, Posting, TransactionStatus, TransactionType};
use leos_core::error::LeosError;
use leos_core::ids::{new_id, new_reference};
use leos_core::money::Money;
use leos_core::policy::WalletPolicy;
use leos_core::qr::MerchantQr;

use crate::auth::AuthUser;
use crate::idempotency::{self, Claim};
use crate::ledger::{self, PostJournal};
use crate::routes::auth::verify_pin;
use crate::routes::wallet::TransactionDto;
use crate::state::AppState;

/// The endpoint identifier used for idempotency keys.
const ENDPOINT: &str = "merchant_pay";

/// A merchant that accepts Leos payments.
#[derive(Debug, Serialize)]
pub struct MerchantDto {
    /// The merchant identifier.
    pub id: String,
    /// The trading name.
    pub name: String,
    /// Where the merchant is.
    pub city: String,
    /// The QR payload the merchant prints.
    pub qr_payload: String,
}

/// The list of merchants.
#[derive(Debug, Serialize)]
pub struct MerchantList {
    /// The merchants.
    pub merchants: Vec<MerchantDto>,
}

/// A request to pay a merchant.
#[derive(Debug, Deserialize)]
pub struct PayRequest {
    /// The scanned QR payload.
    pub qr_payload: String,
    /// The amount, when the QR does not carry one.
    pub amount: Option<String>,
    /// A key that makes a retried request safe.
    pub idempotency_key: String,
    /// The PIN secret derived on the device.
    pub pin_secret: String,
}

/// `GET /v1/merchants`
pub async fn list_merchants(
    State(state): State<AppState>,
) -> Result<Json<MerchantList>, ServerError> {
    let rows = sqlx::query(
        "SELECT id, name, city, qr_payload FROM merchants WHERE active = TRUE ORDER BY name",
    )
    .fetch_all(&state.pool)
    .await?;

    let merchants = rows
        .into_iter()
        .map(|row| MerchantDto {
            id: row.get("id"),
            name: row.get("name"),
            city: row.get("city"),
            qr_payload: row.get("qr_payload"),
        })
        .collect();

    Ok(Json(MerchantList { merchants }))
}

/// `POST /v1/merchants/pay`
pub async fn pay(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<PayRequest>,
) -> Result<Json<TransactionDto>, ServerError> {
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

    let qr = MerchantQr::from_payload(&request.qr_payload)
        .map_err(|e| LeosError::InvalidQr(e.to_string()))?;

    if !qr.is_supported_currency() {
        return Err(LeosError::InvalidQr("that QR code is not in Leos Credits".to_string()).into());
    }

    // The amount comes from the QR when the merchant fixed one, and from the
    // customer otherwise.
    let amount = match (qr.amount, request.amount.as_deref()) {
        (Some(fixed), Some(requested)) => {
            let requested =
                Money::parse(requested).map_err(|e| LeosError::InvalidAmount(e.to_string()))?;
            if requested != fixed {
                return Err(LeosError::InvalidAmount(
                    "the amount does not match the QR code".to_string(),
                )
                .into());
            }
            fixed
        }
        (Some(fixed), None) => fixed,
        (None, Some(requested)) => {
            Money::parse(requested).map_err(|e| LeosError::InvalidAmount(e.to_string()))?
        }
        (None, None) => {
            return Err(LeosError::InvalidAmount(
                "the QR code does not carry an amount".to_string(),
            )
            .into());
        }
    };

    if !amount.is_positive() {
        return Err(
            LeosError::InvalidAmount("the amount must be greater than zero".to_string()).into(),
        );
    }

    let policy = WalletPolicy::default();
    if amount > policy.max_transfer {
        return Err(LeosError::LimitExceeded(format!(
            "a single payment cannot be more than {}",
            policy.max_transfer.to_display_string()
        ))
        .into());
    }

    // Find the merchant. The QR may carry the merchant id directly, or the
    // payload may match one a merchant registered.
    let merchant = find_merchant(&state, &qr, &request.qr_payload).await?;

    let fee = policy.merchant_pay_fee;
    let (net, fee) = amount
        .split_fee(fee)
        .ok_or_else(|| LeosError::Internal("the fee exceeds the amount".to_string()))?;

    let journal_id = new_id("txn");
    let reference = new_reference();

    let mut postings = vec![
        Posting {
            account: ledger::user_account(&user.user_id),
            direction: Direction::Debit,
            amount,
        },
        Posting {
            account: format!("merchant:{}", merchant.0),
            direction: Direction::Credit,
            amount: net,
        },
    ];
    if !fee.is_zero() {
        postings.push(Posting {
            account: "fee:merchant".to_string(),
            direction: Direction::Credit,
            amount: fee,
        });
    }

    let mut tx = state.pool.begin().await?;
    ledger::post_journal(
        &mut tx,
        &PostJournal {
            id: journal_id.clone(),
            tx_type: TransactionType::MerchantPay,
            status: TransactionStatus::Completed,
            reference: reference.clone(),
            memo: qr.merchant_name.clone(),
            metadata: serde_json::json!({
                "merchant_id": merchant.0,
                "merchant_name": merchant.1,
                "fee": fee.centavos(),
            }),
            postings,
        },
    )
    .await?;
    tx.commit().await?;

    let response = TransactionDto {
        id: journal_id,
        transaction_type: "merchant_pay".to_string(),
        status: "completed".to_string(),
        amount: amount.to_string(),
        fee: fee.to_string(),
        reference,
        direction: "out".to_string(),
        counterparty_phone: None,
        counterparty_name: Some(merchant.1),
        note: qr.merchant_name,
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

/// Resolves a scanned QR to a registered merchant.
///
/// Returns the merchant identifier and name.
async fn find_merchant(
    state: &AppState,
    qr: &MerchantQr,
    raw_payload: &str,
) -> Result<(String, String), ServerError> {
    if let Some(merchant_id) = qr.merchant_id() {
        let row = sqlx::query("SELECT id, name FROM merchants WHERE id = $1 AND active = TRUE")
            .bind(merchant_id)
            .fetch_optional(&state.pool)
            .await?;
        if let Some(row) = row {
            return Ok((row.get("id"), row.get("name")));
        }
    }

    let row = sqlx::query("SELECT id, name FROM merchants WHERE qr_payload = $1 AND active = TRUE")
        .bind(raw_payload.trim())
        .fetch_optional(&state.pool)
        .await?;

    row.map(|row| (row.get("id"), row.get("name")))
        .ok_or_else(|| {
            LeosError::NotFound("that merchant does not accept Leos payments".to_string()).into()
        })
}
