//! The ledger: posting journals, balances and limits.
//!
//! Every unit that moves is written as a journal of postings that balances.
//! A journal is posted inside a database transaction together with the
//! balance updates it causes, and the `account_balances` table carries a
//! `balance >= 0` constraint so the wallet cannot go negative even if a
//! caller forgets to check.

use crate::error::ServerError;
use chrono::{DateTime, Datelike, Duration, Utc};
use sqlx::postgres::PgRow;
use sqlx::{Postgres, Row, Transaction};
use std::collections::{HashMap, HashSet};

use leos_core::domain::{Direction, Journal, Posting, TransactionStatus, TransactionType};
use leos_core::error::LeosError;
use leos_core::money::Money;

/// The Philippines is a fixed eight hours ahead of UTC and does not observe
/// daylight saving, so limits can be computed against Manila days without a
/// timezone database.
const MANILA_OFFSET: Duration = Duration::hours(8);

/// A journal to be posted.
#[derive(Debug, Clone)]
pub struct PostJournal {
    /// The journal identifier.
    pub id: String,
    /// What kind of transaction this is.
    pub tx_type: TransactionType,
    /// Where the transaction stands.
    pub status: TransactionStatus,
    /// The reference shown to both parties.
    pub reference: String,
    /// A human readable note.
    pub memo: Option<String>,
    /// Structured details, kept for reconciliation.
    pub metadata: serde_json::Value,
    /// The postings that make up the journal.
    pub postings: Vec<Posting>,
}

/// The ledger account of a user.
pub fn user_account(user_id: &str) -> String {
    format!("user:{user_id}")
}

/// Works out the kind of a ledger account from its identifier.
fn account_kind(account_id: &str) -> &'static str {
    if account_id.starts_with("user:") {
        "user_wallet"
    } else if account_id.starts_with("cash:agents") {
        "agent_cash"
    } else if account_id.starts_with("cash:bank") {
        "bank_funding"
    } else if account_id.starts_with("fee:") {
        "platform_fee"
    } else if account_id.starts_with("platform:") {
        "equity"
    } else {
        "merchant"
    }
}

