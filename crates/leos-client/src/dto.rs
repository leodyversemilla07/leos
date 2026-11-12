//! The shapes the Leos API speaks in.

use serde::{Deserialize, Serialize};

use leos_core::money::Money;

/// A response from the challenge endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeResponse {
    /// The challenge identifier.
    pub challenge_id: String,
    /// When the challenge expires.
    pub expires_at: String,
    /// The passcode, only present in development builds.
    #[serde(default)]
    pub dev_code: Option<String>,
}

/// A user as the API reports it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    /// The user identifier.
    pub id: String,
    /// The mobile number in E.164 form.
    pub phone: String,
    /// The display name shown to counterparties.
    #[serde(default)]
    pub display_name: Option<String>,
    /// The account status.
    pub status: String,
}

/// A session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResponse {
    /// The bearer token for the API.
    pub access_token: String,
    /// The token used to obtain a new access token.
    pub refresh_token: String,
    /// How long the access token is valid for, in seconds.
    pub expires_in: i64,
    /// The signed in user.
    pub user: UserProfile,
}

/// The signed in user and their balance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeResponse {
    /// The signed in user.
    pub user: UserProfile,
    /// The available balance.
    pub balance: Money,
    /// The balance currency.
    pub currency: String,
}

/// The available balance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceResponse {
    /// The available balance.
    pub balance: Money,
    /// The balance currency.
    pub currency: String,
}

/// A transaction as shown in the app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// The transaction identifier.
    pub id: String,
    /// The kind of transaction.
    pub transaction_type: String,
    /// Where the transaction stands.
    pub status: String,
    /// The amount moved.
    pub amount: Money,
    /// The fee charged.
    pub fee: Money,
    /// The reference shown to both parties.
    pub reference: String,
    /// `in` when money arrived, `out` when it left.
    pub direction: String,
    /// The counterparty's phone number.
    #[serde(default)]
    pub counterparty_phone: Option<String>,
    /// The counterparty's display name.
    #[serde(default)]
    pub counterparty_name: Option<String>,
    /// The note attached to the transaction.
    #[serde(default)]
    pub note: Option<String>,
    /// When the transaction happened.
    pub created_at: String,
}

/// The transaction history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryResponse {
    /// The transactions, newest first.
    pub transactions: Vec<Transaction>,
}

/// The result of a cash out request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashOutResponse {
    /// The payout identifier.
    pub id: String,
    /// The reference to show the agent.
    pub reference: String,
    /// The amount that will be handed over.
    pub amount: Money,
    /// The fee charged.
    pub fee: Money,
    /// Where to collect the money.
    pub instructions: String,
    /// When the request was made.
    pub created_at: String,
}

/// The result of redeeming a cash-in voucher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedeemResponse {
    /// The transaction that credited the wallet.
    pub transaction: Transaction,
    /// The amount credited.
    pub amount: Money,
}

/// A pending top-up.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topup {
    /// The top-up identifier.
    pub id: String,
    /// The reference to quote to the bank.
    pub reference: String,
    /// The amount requested.
    pub amount: Money,
    /// Where the top-up stands.
    pub status: String,
    /// The bank to transfer from.
    pub bank_code: String,
    /// The account to transfer to.
    pub bank_account: String,
    /// When the reference expires.
    pub expires_at: String,
    /// When the top-up was created.
    pub created_at: String,
}

/// The list of a user's top-ups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopupList {
    /// The top-ups, newest first.
    pub topups: Vec<Topup>,
}

/// A partner agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentList {
    /// The agents.
    pub agents: Vec<Agent>,
}

/// A merchant that accepts Leos payments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Merchant {
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantList {
    /// The merchants.
    pub merchants: Vec<Merchant>,
}

/// The error body the API returns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    /// The failing part of the request.
    pub error: ErrorDetail,
}

/// The detail of a failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    /// A stable, machine readable code.
    pub code: String,
    /// A human readable message.
    pub message: String,
    /// Whether the client should try again.
    #[serde(default)]
    pub retryable: bool,
}
