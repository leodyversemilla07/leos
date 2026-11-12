//! Integration tests for the Leos wallet.
//!
//! These run the real API against a real Postgres, over real HTTP. The
//! server is started in process on a random port, so the tests exercise the
//! routes, the middleware, the ledger and the database together.
//!
//! The tests need a database. Point `LEOS_TEST_DATABASE_URL` at one and
//! they will run; without it they are skipped.
//!
//! ```text
//! LEOS_TEST_DATABASE_URL=postgres://leos:leos@localhost:5432/leos \
//!     cargo test -p leos-server
//! ```

use std::sync::Arc;

use leos_core::money::Money;
use leos_core::phone::PhoneNumber;
use reqwest::Client;
use serde_json::{Value, json};
use sqlx::PgPool;

use leos_server::config::Config;
use leos_server::state::AppState;

/// A running test server.
struct TestServer {
    base_url: String,
    pool: PgPool,
    client: Client,
}

/// Starts the API against the test database.
async fn start() -> Option<TestServer> {
    let url = std::env::var("LEOS_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url).await.ok()?;
    sqlx::migrate!("./migrations").run(&pool).await.ok()?;

    let config = Config {
        database_url: url,
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        dev_mode: true,
        otp_ttl: std::time::Duration::from_secs(300),
        session_ttl: std::time::Duration::from_secs(900),
        refresh_ttl: std::time::Duration::from_secs(3600),
        allow_any_origin: true,
    };

    let state = AppState {
        pool: pool.clone(),
        config: Arc::new(config),
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.ok()?;
    let address = listener.local_addr().ok()?;
    let app = leos_server::router(state);

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    Some(TestServer {
        base_url: format!("http://{address}"),
        pool,
        client: Client::new(),
    })
}

/// Nine random digits, for a Philippine mobile number.
fn unique_digits() -> String {
    use rand::Rng;
    (0..9)
        .map(|_| char::from_digit(rand::rng().random_range(0..10), 10).unwrap())
        .collect()
}

/// A phone number that no other test uses.
fn unique_phone() -> String {
    // Ten national digits starting with 9, which is a Philippine mobile
    // number.
    format!("+639{}", unique_digits())
}

/// The secret the client would derive from a PIN.
fn pin_secret(pin: &str) -> String {
    leos_core::pin::derive_secret(pin, "test-device-secret-0123456789abc", "test-salt-0001")
        .expect("the secret can be derived")
}

impl TestServer {
    /// Sends a request and returns the JSON body.
    async fn post(&self, path: &str, body: Value) -> Value {
        self.client
            .post(format!("{}{}", self.base_url, path))
            .json(&body)
            .send()
            .await
            .expect("the request completes")
            .json()
            .await
            .expect("the response is JSON")
    }

    /// Sends a request with a bearer token.
    async fn post_auth(&self, path: &str, body: Value, token: &str) -> Value {
        self.client
            .post(format!("{}{}", self.base_url, path))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .expect("the request completes")
            .json()
            .await
            .expect("the response is JSON")
    }

    /// Sends a GET request with a bearer token.
    async fn get_auth(&self, path: &str, token: &str) -> Value {
        self.client
            .get(format!("{}{}", self.base_url, path))
            .bearer_auth(token)
            .send()
            .await
            .expect("the request completes")
            .json()
            .await
            .expect("the response is JSON")
    }

    /// Registers a wallet and returns its session.
    async fn register(&self, phone: &str, pin: &str) -> Value {
        let challenge = self
            .post("/v1/auth/register/start", json!({ "phone": phone }))
            .await;
        let code = challenge["dev_code"]
            .as_str()
            .expect("the development code is returned")
            .to_string();

        self.post(
            "/v1/auth/register/verify",
            json!({
                "challenge_id": challenge["challenge_id"],
                "code": code,
                "pin_secret": pin_secret(pin),
            }),
        )
        .await
    }

    /// Signs in and returns the session.
    async fn login(&self, phone: &str, pin: &str) -> Value {
        self.post(
            "/v1/auth/login",
            json!({ "phone": phone, "pin_secret": pin_secret(pin) }),
        )
        .await
    }

    /// The balance of a wallet, in centavos.
    async fn balance(&self, token: &str) -> i64 {
        let body = self.get_auth("/v1/wallet/balance", token).await;
        Money::parse(body["balance"].as_str().unwrap_or("0"))
            .expect("the balance parses")
            .centavos()
    }
}

/// The ledger balances: every peso that left an account entered another one.
async fn assert_ledger_balances(pool: &PgPool) {
    let unbalanced: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM (
            SELECT journal_id FROM postings
            GROUP BY journal_id
            HAVING SUM(CASE WHEN direction = 'debit' THEN amount ELSE -amount END) <> 0
         ) AS unbalanced",
    )
    .fetch_one(pool)
    .await
    .expect("the ledger can be read");

    assert_eq!(unbalanced, 0, "every journal must balance");
}

#[tokio::test]
async fn a_customer_can_register_and_see_a_zero_balance() {
    let Some(server) = start().await else {
        eprintln!("skipped: LEOS_TEST_DATABASE_URL is not set");
        return;
    };

    let phone = unique_phone();
    let session = server.register(&phone, "4821").await;

    assert!(session["access_token"].is_string(), "{session}");
    assert_eq!(session["user"]["phone"], phone);

    let token = session["access_token"].as_str().unwrap();
    assert_eq!(server.balance(token).await, 0);
}

#[tokio::test]
async fn a_customer_can_top_up_and_see_the_balance() {
    let Some(server) = start().await else {
        return;
    };

    let phone = unique_phone();
    let session = server.register(&phone, "4821").await;
    let token = session["access_token"].as_str().unwrap().to_string();

    let topup = server
        .post_auth("/v1/topups", json!({ "amount": "1500.00" }), &token)
        .await;
    assert_eq!(topup["status"], "pending");

    let reference = topup["reference"].as_str().unwrap().to_string();
    let confirmed = server
        .client
        .post(format!(
            "{}/v1/partner/bank/topups/{reference}/confirm",
            server.base_url
        ))
        .header("x-leos-partner-key", "dev-partner-key")
        .send()
        .await
        .expect("the confirmation completes")
        .json::<Value>()
        .await
        .expect("the confirmation is JSON");

    assert_eq!(confirmed["status"], "credited");
    assert_eq!(server.balance(&token).await, 150_000);

    assert_ledger_balances(&server.pool).await;
}

#[tokio::test]
async fn a_transfer_moves_money_and_cannot_be_replayed() {
    let Some(server) = start().await else {
        return;
    };

    let sender_phone = unique_phone();
    let receiver_phone = unique_phone();

    let sender = server.register(&sender_phone, "4821").await;
    let receiver = server.register(&receiver_phone, "4821").await;
    let sender_token = sender["access_token"].as_str().unwrap().to_string();
    let receiver_token = receiver["access_token"].as_str().unwrap().to_string();

    // Give the sender some money through the bank.
    let topup = server
        .post_auth("/v1/topups", json!({ "amount": "1000.00" }), &sender_token)
        .await;
    let reference = topup["reference"].as_str().unwrap().to_string();
    server
        .client
        .post(format!(
            "{}/v1/partner/bank/topups/{reference}/confirm",
            server.base_url
        ))
        .header("x-leos-partner-key", "dev-partner-key")
        .send()
        .await
        .expect("the confirmation completes");

    // Send half of it.
    let request = json!({
        "to_phone": receiver_phone,
        "amount": "500.00",
        "note": "kambal",
        "idempotency_key": "test-key-1",
        "pin_secret": pin_secret("4821"),
    });

    let first = server
        .post_auth("/v1/transfers", request.clone(), &sender_token)
        .await;
    assert_eq!(first["status"], "completed");
    assert_eq!(first["amount"], "500.00");
    assert_eq!(first["direction"], "out");

    // The same request again must not move the money twice.
    let second = server
        .post_auth("/v1/transfers", request, &sender_token)
        .await;
    assert_eq!(second["id"], first["id"]);

    assert_eq!(server.balance(&sender_token).await, 50_000);
    assert_eq!(server.balance(&receiver_token).await, 50_000);

    assert_ledger_balances(&server.pool).await;
}

#[tokio::test]
async fn a_transfer_is_refused_when_there_is_not_enough_money() {
    let Some(server) = start().await else {
        return;
    };

    let sender_phone = unique_phone();
    let receiver_phone = unique_phone();

    let sender = server.register(&sender_phone, "4821").await;
    server.register(&receiver_phone, "4821").await;
    let sender_token = sender["access_token"].as_str().unwrap().to_string();

    let response = server
        .post_auth(
            "/v1/transfers",
            json!({
                "to_phone": receiver_phone,
                "amount": "10.00",
                "idempotency_key": "test-key-empty",
                "pin_secret": pin_secret("4821"),
            }),
            &sender_token,
        )
        .await;

    assert_eq!(response["error"]["code"], "insufficient_funds");
    assert_eq!(server.balance(&sender_token).await, 0);
}

#[tokio::test]
async fn a_wrong_pin_is_refused() {
    let Some(server) = start().await else {
        return;
    };

    let phone = unique_phone();
    let session = server.register(&phone, "4821").await;
    let token = session["access_token"].as_str().unwrap().to_string();

    let response = server
        .post_auth(
            "/v1/transfers",
            json!({
                "to_phone": unique_phone(),
                "amount": "10.00",
                "idempotency_key": "test-key-bad-pin",
                "pin_secret": pin_secret("1357"),
            }),
            &token,
        )
        .await;

    assert_eq!(response["error"]["code"], "unauthorized");
}

#[tokio::test]
async fn a_cash_in_voucher_credits_the_wallet_once() {
    let Some(server) = start().await else {
        return;
    };

    let phone = unique_phone();
    let session = server.register(&phone, "4821").await;
    let token = session["access_token"].as_str().unwrap().to_string();

    let voucher = server
        .client
        .post(format!("{}/v1/partner/agents/cash-in", server.base_url))
        .header("x-leos-partner-key", "dev-partner-key")
        .json(&json!({
            "agent_code": "SMP-001",
            "phone": phone,
            "amount": "500.00",
        }))
        .send()
        .await
        .expect("the voucher is issued")
        .json::<Value>()
        .await
        .expect("the voucher is JSON");

    let code = voucher["voucher_code"].as_str().unwrap().to_string();

    let redeemed = server
        .post_auth("/v1/agents/redeem", json!({ "voucher_code": code }), &token)
        .await;
    assert_eq!(redeemed["transaction"]["amount"], "500.00");
    assert_eq!(server.balance(&token).await, 50_000);

    // The same voucher cannot be used twice.
    let again = server
        .post_auth("/v1/agents/redeem", json!({ "voucher_code": code }), &token)
        .await;
    assert_eq!(again["error"]["code"], "conflict");
    assert_eq!(server.balance(&token).await, 50_000);

    assert_ledger_balances(&server.pool).await;
}

#[tokio::test]
async fn a_cash_out_reserves_the_money_and_charges_the_fee() {
    let Some(server) = start().await else {
        return;
    };

    let phone = unique_phone();
    let session = server.register(&phone, "4821").await;
    let token = session["access_token"].as_str().unwrap().to_string();

    // Fund the wallet.
    let topup = server
        .post_auth("/v1/topups", json!({ "amount": "1000.00" }), &token)
        .await;
    let reference = topup["reference"].as_str().unwrap().to_string();
    server
        .client
        .post(format!(
            "{}/v1/partner/bank/topups/{reference}/confirm",
            server.base_url
        ))
        .header("x-leos-partner-key", "dev-partner-key")
        .send()
        .await
        .expect("the confirmation completes");

    let payout = server
        .post_auth(
            "/v1/agents/cash-out",
            json!({
                "amount": "500.00",
                "idempotency_key": "test-key-cash-out",
                "pin_secret": pin_secret("4821"),
            }),
            &token,
        )
        .await;

    assert_eq!(payout["amount"], "485.00");
    assert_eq!(payout["fee"], "15.00");

    // The whole amount left the wallet, fee included.
    assert_eq!(server.balance(&token).await, 50_000);

    // The agent hands the cash over.
    let payout_reference = payout["reference"].as_str().unwrap().to_string();
    let confirmed = server
        .client
        .post(format!(
            "{}/v1/partner/agents/payouts/confirm",
            server.base_url
        ))
        .header("x-leos-partner-key", "dev-partner-key")
        .json(&json!({ "reference": payout_reference, "agent_code": "SMP-001" }))
        .send()
        .await
        .expect("the payout is confirmed")
        .json::<Value>()
        .await
        .expect("the payout is JSON");

    assert_eq!(confirmed["status"], "completed");
    assert_eq!(server.balance(&token).await, 50_000);

    assert_ledger_balances(&server.pool).await;
}

#[tokio::test]
async fn a_merchant_qr_payment_is_matched_to_the_merchant() {
    let Some(server) = start().await else {
        return;
    };

    let phone = unique_phone();
    let session = server.register(&phone, "4821").await;
    let token = session["access_token"].as_str().unwrap().to_string();

    // Fund the wallet so the payment can go through.
    let topup = server
        .post_auth("/v1/topups", json!({ "amount": "500.00" }), &token)
        .await;
    let reference = topup["reference"].as_str().unwrap().to_string();
    server
        .client
        .post(format!(
            "{}/v1/partner/bank/topups/{reference}/confirm",
            server.base_url
        ))
        .header("x-leos-partner-key", "dev-partner-key")
        .send()
        .await
        .expect("the confirmation completes");

    let merchants = server
        .client
        .get(format!("{}/v1/merchants", server.base_url))
        .send()
        .await
        .expect("the merchants load")
        .json::<Value>()
        .await
        .expect("the merchants are JSON");

    let merchant = &merchants["merchants"][0];
    let qr = merchant["qr_payload"].as_str().unwrap();

    // The QR is a real QR Ph payload, with a checksum the server checks.
    assert!(qr.starts_with("0002"));
    assert!(qr.contains("6304"));

    let payment = server
        .post_auth(
            "/v1/merchants/pay",
            json!({
                "qr_payload": qr,
                "amount": "250.00",
                "idempotency_key": "test-key-qr",
                "pin_secret": pin_secret("4821"),
            }),
            &token,
        )
        .await;

    assert_eq!(payment["status"], "completed", "{payment}");
    assert_eq!(payment["counterparty_name"], merchant["name"]);
    assert_eq!(server.balance(&token).await, 25_000);

    // A payment larger than the balance is refused.
    let refused = server
        .post_auth(
            "/v1/merchants/pay",
            json!({
                "qr_payload": qr,
                "amount": "400.00",
                "idempotency_key": "test-key-qr-2",
                "pin_secret": pin_secret("4821"),
            }),
            &token,
        )
        .await;
    assert_eq!(refused["error"]["code"], "insufficient_funds");
}

#[tokio::test]
async fn a_tampered_qr_code_is_refused() {
    let Some(server) = start().await else {
        return;
    };

    let phone = unique_phone();
    let session = server.register(&phone, "4821").await;
    let token = session["access_token"].as_str().unwrap().to_string();

    let merchants = server
        .client
        .get(format!("{}/v1/merchants", server.base_url))
        .send()
        .await
        .expect("the merchants load")
        .json::<Value>()
        .await
        .expect("the merchants are JSON");

    let qr = merchants["merchants"][0]["qr_payload"]
        .as_str()
        .unwrap()
        .to_string();
    let name = merchants["merchants"][0]["name"].as_str().unwrap();

    // Change one character of the merchant name, keeping the length, so the
    // payload stays well formed and only the checksum gives it away.
    let mut tampered_name = name.to_string();
    tampered_name.replace_range(0..1, "X");
    let tampered = qr.replace(name, &tampered_name);
    assert_ne!(tampered, qr, "the payload should have changed");

    let payment = server
        .post_auth(
            "/v1/merchants/pay",
            json!({
                "qr_payload": tampered,
                "amount": "250.00",
                "idempotency_key": "test-key-tampered",
                "pin_secret": pin_secret("4821"),
            }),
            &token,
        )
        .await;

    assert_eq!(payment["error"]["code"], "invalid_qr");
}

#[tokio::test]
async fn the_session_cannot_be_used_after_signing_out() {
    let Some(server) = start().await else {
        return;
    };

    let phone = unique_phone();
    let session = server.register(&phone, "4821").await;
    let token = session["access_token"].as_str().unwrap().to_string();

    assert_eq!(server.balance(&token).await, 0);

    server.post_auth("/v1/auth/logout", json!({}), &token).await;

    let response = server.get_auth("/v1/wallet/balance", &token).await;
    assert_eq!(response["error"]["code"], "unauthorized");
}

#[tokio::test]
async fn a_refresh_token_gives_a_new_session() {
    let Some(server) = start().await else {
        return;
    };

    let phone = unique_phone();
    let session = server.register(&phone, "4821").await;
    let refresh = session["refresh_token"].as_str().unwrap().to_string();

    let refreshed = server
        .post("/v1/auth/refresh", json!({ "refresh_token": refresh }))
        .await;
    assert!(refreshed["access_token"].is_string(), "{refreshed}");

    let token = refreshed["access_token"].as_str().unwrap();
    assert_eq!(server.balance(token).await, 0);
}

#[tokio::test]
async fn a_customer_can_sign_in_again_after_registering() {
    let Some(server) = start().await else {
        return;
    };

    let phone = unique_phone();
    server.register(&phone, "4821").await;

    // The right PIN gives a session.
    let session = server.login(&phone, "4821").await;
    assert!(session["access_token"].is_string(), "{session}");
    let token = session["access_token"].as_str().unwrap();
    assert_eq!(server.balance(token).await, 0);

    // The wrong PIN does not.
    let refused = server.login(&phone, "1357").await;
    assert_eq!(refused["error"]["code"], "unauthorized");
}

#[tokio::test]
async fn a_phone_number_is_normalised_the_same_way_everywhere() {
    let Some(server) = start().await else {
        return;
    };

    // A number typed the way a person would type it is stored in E.164.
    let digits = unique_digits();
    let national = format!("0917{}", &digits[..7]);
    let expected = format!("+63917{}", &digits[..7]);

    let session = server.register(&national, "4821").await;
    assert_eq!(session["user"]["phone"], expected);

    // A landline is not a wallet.
    let refused = server
        .post("/v1/auth/register/start", json!({ "phone": "0281234567" }))
        .await;
    assert_eq!(refused["error"]["code"], "invalid_phone_number");

    // The number is stored in E.164 form.
    let parsed = PhoneNumber::parse(&national).unwrap();
    assert_eq!(parsed.e164(), expected);
}
