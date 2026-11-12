//! The JSON operation dispatcher.
//!
//! The native UI talks to this crate through a single entry point that takes
//! a JSON request and returns a JSON response. Keeping the boundary at one
//! function means the Android and iOS apps need no generated bindings, and
//! adding an operation never means touching the mobile build.

use serde_json::{Value, json};

use leos_core::money::Money;
use leos_core::phone::PhoneNumber;
use leos_core::pin::derive_secret;
use leos_core::policy::validate_pin;
use leos_core::qr::MerchantQr;

use crate::api::ApiClient;
use crate::error::ClientError;

/// The on-device wallet core.
pub struct Wallet {
    /// The API client.
    client: ApiClient,
}

impl Wallet {
    /// Creates a wallet core pointed at an API.
    pub fn new(base_url: impl Into<String>) -> Result<Self, ClientError> {
        Ok(Self {
            client: ApiClient::new(base_url)?,
        })
    }

    /// The API client, for callers that want the typed API.
    pub fn client(&self) -> &ApiClient {
        &self.client
    }

    /// Handles one request.
    ///
    /// The result is always JSON: `{"ok": true, "data": ...}` or
    /// `{"ok": false, "error": {...}}`.
    pub async fn dispatch(&self, request: &str) -> String {
        let parsed: Value = match serde_json::from_str(request) {
            Ok(value) => value,
            Err(error) => {
                return failure(&ClientError::InvalidRequest(format!(
                    "the request was not valid JSON: {error}"
                )))
                .to_string();
            }
        };

        let op = parsed
            .get("op")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        match self.handle(&op, &parsed).await {
            Ok(data) => json!({ "ok": true, "data": data }).to_string(),
            Err(error) => failure(&error).to_string(),
        }
    }

