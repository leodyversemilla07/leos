//! Demo data: partner agents, merchants and the float they need to operate.

use crate::error::ServerError;
use sqlx::PgPool;

use leos_core::domain::{Direction, Posting, TransactionStatus, TransactionType};
use leos_core::ids::new_id;
use leos_core::money::Money;
use leos_core::pin::derive_secret;
use leos_core::qr::MerchantQr;

use crate::auth::hash_secret;
use crate::ledger::{self, PostJournal};

/// The PIN the demo wallets are set up with.
pub const DEMO_PIN: &str = "4821";

/// The device secret the demo wallets use.
///
/// A real device keeps this in the keystore; the demo needs it to be
/// predictable so the simulator can log in.
pub const DEMO_DEVICE_SECRET: &str = "leos-demo-device-secret-000001";

/// The client salt the demo wallets use.
pub const DEMO_SALT: &str = "leos-demo-salt-0001";

/// A demo wallet.
pub struct DemoWallet {
    /// The mobile number.
    pub phone: &'static str,
    /// The display name.
    pub name: &'static str,
}

/// The demo wallets created by the seeder.
pub const DEMO_WALLETS: &[DemoWallet] = &[
    DemoWallet {
        phone: "+639170000001",
        name: "Ana Reyes",
    },
    DemoWallet {
        phone: "+639170000002",
        name: "Bello Santos",
    },
];

/// Creates the demo agents, merchants and wallets if they are missing.
///
/// Seeding is idempotent: running it against a populated database does
/// nothing.
pub async fn seed(pool: &PgPool) -> Result<(), ServerError> {
    seed_agents(pool).await?;
    seed_merchants(pool).await?;
    seed_float(pool).await?;
    seed_wallets(pool).await?;
    Ok(())
}

/// Creates the partner agents.
async fn seed_agents(pool: &PgPool) -> Result<(), ServerError> {
    let agents = [
        (
            "SMP-001",
            "Sampaguita Cash Point",
            "77 Katipunan Ave, Quezon City",
            15_000_000,
        ),
        (
            "MBH-014",
            "Mabuhay Cash Center",
            "250 Recto Ave, Manila",
            10_000_000,
        ),
        (
            "BYN-007",
            "Bayan Bank Deposit",
            "Ayala Ave, Makati",
            10_000_000,
        ),
    ];

    for (code, name, address, balance) in agents {
        sqlx::query(
            "INSERT INTO agents (id, code, name, address, cash_balance)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (code) DO NOTHING",
        )
        .bind(new_id("agt"))
        .bind(code)
        .bind(name)
        .bind(address)
        .bind(balance)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Creates the merchants that accept QR payments.
async fn seed_merchants(pool: &PgPool) -> Result<(), ServerError> {
    let merchants = [
        ("MERCHANT-000123", "Kasambahay Kitchen", "Mandaluyong"),
        ("MERCHANT-000456", "Brew and Bean", "Makati"),
        ("MERCHANT-000789", "ShopSphere", "Pasig"),
    ];

    for (id, name, city) in merchants {
        let qr = MerchantQr::leos_merchant(id, name, city, None);
        sqlx::query(
            "INSERT INTO merchants (id, name, city, qr_payload)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(name)
        .bind(city)
        .bind(qr.to_payload())
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Gives the system accounts the float they need to operate.
///
/// Agents need cash on hand to pay customers out, and the bank account needs
/// a balance to draw top-ups from. Both come from the platform's capital:
/// the platform raises capital into its bank account, then moves part of it
/// out to the agents who hold cash for customers.
async fn seed_float(pool: &PgPool) -> Result<(), ServerError> {
    let already: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM account_balances WHERE account_id = 'cash:agents'",
    )
    .fetch_one(pool)
    .await?;

    if already > 0 {
        return Ok(());
    }

    let mut tx = pool.begin().await?;

    // The platform raises capital into its own bank account.
    ledger::post_journal(
        &mut tx,
        &PostJournal {
            id: new_id("txn"),
            tx_type: TransactionType::TopUp,
            status: TransactionStatus::Completed,
            reference: format!("SEED-{}", new_id("ref")),
            memo: Some("Opening capital".to_string()),
            metadata: serde_json::json!({ "seed": true }),
            postings: vec![
                Posting {
                    account: "cash:bank".to_string(),
                    direction: Direction::Debit,
                    amount: Money::from_centavos(100_000_000),
                },
                Posting {
                    account: "platform:capital".to_string(),
                    direction: Direction::Credit,
                    amount: Money::from_centavos(100_000_000),
                },
            ],
        },
    )
    .await?;

    // Part of that capital is held as float by the partner agents.
    ledger::post_journal(
        &mut tx,
        &PostJournal {
            id: new_id("txn"),
            tx_type: TransactionType::TopUp,
            status: TransactionStatus::Completed,
            reference: format!("SEED-{}", new_id("ref")),
            memo: Some("Opening float for agents".to_string()),
            metadata: serde_json::json!({ "seed": true }),
            postings: vec![
                Posting {
                    account: "platform:capital".to_string(),
                    direction: Direction::Debit,
                    amount: Money::from_centavos(35_000_000),
                },
                Posting {
                    account: "cash:agents".to_string(),
                    direction: Direction::Credit,
                    amount: Money::from_centavos(35_000_000),
                },
            ],
        },
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Creates the demo wallets with a known PIN.
async fn seed_wallets(pool: &PgPool) -> Result<(), ServerError> {
    let secret = derive_secret(DEMO_PIN, DEMO_DEVICE_SECRET, DEMO_SALT)?;
    let (verifier, salt) = hash_secret(&secret)?;

    for wallet in DEMO_WALLETS {
        let exists: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE phone = $1")
            .bind(wallet.phone)
            .fetch_optional(pool)
            .await?;
        if exists.is_some() {
            continue;
        }

        let user_id = new_id("usr");
        let mut tx = pool.begin().await?;

        sqlx::query(
            "INSERT INTO users (id, phone, display_name, status, pin_verifier, pin_salt)
             VALUES ($1, $2, $3, 'active', $4, $5)",
        )
        .bind(&user_id)
        .bind(wallet.phone)
        .bind(wallet.name)
        .bind(&verifier)
        .bind(&salt)
        .execute(&mut *tx)
        .await?;

        ledger::ensure_user_account(&mut tx, &user_id).await?;
        tx.commit().await?;
    }

    Ok(())
}
