//! The Leos partner tools.
//!
//! These stand in for the parties outside the wallet: a cash agent with a
//! terminal on the counter, and the bank a customer transfers from. They
//! make the whole payment flow runnable without real money moving, which is
//! what makes the wallet testable end to end.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use reqwest::Client;
use reqwest::header::HeaderMap;
use serde_json::{Value, json};

use leos_core::money::Money;
use leos_core::phone::PhoneNumber;

/// The tools.
#[derive(Debug, Parser)]
#[command(name = "leos-cli", version, about = "Leos partner tools")]
struct Cli {
    /// The address of the Leos API.
    #[arg(long, env = "LEOS_API_URL", default_value = "http://127.0.0.1:8080")]
    api: String,

    /// The key the partners present to the API.
    #[arg(long, env = "LEOS_PARTNER_KEY", default_value = "dev-partner-key")]
    partner_key: String,

    /// What to do.
    #[command(subcommand)]
    command: Command,
}

/// The things the tools can do.
#[derive(Debug, Subcommand)]
enum Command {
    /// The cash agent terminal.
    Agent {
        /// What the agent should do.
        #[command(subcommand)]
        command: AgentCommand,
    },

    /// The bank that customers transfer from.
    Bank {
        /// What the bank should do.
        #[command(subcommand)]
        command: BankCommand,
    },

    /// A customer using the wallet from a terminal.
    Wallet {
        /// What the customer should do.
        #[command(subcommand)]
        command: WalletCommand,
    },

    /// Runs the whole flow end to end, which is the quickest way to see the
    /// wallet working.
    Demo,
}

/// The agent's actions.
#[derive(Debug, Subcommand)]
enum AgentCommand {
    /// Issues a cash-in voucher for a customer who paid cash.
    CashIn {
        /// The customer's mobile number.
        #[arg(long)]
        phone: String,

        /// The amount of cash taken.
        #[arg(long)]
        amount: String,

        /// The agent's code.
        #[arg(long, default_value = "SMP-001")]
        code: String,
    },

    /// Lists the payouts waiting to be collected.
    Payouts,

    /// Hands cash over to a customer who asked for it.
    Pay {
        /// The reference the customer presented.
        #[arg(long)]
        reference: String,

        /// The agent's code.
        #[arg(long, default_value = "SMP-001")]
        code: String,
    },

    /// Lists the agents that are registered.
    List,
}

/// The bank's actions.
#[derive(Debug, Subcommand)]
enum BankCommand {
    /// Confirms a transfer that has settled, which credits the wallet.
    Confirm {
        /// The reference the customer quoted.
        #[arg(long)]
        reference: String,

        /// The amount that was transferred.
        #[arg(long)]
        amount: String,
    },
}

/// The customer's actions.
#[derive(Debug, Subcommand)]
enum WalletCommand {
    /// Signs in and shows the balance.
    Balance {
        /// The mobile number.
        #[arg(long, default_value = "+639170000001")]
        phone: String,

        /// The PIN.
        #[arg(long, default_value = "4821")]
        pin: String,
    },

    /// Sends money to another wallet.
    Transfer {
        /// The recipient's mobile number.
        #[arg(long)]
        to: String,

        /// The amount to send.
        #[arg(long)]
        amount: String,

        /// The PIN.
        #[arg(long, default_value = "4821")]
        pin: String,
    },
}

/// The main entry point.
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let Cli {
        api,
        partner_key,
        command,
    } = cli;

    match command {
        Command::Agent { command } => agent(&api, &partner_key, command).await,
        Command::Bank { command } => bank(&api, &partner_key, command).await,
        Command::Wallet { command } => wallet(&api, &partner_key, command).await,
        Command::Demo => demo(&api, &partner_key).await,
    }
}