    /// Runs one operation.
    async fn handle(&self, op: &str, request: &Value) -> Result<Value, ClientError> {
        match op {
            // Operations that never touch the network.
            "derive_secret" => {
                let pin = field(request, "pin")?;
                let device_secret = field(request, "device_secret")?;
                let salt = field(request, "salt")?;
                let secret = derive_secret(&pin, &device_secret, &salt)
                    .map_err(|e| ClientError::InvalidRequest(e.to_string()))?;
                Ok(json!({ "secret": secret }))
            }
            "validate_pin" => {
                let pin = field(request, "pin")?;
                match validate_pin(&pin) {
                    Ok(()) => Ok(json!({ "valid": true })),
                    Err(violation) => Ok(json!({
                        "valid": false,
                        "reason": violation.to_string(),
                    })),
                }
            }
            "normalize_phone" => {
                let raw = field(request, "phone")?;
                let phone = PhoneNumber::parse(&raw)
                    .map_err(|e| ClientError::InvalidRequest(e.to_string()))?;
                Ok(json!({
                    "e164": phone.e164(),
                    "masked": phone.masked(),
                    "mobile": phone.is_mobile(),
                }))
            }
            "parse_qr" => {
                let payload = field(request, "payload")?;
                let qr = MerchantQr::from_payload(&payload)
                    .map_err(|e| ClientError::InvalidRequest(e.to_string()))?;
                Ok(json!({
                    "merchant_name": qr.merchant_name,
                    "merchant_city": qr.merchant_city,
                    "merchant_id": qr.merchant_id(),
                    "amount": qr.amount.map(|amount| amount.to_string()),
                    "currency": qr.currency,
                    "country": qr.country,
                    "domestic": qr.is_domestic(),
                    "supported_currency": qr.is_supported_currency(),
                }))
            }
            "format_amount" => {
                let raw = field(request, "amount")?;
                let amount =
                    Money::parse(&raw).map_err(|e| ClientError::InvalidRequest(e.to_string()))?;
                Ok(json!({
                    "plain": amount.to_string(),
                    "formatted": amount.to_display_string(),
                    "centavos": amount.centavos(),
                }))
            }

            // Account.
            "register_start" => {
                let phone = field(request, "phone")?;
                let challenge = self.client.register_start(&phone).await?;
                Ok(to_value(&challenge))
            }
            "register_verify" => {
                let challenge_id = field(request, "challenge_id")?;
                let code = field(request, "code")?;
                let pin_secret = field(request, "pin_secret")?;
                let session = self
                    .client
                    .register_verify(&challenge_id, &code, &pin_secret)
                    .await?;
                Ok(to_value(&session))
            }
            "login" => {
                let phone = field(request, "phone")?;
                let pin_secret = field(request, "pin_secret")?;
                let session = self.client.login(&phone, &pin_secret).await?;
                Ok(to_value(&session))
            }
            "login_with_pin" => {
                let phone = field(request, "phone")?;
                let pin = field(request, "pin")?;
                let device_secret = field(request, "device_secret")?;
                let salt = field(request, "salt")?;
                let session = self
                    .client
                    .login_with_pin(&phone, &pin, &device_secret, &salt)
                    .await?;
                Ok(to_value(&session))
            }
            "logout" => {
                self.client.logout().await?;
                Ok(json!({ "ok": true }))
            }
            "session" => match self.client.session().await {
                Some(session) => Ok(to_value(&session)),
                None => Ok(Value::Null),
            },
            "set_session" => {
                let session = request
                    .get("session")
                    .cloned()
                    .ok_or_else(|| ClientError::InvalidRequest("no session given".to_string()))?;
                let session: crate::dto::SessionResponse = serde_json::from_value(session)
                    .map_err(|e| ClientError::InvalidRequest(e.to_string()))?;
                self.client.set_session(session).await;
                Ok(json!({ "ok": true }))
            }

            // Wallet.
            "me" => {
                let me = self.client.me().await?;
                Ok(to_value(&me))
            }
            "balance" => {
                let balance = self.client.balance().await?;
                Ok(to_value(&balance))
            }
            "history" => {
                let limit = request.get("limit").and_then(Value::as_u64).unwrap_or(50) as u32;
                let history = self.client.history(limit).await?;
                Ok(to_value(&history))
            }

            // Money movement.
            "transfer" => {
                let to_phone = field(request, "to_phone")?;
                let amount = money_field(request, "amount")?;
                let note = request.get("note").and_then(Value::as_str);
                let pin_secret = field(request, "pin_secret")?;
                let transaction = self
                    .client
                    .transfer(&to_phone, amount, note, &pin_secret)
                    .await?;
                Ok(to_value(&transaction))
            }
            "pay_merchant" => {
                let qr_payload = field(request, "qr_payload")?;
                let amount = match request.get("amount").and_then(Value::as_str) {
                    Some(raw) => Some(
                        Money::parse(raw)
                            .map_err(|e| ClientError::InvalidRequest(e.to_string()))?,
                    ),
                    None => None,
                };
                let pin_secret = field(request, "pin_secret")?;
                let transaction = self
                    .client
                    .pay_merchant(&qr_payload, amount, &pin_secret)
                    .await?;
                Ok(to_value(&transaction))
            }
            "redeem" => {
                let voucher_code = field(request, "voucher_code")?;
                let redeemed = self.client.redeem_voucher(&voucher_code).await?;
                Ok(to_value(&redeemed))
            }
            "cash_out" => {
                let amount = money_field(request, "amount")?;
                let pin_secret = field(request, "pin_secret")?;
                let payout = self.client.cash_out(amount, &pin_secret).await?;
                Ok(to_value(&payout))
            }
            "create_topup" => {
                let amount = money_field(request, "amount")?;
                let topup = self.client.create_topup(amount).await?;
                Ok(to_value(&topup))
            }
            "list_topups" => {
                let topups = self.client.list_topups().await?;
                Ok(to_value(&topups))
            }
            "agents" => {
                let agents = self.client.agents().await?;
                Ok(to_value(&agents))
            }
            "merchants" => {
                let merchants = self.client.merchants().await?;
                Ok(to_value(&merchants))
            }

            other => Err(ClientError::InvalidRequest(format!(
                "unknown operation {other}"
            ))),
        }
    }
}

/// Reads a required string field.
fn field(request: &Value, name: &str) -> Result<String, ClientError> {
    request
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ClientError::InvalidRequest(format!("the field {name} is missing")))
}

/// Reads a required amount field.
fn money_field(request: &Value, name: &str) -> Result<Money, ClientError> {
    let raw = field(request, name)?;
    Money::parse(&raw).map_err(|e| ClientError::InvalidRequest(e.to_string()))
}

/// Serialises a value, falling back to null if it cannot be encoded.
fn to_value<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// Builds the error envelope.
fn failure(error: &ClientError) -> Value {
    json!({
        "ok": false,
        "error": {
            "code": error.code(),
            "message": error.to_string(),
            "retryable": error.is_retryable(),
        }
    })
}
