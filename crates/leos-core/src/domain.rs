//! Core domain types: accounts, ledger postings and transactions.

use serde::{Deserialize, Serialize};

use crate::error::{LeosError, Result};
use crate::money::Money;
use crate::phone::PhoneNumber;

/// A ledger account. Balances are derived from postings, never stored raw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerAccount {
    /// The stable account identifier, e.g. `user:usr_...` or `cash:agents`.
    pub id: String,
    /// What the account represents.
    pub kind: AccountKind,
    /// The owning user, for user wallet accounts.
    pub owner: Option<String>,
}

impl LedgerAccount {
    /// The account id of a user's wallet.
    pub fn user_wallet(user_id: &str) -> Self {
        Self {
            id: format!("user:{user_id}"),
            kind: AccountKind::UserWallet,
            owner: Some(user_id.to_string()),
        }
    }

    /// A system account such as `cash:agents` or `fee:cash_out`.
    pub fn system(id: &str, kind: AccountKind) -> Self {
        Self {
            id: id.to_string(),
            kind,
            owner: None,
        }
    }
}

/// The nature of a ledger account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    /// A user's spendable balance.
    UserWallet,
    /// Cash held by partner agents.
    AgentCash,
    /// Cash arriving from a bank transfer.
    BankFunding,
    /// A merchant's settlement balance.
    Merchant,
    /// Fees collected by Leos.
    PlatformFee,
    /// The platform's own capital.
    Equity,
    /// A holding account used while a transfer is in flight.
    Suspense,
}

/// Which way money moved against an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Money leaving the account.
    Debit,
    /// Money entering the account.
    Credit,
}

/// A single movement of money between two accounts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Posting {
    /// The account being moved.
    pub account: String,
    /// Whether money leaves or enters the account.
    pub direction: Direction,
    /// How much moved.
    pub amount: Money,
}

impl Posting {
    /// Money leaving an account.
    pub fn debit(account: impl Into<String>, amount: Money) -> Self {
        Self {
            account: account.into(),
            direction: Direction::Debit,
            amount,
        }
    }

    /// Money entering an account.
    pub fn credit(account: impl Into<String>, amount: Money) -> Self {
        Self {
            account: account.into(),
            direction: Direction::Credit,
            amount,
        }
    }
}

/// A complete, balanced transaction: a list of postings that move money.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    /// The postings making up this journal.
    pub postings: Vec<Posting>,
}

impl Journal {
    /// Builds a journal and checks that it balances.
    ///
    /// Every peso has to come from somewhere, so the sum of debits must equal
    /// the sum of credits. A journal that does not balance is a bug and is
    /// rejected before it can reach the database.
    pub fn new(postings: Vec<Posting>) -> Result<Self> {
        let journal = Self { postings };
        journal.check_balanced()?;
        Ok(journal)
    }

    /// The total value of the debits.
    pub fn total_debits(&self) -> Money {
        self.postings
            .iter()
            .filter(|p| p.direction == Direction::Debit)
            .fold(Money::zero(), |acc, p| acc.saturating_add(p.amount))
    }

    /// The total value of the credits.
    pub fn total_credits(&self) -> Money {
        self.postings
            .iter()
            .filter(|p| p.direction == Direction::Credit)
            .fold(Money::zero(), |acc, p| acc.saturating_add(p.amount))
    }

    /// Whether debits and credits match.
    pub fn is_balanced(&self) -> bool {
        self.total_debits() == self.total_credits()
    }

    /// Returns an error when the journal does not balance.
    pub fn check_balanced(&self) -> Result<()> {
        if self.is_balanced() {
            Ok(())
        } else {
            Err(LeosError::Internal(format!(
                "unbalanced journal: debits {} credits {}",
                self.total_debits(),
                self.total_credits()
            )))
        }
    }
}

/// The kind of movement a transaction represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    /// Money sent to another Leos user.
    Transfer,
    /// Money received from a partner agent.
    CashIn,
    /// Money handed out by a partner agent.
    CashOut,
    /// A payment to a merchant.
    MerchantPay,
    /// A top-up from a bank transfer.
    TopUp,
    /// A fee charged by Leos.
    Fee,
    /// A refund of a previous transaction.
    Reversal,
}

impl TransactionType {
    /// The ledger account kind that absorbs this transaction's value.
    pub fn counterparty_account_kind(&self) -> AccountKind {
        match self {
            TransactionType::Transfer => AccountKind::UserWallet,
            TransactionType::CashIn => AccountKind::AgentCash,
            TransactionType::CashOut => AccountKind::AgentCash,
            TransactionType::MerchantPay => AccountKind::Merchant,
            TransactionType::TopUp => AccountKind::BankFunding,
            TransactionType::Fee | TransactionType::Reversal => AccountKind::PlatformFee,
        }
    }
}

/// Where a transaction stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    /// Accepted but not yet settled.
    Pending,
    /// Settled and posted to the ledger.
    Completed,
    /// Rejected.
    Failed,
    /// Posted then undone.
    Reversed,
}

/// A transaction as shown to a user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionSummary {
    /// The transaction identifier.
    pub id: String,
    /// What kind of transaction this is.
    pub transaction_type: TransactionType,
    /// Where the transaction stands.
    pub status: TransactionStatus,
    /// The amount moved, always positive.
    pub amount: Money,
    /// The fee charged on top of the amount.
    pub fee: Money,
    /// The reference shown to both parties.
    pub reference: String,
    /// The counterparty phone number, when there is one.
    pub counterparty_phone: Option<PhoneNumber>,
    /// The counterparty display name, when known.
    pub counterparty_name: Option<String>,
    /// A human readable note.
    pub note: Option<String>,
    /// When the transaction was created.
    pub created_at: i64,
}

/// A user account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    /// The user identifier.
    pub id: String,
    /// The registered mobile number.
    pub phone: PhoneNumber,
    /// The display name shown to counterparties.
    pub display_name: Option<String>,
    /// When the account was created, as a Unix timestamp.
    pub created_at: i64,
    /// Whether the account is active.
    pub status: UserStatus,
}

/// Whether a user account is usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    /// Created but not yet verified.
    Pending,
    /// Fully verified and able to transact.
    Active,
    /// Frozen by operations.
    Suspended,
    /// Closed by the user.
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn balanced_journal_is_accepted() {
        let journal = Journal::new(vec![
            Posting::debit("user:a", Money::from_centavos(1_000)),
            Posting::credit("user:b", Money::from_centavos(1_000)),
        ])
        .unwrap();
        assert!(journal.is_balanced());
        assert_eq!(journal.total_debits(), Money::from_centavos(1_000));
    }

    #[test]
    fn unbalanced_journal_is_rejected() {
        let error = Journal::new(vec![
            Posting::debit("user:a", Money::from_centavos(1_000)),
            Posting::credit("user:b", Money::from_centavos(900)),
        ])
        .unwrap_err();
        assert!(matches!(error, LeosError::Internal(_)));
    }

    #[test]
    fn user_wallet_account_ids_are_namespaced() {
        assert_eq!(LedgerAccount::user_wallet("usr_123").id, "user:usr_123");
    }
}
