//! Person to person transfers between Leos wallets.

use crate::error::ServerError;
use axum::Json;
use axum::extract::State;
use chrono::Utc;
use serde::Deserialize;

use leos_core::domain::{Direction, Posting, TransactionStatus, TransactionType};
use leos_core::error::LeosError;
use leos_core::ids::{new_id, new_reference};
use leos_core::money::Money;
use leos_core::phone::PhoneNumber;
use leos_core::policy::WalletPolicy;

use crate::auth::AuthUser;
use crate::idempotency::{self, Claim};
use crate::ledger::{self, PostJournal};
use crate::routes::auth::verify_pin;
use crate::routes::wallet::TransactionDto;
use crate::state::AppState;

/// The endpoint identifier used for idempotency keys.
const ENDPOINT: &str = "transfer";

/// A request to send money to another wallet.
#[derive(Debug, Deserialize)]
pub struct TransferRequest {
    /// The recipient's mobile number.
    pub to_phone: String,
    /// The amount to send, as a decimal string.
    pub amount: String,
    /// An optional note for the recipient.
    pub note: Option<String>,
    /// A key that makes a retried request safe.
    pub idempotency_key: String,
    /// The PIN secret derived on the device.
    pub pin_secret: String,
}

/// `POST /v1/transfers`
pub async fn transfer(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<TransferRequest>,
) -> Result<Json<TransactionDto>, ServerError> {
    idempotency::validate_key(&request.idempotency_key)?;

    // Claim the key before doing any work so a retry cannot double spend.
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

    let recipient_phone = PhoneNumber::parse(&request.to_phone)
        .map_err(|e| LeosError::InvalidPhoneNumber(e.to_string()))?;
    if !recipient_phone.is_mobile() {
        return Err(LeosError::InvalidPhoneNumber(
            "the recipient's number is not a mobile number".to_string(),
        )
        .into());
    }
    if recipient_phone == user.phone {
        return Err(
            LeosError::InvalidRequest("you cannot send money to yourself".to_string()).into(),
        );
    }

    let amount =
        Money::parse(&request.amount).map_err(|e| LeosError::InvalidAmount(e.to_string()))?;
    if !amount.is_positive() {
        return Err(
            LeosError::InvalidAmount("the amount must be greater than zero".to_string()).into(),
        );
    }

    let policy = WalletPolicy::default();
    if amount > policy.max_transfer {
        return Err(LeosError::LimitExceeded(format!(
            "a single transfer cannot be more than {}",
            policy.max_transfer.to_display_string()
        ))
        .into());
    }

    let recipient_id: String =
        sqlx::query_scalar("SELECT id FROM users WHERE phone = $1 AND status = 'active'")
            .bind(recipient_phone.e164())
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| {
                LeosError::NotFound("the recipient does not have a Leos wallet".to_string())
            })?;

    let recipient_name: Option<String> =
        sqlx::query_scalar("SELECT display_name FROM users WHERE id = $1")
            .bind(&recipient_id)
            .fetch_optional(&state.pool)
            .await?
            .flatten();

    let sender_account = ledger::user_account(&user.user_id);
    let now = Utc::now();

    let daily = ledger::outgoing_since(
        &mut connection,
        &sender_account,
        ledger::manila_day_start(now),
    )
    .await?;
    if daily + amount.centavos() > policy.max_daily_transfer.centavos() {
        return Err(LeosError::LimitExceeded(format!(
            "the daily transfer limit of {} has been reached",
            policy.max_daily_transfer.to_display_string()
        ))
        .into());
    }

    let monthly = ledger::outgoing_since(
        &mut connection,
        &sender_account,
        ledger::manila_month_start(now),
    )
    .await?;
    if monthly + amount.centavos() > policy.max_monthly_transfer.centavos() {
        return Err(LeosError::LimitExceeded(format!(
            "the monthly transfer limit of {} has been reached",
            policy.max_monthly_transfer.to_display_string()
        ))
        .into());
    }

    let fee = policy.transfer_fee;
    let (net, fee) = amount
        .split_fee(fee)
        .ok_or_else(|| LeosError::Internal("the fee exceeds the amount".to_string()))?;

    let reference = new_reference();
    let journal_id = new_id("txn");

    let mut postings = vec![
        Posting {
            account: sender_account.clone(),
            direction: Direction::Debit,
            amount,
        },
        Posting {
            account: ledger::user_account(&recipient_id),
            direction: Direction::Credit,
            amount: net,
        },
    ];
    if !fee.is_zero() {
        postings.push(Posting {
            account: "fee:transfer".to_string(),
            direction: Direction::Credit,
            amount: fee,
        });
    }

    let mut tx = state.pool.begin().await?;
    ledger::post_journal(
        &mut tx,
        &PostJournal {
            id: journal_id.clone(),
            tx_type: TransactionType::Transfer,
            status: TransactionStatus::Completed,
            reference: reference.clone(),
            memo: request.note.clone(),
            metadata: serde_json::json!({
                "from": user.user_id,
                "to": recipient_id,
                "to_phone": recipient_phone.e164(),
                "fee": fee.centavos(),
            }),
            postings,
        },
    )
    .await?;
    tx.commit().await?;

    let response = TransactionDto {
        id: journal_id,
        transaction_type: "transfer".to_string(),
        status: "completed".to_string(),
        amount: amount.to_string(),
        fee: fee.to_string(),
        reference,
        direction: "out".to_string(),
        counterparty_phone: Some(recipient_phone.e164().to_string()),
        counterparty_name: recipient_name,
        note: request.note,
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