/// The agent terminal.
async fn agent(api: &str, partner_key: &str, command: AgentCommand) -> Result<()> {
    let client = Client::new();

    match command {
        AgentCommand::CashIn {
            phone,
            amount,
            code,
        } => {
            let phone = PhoneNumber::parse(&phone)
                .with_context(|| format!("{phone} is not a Philippine number"))?;
            let amount = Money::parse(&amount)?;

            let response = client
                .post(format!("{}/v1/partner/agents/cash-in", api))
                .headers(partner_headers(partner_key))
                .json(&json!({
                    "agent_code": code,
                    "phone": phone.e164(),
                    "amount": amount.to_string(),
                }))
                .send()
                .await?
                .json::<Value>()
                .await?;

            println!("Cash in accepted");
            println!("  Amount      {}", amount.to_display_string());
            println!("  Customer    {}", phone.masked());
            println!(
                "  Voucher code {}",
                response["voucher_code"].as_str().unwrap_or("?")
            );
            println!();
            println!("Give the customer the voucher code. They enter it in the app.");
        }

        AgentCommand::Payouts => {
            let response = client
                .get(format!("{}/v1/partner/agents/payouts", api))
                .headers(partner_headers(partner_key))
                .send()
                .await?
                .json::<Value>()
                .await?;

            let payouts = response.as_array().cloned().unwrap_or_default();
            if payouts.is_empty() {
                println!("No payouts are waiting to be collected.");
            } else {
                println!("Payouts waiting to be collected:");
                for payout in payouts {
                    println!(
                        "  {}  {:>12}  {}",
                        payout["reference"].as_str().unwrap_or("?"),
                        format_peso(payout["amount"].as_str().unwrap_or("0")),
                        payout["customer_phone"].as_str().unwrap_or("?"),
                    );
                }
            }
        }

        AgentCommand::Pay { reference, code } => {
            let response = client
                .post(format!("{}/v1/partner/agents/payouts/confirm", api))
                .headers(partner_headers(partner_key))
                .json(&json!({ "reference": reference, "agent_code": code }))
                .send()
                .await?
                .json::<Value>()
                .await?;

            println!(
                "Paid {} to the customer.",
                format_peso(response["amount"].as_str().unwrap_or("0"))
            );
        }

        AgentCommand::List => {
            let response = client
                .get(format!("{}/v1/agents", api))
                .send()
                .await?
                .json::<Value>()
                .await?;

            println!("Partner agents:");
            for agent in response["agents"].as_array().cloned().unwrap_or_default() {
                println!(
                    "  {:<10} {:<28} {}",
                    agent["code"].as_str().unwrap_or("?"),
                    agent["name"].as_str().unwrap_or("?"),
                    agent["address"].as_str().unwrap_or("?"),
                );
            }
        }
    }

    Ok(())
}

/// The bank.
async fn bank(api: &str, partner_key: &str, command: BankCommand) -> Result<()> {
    let client = Client::new();

    match command {
        BankCommand::Confirm { reference, amount } => {
            let amount = Money::parse(&amount)?;
            println!(
                "Transfer of {} received for reference {}.",
                amount.to_display_string(),
                reference
            );

            let response = client
                .post(format!(
                    "{}/v1/partner/bank/topups/{reference}/confirm",
                    api
                ))
                .headers(partner_headers(partner_key))
                .send()
                .await?;

            let body: Value = response.json().await?;
            println!(
                "Wallet credited: {}",
                body["status"].as_str().unwrap_or("?")
            );
        }
    }

    Ok(())
}

/// A customer using the wallet.
async fn wallet(api: &str, _partner_key: &str, command: WalletCommand) -> Result<()> {
    let client = Client::new();

    match command {
        WalletCommand::Balance { phone, pin } => {
            let session = login(&client, api, &phone, &pin).await?;
            let token = session["access_token"].as_str().unwrap_or_default();

            let response = client
                .get(format!("{}/v1/wallet/balance", api))
                .bearer_auth(token)
                .send()
                .await?
                .json::<Value>()
                .await?;

            println!(
                "Balance: {}",
                format_peso(response["balance"].as_str().unwrap_or("0"))
            );
        }

        WalletCommand::Transfer { to, amount, pin } => {
            let phone = PhoneNumber::parse(&to)?;
            let amount = Money::parse(&amount)?;

            let session = login(&client, api, "+639170000001", &pin).await?;
            let token = session["access_token"].as_str().unwrap_or_default();
            let secret = derive_demo_secret(&pin);

            let response = client
                .post(format!("{}/v1/transfers", api))
                .bearer_auth(token)
                .json(&json!({
                    "to_phone": phone.e164(),
                    "amount": amount.to_string(),
                    "idempotency_key": format!("cli-{}", leos_core::ids::new_id("idem")),
                    "pin_secret": secret,
                }))
                .send()
                .await?;

            let body: Value = response.json().await?;
            if body.get("error").is_some() {
                anyhow::bail!(
                    "{}",
                    body["error"]["message"]
                        .as_str()
                        .unwrap_or("the transfer failed")
                );
            }
            println!(
                "Sent {} to {}.",
                format_peso(body["amount"].as_str().unwrap_or("0")),
                phone.masked()
            );
            println!("Reference {}", body["reference"].as_str().unwrap_or("?"));
        }
    }

    Ok(())
}

