//! The HTTP client the mobile app talks to the Leos API through.

use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::RwLock;

use leos_core::ids::new_id;
use leos_core::money::Money;
use leos_core::pin::derive_secret;

use crate::dto::{
    AgentList, BalanceResponse, CashOutResponse, ChallengeResponse, ErrorBody, HistoryResponse,
    MeResponse, MerchantList, RedeemResponse, SessionResponse, Topup, TopupList, Transaction,
};
use crate::error::{ClientError, Result};

/// How long a request may take before it is given up on.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// The on-device client for the Leos API.
///
/// It holds the current session, so a signed in app can call the money
/// moving operations without passing tokens around.
pub struct ApiClient {
    /// The base URL of the API, without a trailing slash.
    base_url: String,
    /// The underlying HTTP client.
    http: reqwest::Client,
    /// The current session, if the user is signed in.
    session: RwLock<Option<SessionResponse>>,
}

/// The credentials a session is made of.
#[derive(Debug, Clone)]
pub struct Session {
    /// The bearer token.
    pub access_token: String,
    /// The token used to renew the session.
    pub refresh_token: String,
}

impl ApiClient {
    /// Creates a client for the given base URL.
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        if base_url.is_empty() {
            return Err(ClientError::InvalidRequest(
                "the server address is empty".to_string(),
            ));
        }

        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!("leos/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| ClientError::Network(e.to_string()))?;

        Ok(Self {
            base_url,
            http,
            session: RwLock::new(None),
        })
    }

    /// The server the client is pointed at.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The current session, if there is one.
    pub async fn session(&self) -> Option<SessionResponse> {
        self.session.read().await.clone()
    }

    /// Installs a session.
    pub async fn set_session(&self, session: SessionResponse) {
        *self.session.write().await = Some(session);
    }

    /// Forgets the current session.
    pub async fn clear_session(&self) {
        *self.session.write().await = None;
    }

    /// The bearer token of the current session.
    async fn access_token(&self) -> Result<String> {
        self.session
            .read()
            .await
            .as_ref()
            .map(|session| session.access_token.clone())
            .ok_or(ClientError::NotSignedIn)
    }

    /// Starts a registration for a mobile number.
    pub async fn register_start(&self, phone: &str) -> Result<ChallengeResponse> {
        self.send(
            Method::POST,
            "/v1/auth/register/start",
            Some(serde_json::json!({ "phone": phone })),
            false,
        )
        .await
    }

    /// Completes a registration and signs the new user in.
    pub async fn register_verify(
        &self,
        challenge_id: &str,
        code: &str,
        pin_secret: &str,
    ) -> Result<SessionResponse> {
        let session: SessionResponse = self
            .send(
                Method::POST,
                "/v1/auth/register/verify",
                Some(serde_json::json!({
                    "challenge_id": challenge_id,
                    "code": code,
                    "pin_secret": pin_secret,
                })),
                false,
            )
            .await?;

        self.set_session(session.clone()).await;
        Ok(session)
    }

    /// Signs in with a mobile number and a PIN secret.
    pub async fn login(&self, phone: &str, pin_secret: &str) -> Result<SessionResponse> {
        let session: SessionResponse = self
            .send(
                Method::POST,
                "/v1/auth/login",
                Some(serde_json::json!({ "phone": phone, "pin_secret": pin_secret })),
                false,
            )
            .await?;

        self.set_session(session.clone()).await;
        Ok(session)
    }

    /// Signs in with a mobile number and the PIN itself.
    ///
    /// The PIN is stretched into a secret on the device, so the PIN itself
    /// never crosses the network.
    pub async fn login_with_pin(
        &self,
        phone: &str,
        pin: &str,
        device_secret: &str,
        salt: &str,
    ) -> Result<SessionResponse> {
        let secret = derive_secret(pin, device_secret, salt)
            .map_err(|e| ClientError::InvalidRequest(e.to_string()))?;
        self.login(phone, &secret).await
    }

    /// Renews the session.
    pub async fn refresh(&self) -> Result<SessionResponse> {
        let refresh_token = self
            .session
            .read()
            .await
            .as_ref()
            .map(|session| session.refresh_token.clone())
            .ok_or(ClientError::NotSignedIn)?;

        let session: SessionResponse = self
            .send(
                Method::POST,
                "/v1/auth/refresh",
                Some(serde_json::json!({ "refresh_token": refresh_token })),
                false,
            )
            .await?;

        self.set_session(session.clone()).await;
        Ok(session)
    }

    /// Signs out and forgets the session.
    pub async fn logout(&self) -> Result<()> {
        let result = self
            .send::<Value>(Method::POST, "/v1/auth/logout", Some(Value::Null), true)
            .await;
        self.clear_session().await;
        result.map(|_| ())
    }