/// Creates the ledger account for a user if it does not exist yet.
pub async fn ensure_user_account(
    tx: &mut Transaction<'_, Postgres>,
    user_id: &str,
) -> Result<(), ServerError> {
    let account_id = user_account(user_id);
    sqlx::query(
        "INSERT INTO accounts (id, kind, owner_user_id) VALUES ($1, 'user_wallet', $2)
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(&account_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        "INSERT INTO account_balances (account_id, balance, allow_negative)
         VALUES ($1, 0, FALSE)
         ON CONFLICT (account_id) DO NOTHING",
    )
    .bind(&account_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Posts a balanced journal and updates the balances it touches.
///
/// The whole thing runs inside the caller's transaction so the journal and
/// the balances move together or not at all.
pub async fn post_journal(
    tx: &mut Transaction<'_, Postgres>,
    input: &PostJournal,
) -> Result<(), ServerError> {
    // A journal that does not balance is a programming error, and it must
    // never reach the database.
    let journal = Journal::new(input.postings.clone())?;

    sqlx::query(
        "INSERT INTO journals (id, tx_type, status, reference, memo, metadata)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&input.id)
    .bind(tx_type_label(input.tx_type))
    .bind(tx_status_label(input.status))
    .bind(&input.reference)
    .bind(&input.memo)
    .bind(&input.metadata)
    .execute(&mut **tx)
    .await?;

    // Make sure every account involved exists, and take its balance row.
    let mut account_ids: Vec<String> = journal
        .postings
        .iter()
        .map(|posting| posting.account.clone())
        .collect();
    account_ids.sort();
    account_ids.dedup();

    for account_id in &account_ids {
        sqlx::query("INSERT INTO accounts (id, kind) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING")
            .bind(account_id)
            .bind(account_kind(account_id))
            .execute(&mut **tx)
            .await?;

        sqlx::query(
            "INSERT INTO account_balances (account_id, balance, allow_negative)
             VALUES ($1, 0, $2)
             ON CONFLICT (account_id) DO NOTHING",
        )
        .bind(account_id)
        .bind(account_kind(account_id) != "user_wallet")
        .execute(&mut **tx)
        .await?;
    }

    // Lock the balance rows in a fixed order. Two transfers crossing over
    // each other would otherwise be able to deadlock.
    let locked: Vec<PgRow> = sqlx::query(
        "SELECT account_id, balance, allow_negative FROM account_balances
         WHERE account_id = ANY($1) ORDER BY account_id FOR UPDATE",
    )
    .bind(&account_ids)
    .fetch_all(&mut **tx)
    .await?;

    let mut balances: std::collections::HashMap<String, i64> = HashMap::new();
    let mut protected: std::collections::HashSet<String> = HashSet::new();

    for row in locked {
        let account_id: String = row.get("account_id");
        balances.insert(account_id.clone(), row.get::<i64, _>("balance"));
        if !row.get::<bool, _>("allow_negative") {
            protected.insert(account_id);
        }
    }

    for posting in &journal.postings {
        let signed = match posting.direction {
            Direction::Debit => -posting.amount.centavos(),
            Direction::Credit => posting.amount.centavos(),
        };
        let current = balances.get(&posting.account).copied().unwrap_or(0);
        let updated = current + signed;
        // A customer wallet must never go negative. System accounts are
        // allowed to, because their balances represent money the platform
        // holds rather than money it owes.
        if updated < 0 && protected.contains(&posting.account) {
            return Err(LeosError::InsufficientFunds.into());
        }
        balances.insert(posting.account.clone(), updated);
    }

    for (account_id, balance) in &balances {
        sqlx::query(
            "UPDATE account_balances SET balance = $2, updated_at = now() WHERE account_id = $1",
        )
        .bind(account_id)
        .bind(balance)
        .execute(&mut **tx)
        .await?;
    }

    for posting in &journal.postings {
        sqlx::query(
            "INSERT INTO postings (journal_id, account_id, direction, amount)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(&input.id)
        .bind(&posting.account)
        .bind(direction_label(posting.direction))
        .bind(posting.amount.centavos())
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

/// The balance of an account, in centavos.
pub async fn balance(
    executor: &mut sqlx::PgConnection,
    account_id: &str,
) -> Result<i64, ServerError> {
    let row = sqlx::query("SELECT balance FROM account_balances WHERE account_id = $1")
        .bind(account_id)
        .fetch_optional(executor)
        .await?;

    Ok(row.map(|row| row.get::<i64, _>("balance")).unwrap_or(0))
}

/// The balance of a user's wallet, in centavos.
pub async fn user_balance(
    executor: &mut sqlx::PgConnection,
    user_id: &str,
) -> Result<Money, ServerError> {
    Ok(Money::from_centavos(
        balance(executor, &user_account(user_id)).await?,
    ))
}

/// The total debited from an account since a point in time, in centavos.
///
/// This is what daily and monthly limits are measured against.
pub async fn outgoing_since(
    executor: &mut sqlx::PgConnection,
    account_id: &str,
    since: DateTime<Utc>,
) -> Result<i64, ServerError> {
    let row = sqlx::query(
        "SELECT COALESCE(SUM(p.amount), 0)::bigint AS total
         FROM postings p
         JOIN journals j ON j.id = p.journal_id
         WHERE p.account_id = $1
           AND p.direction = 'debit'
           AND j.status IN ('completed', 'pending')
           AND j.created_at >= $2",
    )
    .bind(account_id)
    .bind(since)
    .fetch_one(&mut *executor)
    .await?;

    Ok(row.get::<i64, _>("total"))
}

/// The start of the current Manila day.
pub fn manila_day_start(now: DateTime<Utc>) -> DateTime<Utc> {
    let local = now + MANILA_OFFSET;
    let day = local.date_naive();
    let start_local = day.and_hms_opt(0, 0, 0).expect("midnight is a valid time");
    start_local.and_utc() - MANILA_OFFSET
}

/// The start of the current Manila calendar month.
pub fn manila_month_start(now: DateTime<Utc>) -> DateTime<Utc> {
    let local = now + MANILA_OFFSET;
    let first = local.date_naive().with_day(1).expect("day 1 exists");
    let start_local = first
        .and_hms_opt(0, 0, 0)
        .expect("midnight is a valid time");
    start_local.and_utc() - MANILA_OFFSET
}

/// The label stored for a transaction type.
pub fn tx_type_label(tx_type: TransactionType) -> &'static str {
    match tx_type {
        TransactionType::Transfer => "transfer",
        TransactionType::CashIn => "cash_in",
        TransactionType::CashOut => "cash_out",
        TransactionType::MerchantPay => "merchant_pay",
        TransactionType::TopUp => "top_up",
        TransactionType::Fee => "fee",
        TransactionType::Reversal => "reversal",
    }
}

/// The label stored for a transaction status.
pub fn tx_status_label(status: TransactionStatus) -> &'static str {
    match status {
        TransactionStatus::Pending => "pending",
        TransactionStatus::Completed => "completed",
        TransactionStatus::Failed => "failed",
        TransactionStatus::Reversed => "reversed",
    }
}

/// The label stored for a posting direction.
fn direction_label(direction: Direction) -> &'static str {
    match direction {
        Direction::Debit => "debit",
        Direction::Credit => "credit",
    }
}

/// A transaction as shown in a wallet's history.
#[derive(Debug, Clone)]
pub struct LedgerEntry {
    /// The transaction identifier.
    pub id: String,
    /// What kind of transaction this is.
    pub tx_type: TransactionType,
    /// Where the transaction stands.
    pub status: TransactionStatus,
    /// The amount moved, always positive.
    pub amount: Money,
    /// The fee charged.
    pub fee: Money,
    /// The reference shown to both parties.
    pub reference: String,
    /// The note attached to the transaction.
    pub memo: Option<String>,
    /// Whether the money came in or went out.
    pub incoming: bool,
    /// The counterparty's phone number, when there is one.
    pub counterparty_phone: Option<String>,
    /// The counterparty's display name, when known.
    pub counterparty_name: Option<String>,
    /// When the transaction happened.
    pub created_at: DateTime<Utc>,
}

/// Loads the transaction history for a user, newest first.
pub async fn list_transactions(
    executor: &mut sqlx::PgConnection,
    user_id: &str,
    limit: i64,
) -> Result<Vec<LedgerEntry>, ServerError> {
    let account_id = user_account(user_id);

    let rows = sqlx::query(
        "SELECT j.id, j.tx_type, j.status, j.reference, j.memo, j.created_at,
                p.amount, p.direction
         FROM postings p
         JOIN journals j ON j.id = p.journal_id
         WHERE p.account_id = $1
         ORDER BY j.created_at DESC, p.id DESC
         LIMIT $2",
    )
    .bind(&account_id)
    .bind(limit)
    .fetch_all(&mut *executor)
    .await?;

    let mut entries = Vec::with_capacity(rows.len());

    for row in rows {
        let id: String = row.get("id");
        let tx_type = parse_tx_type(row.get::<String, _>("tx_type").as_str())?;
        let status = parse_tx_status(row.get::<String, _>("status").as_str())?;
        let reference: String = row.get("reference");
        let memo: Option<String> = row.get("memo");
        let created_at: DateTime<Utc> = row.get("created_at");
        let amount = Money::from_centavos(row.get::<i64, _>("amount"));
        let direction: String = row.get("direction");
        let incoming = direction == "credit";

        let (fee, counterparty_account) =
            counterparty_and_fee(&mut *executor, &id, &account_id).await?;
        let (counterparty_phone, counterparty_name) =
            describe_counterparty(&mut *executor, counterparty_account.as_deref()).await?;

        entries.push(LedgerEntry {
            id,
            tx_type,
            status,
            amount,
            fee,
            reference,
            memo,
            incoming,
            counterparty_phone,
            counterparty_name,
            created_at,
        });
    }

    Ok(entries)
}

/// Finds the fee posting and the counterparty account of a journal.
///
/// The counterparty is whichever account moved against the one being
/// viewed, so a transfer shows the other party and a cash-in shows the
/// agent. Fee postings are accounted for separately.
async fn counterparty_and_fee(
    executor: &mut sqlx::PgConnection,
    journal_id: &str,
    viewer_account: &str,
) -> Result<(Money, Option<String>), ServerError> {
    let rows =
        sqlx::query("SELECT account_id, direction, amount FROM postings WHERE journal_id = $1")
            .bind(journal_id)
            .fetch_all(&mut *executor)
            .await?;

    let mut fee = 0i64;
    let mut counterparty = None;

    for row in rows {
        let account_id: String = row.get("account_id");
        let amount: i64 = row.get("amount");

        if account_id.starts_with("fee:") {
            fee += amount;
        } else if account_id != viewer_account && counterparty.is_none() {
            counterparty = Some(account_id);
        }
    }

    Ok((Money::from_centavos(fee), counterparty))
}

/// Resolves a counterparty account into a name and, for users, a phone number.
async fn describe_counterparty(
    executor: &mut sqlx::PgConnection,
    account_id: Option<&str>,
) -> Result<(Option<String>, Option<String>), ServerError> {
    let Some(account_id) = account_id else {
        return Ok((None, None));
    };

    if let Some(user_id) = account_id.strip_prefix("user:") {
        let row = sqlx::query("SELECT phone, display_name FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(executor)
            .await?;
        return Ok(match row {
            Some(row) => (
                Some(row.get::<String, _>("phone")),
                row.get::<Option<String>, _>("display_name"),
            ),
            None => (None, None),
        });
    }

    if let Some(merchant_id) = account_id.strip_prefix("merchant:") {
        let row = sqlx::query("SELECT name FROM merchants WHERE id = $1")
            .bind(merchant_id)
            .fetch_optional(executor)
            .await?;
        return Ok((None, row.map(|row| row.get::<String, _>("name"))));
    }

    if account_id.starts_with("suspense:cash_out") {
        let row = sqlx::query("SELECT name FROM agents WHERE active = TRUE LIMIT 1")
            .fetch_optional(&mut *executor)
            .await?;
        return Ok((
            None,
            Some(
                row.map(|row| row.get::<String, _>("name"))
                    .unwrap_or_else(|| "Cash agent".to_string()),
            ),
        ));
    }

    if account_id.starts_with("cash:agents") {
        let row = sqlx::query("SELECT name FROM agents WHERE active = TRUE LIMIT 1")
            .fetch_optional(executor)
            .await?;
        return Ok((
            None,
            Some(
                row.map(|row| row.get::<String, _>("name"))
                    .unwrap_or_else(|| "Cash agent".to_string()),
            ),
        ));
    }

    if account_id.starts_with("cash:bank") {
        return Ok((None, Some("Bank transfer".to_string())));
    }

    Ok((None, None))
}

/// Parses a stored transaction type.
pub fn parse_tx_type(label: &str) -> Result<TransactionType, ServerError> {
    Ok(match label {
        "transfer" => TransactionType::Transfer,
        "cash_in" => TransactionType::CashIn,
        "cash_out" => TransactionType::CashOut,
        "merchant_pay" => TransactionType::MerchantPay,
        "top_up" => TransactionType::TopUp,
        "fee" => TransactionType::Fee,
        "reversal" => TransactionType::Reversal,
        other => {
            return Err(LeosError::Internal(format!("unknown transaction type {other}")).into());
        }
    })
}

/// Parses a stored transaction status.
pub fn parse_tx_status(label: &str) -> Result<TransactionStatus, ServerError> {
    Ok(match label {
        "pending" => TransactionStatus::Pending,
        "completed" => TransactionStatus::Completed,
        "failed" => TransactionStatus::Failed,
        "reversed" => TransactionStatus::Reversed,
        other => {
            return Err(LeosError::Internal(format!("unknown transaction status {other}")).into());
        }
    })
}