/// Runs the whole flow, which is the quickest way to see everything work.
async fn demo(api: &str, partner_key: &str) -> Result<()> {
    let client = Client::new();

    println!("Leos end to end demo");
    println!("====================\n");
    println!("{}", leos_core::currency::SIMULATION_NOTICE);
    println!();

    // A customer signs in with the demo wallet.
    let session = login(&client, api, "+639170000001", "4821").await?;
    let token = session["access_token"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let secret = derive_demo_secret("4821");
    println!("1. Signed in as Ana Reyes (+63 917 *** 0001)");

    // The customer tops up from their bank.
    let topup: Value = client
        .post(format!("{}/v1/topups", api))
        .bearer_auth(&token)
        .json(&json!({ "amount": "2000.00" }))
        .send()
        .await?
        .json()
        .await?;
    let reference = topup["reference"].as_str().unwrap_or_default().to_string();
    println!("2. Asked for a bank transfer reference: {reference}");

    // The bank confirms it.
    client
        .post(format!(
            "{}/v1/partner/bank/topups/{reference}/confirm",
            api
        ))
        .headers(partner_headers(partner_key))
        .send()
        .await?;
    println!("3. The bank confirmed the transfer of LC 2,000.00");

    // The customer pays a merchant.
    let merchants: Value = client
        .get(format!("{}/v1/merchants", api))
        .send()
        .await?
        .json()
        .await?;
    let merchant = &merchants["merchants"][0];
    let qr = merchant["qr_payload"].as_str().unwrap_or_default();
    client
        .post(format!("{}/v1/merchants/pay", api))
        .bearer_auth(&token)
        .json(&json!({
            "qr_payload": qr,
            "amount": "250.00",
            "idempotency_key": format!("cli-{}", leos_core::ids::new_id("idem")),
            "pin_secret": secret,
        }))
        .send()
        .await?;
    println!(
        "4. Paid LC 250.00 to {} by QR",
        merchant["name"].as_str().unwrap_or("the merchant")
    );

    // An agent issues a cash-in voucher.
    let voucher: Value = client
        .post(format!("{}/v1/partner/agents/cash-in", api))
        .headers(partner_headers(partner_key))
        .json(&json!({
            "agent_code": "SMP-001",
            "phone": "+639170000001",
            "amount": "500.00",
        }))
        .send()
        .await?
        .json()
        .await?;
    let code = voucher["voucher_code"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    println!("5. Paid LC 500.00 cash at Sampaguita Cash Point, voucher code {code}");

    // The customer redeems it.
    client
        .post(format!("{}/v1/agents/redeem", api))
        .bearer_auth(&token)
        .json(&json!({ "voucher_code": code }))
        .send()
        .await?;
    println!("6. Redeemed the voucher in the app");

    // The customer sends money to a friend.
    let transfer: Value = client
        .post(format!("{}/v1/transfers", api))
        .bearer_auth(&token)
        .json(&json!({
            "to_phone": "+639170000002",
            "amount": "150.00",
            "note": "kambal",
            "idempotency_key": format!("cli-{}", leos_core::ids::new_id("idem")),
            "pin_secret": secret,
        }))
        .send()
        .await?
        .json()
        .await?;
    println!(
        "7. Sent LC 150.00 to Bello Santos, reference {}",
        transfer["reference"].as_str().unwrap_or("?")
    );

    // The customer takes cash out.
    let payout: Value = client
        .post(format!("{}/v1/agents/cash-out", api))
        .bearer_auth(&token)
        .json(&json!({
            "amount": "500.00",
            "idempotency_key": format!("cli-{}", leos_core::ids::new_id("idem")),
            "pin_secret": secret,
        }))
        .send()
        .await?
        .json()
        .await?;
    let payout_reference = payout["reference"].as_str().unwrap_or_default().to_string();
    println!("8. Asked for LC 500.00 in cash, reference {payout_reference}");

    // The agent hands the cash over.
    client
        .post(format!("{}/v1/partner/agents/payouts/confirm", api))
        .headers(partner_headers(partner_key))
        .json(&json!({ "reference": payout_reference, "agent_code": "SMP-001" }))
        .send()
        .await?;
    println!("9. Collected the cash at the agent");

    // And the balance at the end.
    let balance: Value = client
        .get(format!("{}/v1/wallet/balance", api))
        .bearer_auth(&token)
        .send()
        .await?
        .json()
        .await?;
    println!();
    println!(
        "Final balance: {}",
        format_peso(balance["balance"].as_str().unwrap_or("0"))
    );

    Ok(())
}

/// Signs in and returns the session.
async fn login(client: &Client, api: &str, phone: &str, pin: &str) -> Result<Value> {
    let response = client
        .post(format!("{}/v1/auth/login", api))
        .json(&json!({
            "phone": phone,
            "pin_secret": derive_demo_secret(pin),
        }))
        .send()
        .await?
        .json::<Value>()
        .await?;

    if response.get("error").is_some() {
        anyhow::bail!(
            "{}",
            response["error"]["message"]
                .as_str()
                .unwrap_or("the sign in failed")
        );
    }

    Ok(response)
}

/// The PIN secret the demo wallets use.
///
/// The real app derives this on the device with a keystore held secret. The
/// tools use the demo values so they can sign in to the seeded wallets.
fn derive_demo_secret(pin: &str) -> String {
    leos_core::pin::derive_secret(pin, "leos-demo-device-secret-000001", "leos-demo-salt-0001")
        .unwrap_or_default()
}

/// The headers a partner presents.
fn partner_headers(partner_key: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-leos-partner-key",
        partner_key
            .parse()
            .unwrap_or_else(|_| "dev-partner-key".parse().unwrap()),
    );
    headers
}

/// Formats a peso amount given as a decimal string.
fn format_peso(amount: &str) -> String {
    match Money::parse(amount) {
        Ok(amount) => amount.to_display_string(),
        Err(_) => amount.to_string(),
    }
}