    /// The signed in user and their balance.
    pub async fn me(&self) -> Result<MeResponse> {
        self.send(Method::GET, "/v1/me", None, true).await
    }

    /// The available balance.
    pub async fn balance(&self) -> Result<BalanceResponse> {
        self.send(Method::GET, "/v1/wallet/balance", None, true)
            .await
    }

    /// The transaction history.
    pub async fn history(&self, limit: u32) -> Result<HistoryResponse> {
        self.get(&format!("/v1/transactions?limit={limit}"), true)
            .await
    }

    /// Sends money to another Leos wallet.
    pub async fn transfer(
        &self,
        to_phone: &str,
        amount: Money,
        note: Option<&str>,
        pin_secret: &str,
    ) -> Result<Transaction> {
        self.send(
            Method::POST,
            "/v1/transfers",
            Some(serde_json::json!({
                "to_phone": to_phone,
                "amount": amount.to_string(),
                "note": note,
                "idempotency_key": new_id("idem"),
                "pin_secret": pin_secret,
            })),
            true,
        )
        .await
    }

    /// Pays a merchant by their QR code.
    pub async fn pay_merchant(
        &self,
        qr_payload: &str,
        amount: Option<Money>,
        pin_secret: &str,
    ) -> Result<Transaction> {
        let amount = amount.map(|amount| amount.to_string());
        self.send(
            Method::POST,
            "/v1/merchants/pay",
            Some(serde_json::json!({
                "qr_payload": qr_payload,
                "amount": amount,
                "idempotency_key": new_id("idem"),
                "pin_secret": pin_secret,
            })),
            true,
        )
        .await
    }

    /// Redeems a cash-in voucher an agent gave the customer.
    pub async fn redeem_voucher(&self, voucher_code: &str) -> Result<RedeemResponse> {
        self.send(
            Method::POST,
            "/v1/agents/redeem",
            Some(serde_json::json!({ "voucher_code": voucher_code })),
            true,
        )
        .await
    }

    /// Withdraws cash through a partner agent.
    pub async fn cash_out(&self, amount: Money, pin_secret: &str) -> Result<CashOutResponse> {
        self.send(
            Method::POST,
            "/v1/agents/cash-out",
            Some(serde_json::json!({
                "amount": amount.to_string(),
                "idempotency_key": new_id("idem"),
                "pin_secret": pin_secret,
            })),
            true,
        )
        .await
    }

    /// Starts a top-up from a bank transfer.
    pub async fn create_topup(&self, amount: Money) -> Result<Topup> {
        self.send(
            Method::POST,
            "/v1/topups",
            Some(serde_json::json!({ "amount": amount.to_string() })),
            true,
        )
        .await
    }

    /// The user's top-ups.
    pub async fn list_topups(&self) -> Result<TopupList> {
        self.get("/v1/topups", true).await
    }

    /// The partner agents.
    pub async fn agents(&self) -> Result<AgentList> {
        self.get("/v1/agents", false).await
    }

    /// The merchants that accept Leos payments.
    pub async fn merchants(&self) -> Result<MerchantList> {
        self.get("/v1/merchants", false).await
    }

    /// Sends a GET request and decodes the response.
    async fn get<T: DeserializeOwned>(&self, path: &str, authenticated: bool) -> Result<T> {
        self.send(Method::GET, path, None, authenticated).await
    }

    /// Sends a request and decodes the response.
    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        authenticated: bool,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.http.request(method, &url);

        if authenticated {
            let token = self.access_token().await?;
            request = request.bearer_auth(token);
        }

        if let Some(body) = body {
            if !body.is_null() {
                request = request.json(&body);
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| ClientError::Network(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| ClientError::Network(e.to_string()))?;

        if status.is_success() {
            return serde_json::from_str(&text)
                .map_err(|e| ClientError::Decode(format!("{e}: {text}")));
        }

        // The API always answers with a structured error.
        if let Ok(body) = serde_json::from_str::<ErrorBody>(&text) {
            return Err(ClientError::Api {
                code: body.error.code,
                message: body.error.message,
            });
        }

        Err(ClientError::Api {
            code: status_code_code(status),
            message: if text.is_empty() {
                format!("the server returned {status}")
            } else {
                text
            },
        })
    }
}

/// Maps an HTTP status onto a Leos style error code.
fn status_code_code(status: StatusCode) -> String {
    match status {
        StatusCode::UNAUTHORIZED => "unauthorized",
        StatusCode::FORBIDDEN => "forbidden",
        StatusCode::NOT_FOUND => "not_found",
        StatusCode::CONFLICT => "conflict",
        StatusCode::TOO_MANY_REQUESTS => "rate_limited",
        StatusCode::UNPROCESSABLE_ENTITY => "unprocessable",
        StatusCode::BAD_REQUEST => "invalid_request",
        _ => "server_error",
    }
    .to_string()
}
