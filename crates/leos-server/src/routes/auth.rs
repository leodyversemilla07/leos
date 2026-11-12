//! Registration, login, PIN reset and session management.

use crate::error::ServerError;
use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use leos_core::error::LeosError;
use leos_core::ids::{new_id, random_otp};
use leos_core::phone::PhoneNumber;
use leos_core::policy::validate_pin;

use crate::auth::{
    LOCKOUT, MAX_PIN_ATTEMPTS, hash_code, hash_secret, hash_token, random_token, verify_secret,
};
use crate::ledger;
use crate::state::AppState;

/// A client derived PIN secret is 32 bytes, hex encoded.
const PIN_SECRET_LEN: usize = 64;

/// Starts a registration challenge for a phone number.
#[derive(Debug, Deserialize)]
pub struct StartRegistration {
    /// The mobile number to register.
    pub phone: String,
}

/// The result of starting a challenge.
#[derive(Debug, Serialize)]
pub struct ChallengeResponse {
    /// The challenge identifier.
    pub challenge_id: String,
    /// When the challenge expires.
    pub expires_at: DateTime<Utc>,
    /// The one-time passcode, only ever returned in development.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_code: Option<String>,
}

/// Completes a registration challenge and sets the wallet PIN.
#[derive(Debug, Deserialize)]
pub struct VerifyRegistration {
    /// The challenge to complete.
    pub challenge_id: String,
    /// The one-time passcode that was sent.
    pub code: String,
    /// The PIN secret derived on the device.
    pub pin_secret: String,
}

/// Signs in with a phone number and PIN secret.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// The registered mobile number.
    pub phone: String,
    /// The PIN secret derived on the device.
    pub pin_secret: String,
}

/// Starts a PIN reset challenge.
#[derive(Debug, Deserialize)]
pub struct StartPinReset {
    /// The registered mobile number.
    pub phone: String,
}

/// Completes a PIN reset challenge.
#[derive(Debug, Deserialize)]
pub struct VerifyPinReset {
    /// The challenge to complete.
    pub challenge_id: String,
    /// The one-time passcode that was sent.
    pub code: String,
    /// The new PIN secret derived on the device.
    pub pin_secret: String,
}

/// Exchanges a refresh token for a new session.
#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    /// The refresh token.
    pub refresh_token: String,
}

/// A user as the API reports it.
#[derive(Debug, Clone, Serialize)]
pub struct UserDto {
    /// The user identifier.
    pub id: String,
    /// The mobile number in E.164 form.
    pub phone: String,
    /// The display name shown to counterparties.
    pub display_name: Option<String>,
    /// The account status.
    pub status: String,
}

/// A new session.
#[derive(Debug, Serialize)]
pub struct SessionResponse {
    /// The bearer token for the API.
    pub access_token: String,
    /// The token used to obtain a new access token.
    pub refresh_token: String,
    /// How long the access token is valid for, in seconds.
    pub expires_in: i64,
    /// The signed in user.
    pub user: UserDto,
}

/// `POST /v1/auth/register/start`
pub async fn start_registration(
    State(state): State<AppState>,
    Json(request): Json<StartRegistration>,
) -> Result<Json<ChallengeResponse>, ServerError> {
    let phone = parse_mobile(&request.phone)?;

    let existing: Option<(String, String)> =
        sqlx::query_as("SELECT id, status FROM users WHERE phone = $1")
            .bind(phone.e164())
            .fetch_optional(&state.pool)
            .await?;

    if let Some((_, status)) = existing {
        if status != "pending" {
            return Err(
                LeosError::Conflict("this number is already registered".to_string()).into(),
            );
        }
    }

    let challenge = create_challenge(&state, &phone, "registration").await?;
    Ok(Json(challenge))
}

/// `POST /v1/auth/register/verify`
pub async fn verify_registration(
    State(state): State<AppState>,
    Json(request): Json<VerifyRegistration>,
) -> Result<Json<SessionResponse>, ServerError> {
    validate_pin_secret(&request.pin_secret)?;
    let pin_secret = &request.pin_secret;
    let phone =
        consume_challenge(&state, &request.challenge_id, &request.code, "registration").await?;

    let mut tx = state.pool.begin().await?;

    let existing: Option<(String, String)> =
        sqlx::query_as("SELECT id, status FROM users WHERE phone = $1 FOR UPDATE")
            .bind(phone.e164())
            .fetch_optional(&mut *tx)
            .await?;

    let user_id = match existing {
        Some((id, status)) => {
            if status == "active" {
                return Err(
                    LeosError::Conflict("this number is already registered".to_string()).into(),
                );
            }
            id
        }
        None => {
            let id = new_id("usr");
            sqlx::query(
                "INSERT INTO users (id, phone, status, pin_verifier, pin_salt)
                 VALUES ($1, $2, 'pending', '', '')",
            )
            .bind(&id)
            .bind(phone.e164())
            .execute(&mut *tx)
            .await?;
            id
        }
    };

    let (verifier, salt) = hash_secret(pin_secret)?;
    sqlx::query(
        "UPDATE users SET pin_verifier = $2, pin_salt = $3, status = 'active',
                failed_attempts = 0, locked_until = NULL, updated_at = now()
         WHERE id = $1",
    )
    .bind(&user_id)
    .bind(&verifier)
    .bind(&salt)
    .execute(&mut *tx)
    .await?;

    ledger::ensure_user_account(&mut tx, &user_id).await?;
    tx.commit().await?;

    issue_session(&state, &user_id, None).await.map(Json)
}

/// `POST /v1/auth/login`
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<SessionResponse>, ServerError> {
    let phone = parse_mobile(&request.phone)?;

    let row = sqlx::query(
        "SELECT id, pin_verifier, failed_attempts, locked_until, status FROM users WHERE phone = $1",
    )
    .bind(phone.e164())
    .fetch_optional(&state.pool)
    .await?;

    let Some(row) = row else {
        // Spend roughly the same time as a real check so the response does
        // not reveal whether the number is registered.
        verify_secret(&request.pin_secret, DUMMY_VERIFIER);
        return Err(LeosError::Unauthorized("invalid credentials".to_string()).into());
    };

    let status: String = row.get("status");
    if status != "active" {
        return Err(LeosError::Forbidden(format!("account is {status}")).into());
    }

    let locked_until: Option<DateTime<Utc>> = row.get("locked_until");
    if let Some(until) = locked_until {
        if until > Utc::now() {
            return Err(LeosError::Locked(
                "too many incorrect attempts, try again later".to_string(),
            )
            .into());
        }
    }

    let user_id: String = row.get("id");
    let verifier: String = row.get("pin_verifier");

    if !verify_secret(&request.pin_secret, &verifier) {
        let failed: i32 = row.get("failed_attempts");
        let attempts = failed + 1;
        let lock = attempts >= MAX_PIN_ATTEMPTS;
        sqlx::query(
            "UPDATE users SET failed_attempts = $2, locked_until = $3, updated_at = now()
             WHERE id = $1",
        )
        .bind(&user_id)
        .bind(attempts)
        .bind(if lock {
            Some(Utc::now() + LOCKOUT)
        } else {
            None
        })
        .execute(&state.pool)
        .await?;

        if lock {
            return Err(LeosError::Locked(
                "too many incorrect attempts, try again later".to_string(),
            )
            .into());
        }
        return Err(LeosError::Unauthorized("invalid credentials".to_string()).into());
    }

    sqlx::query(
        "UPDATE users SET failed_attempts = 0, locked_until = NULL, updated_at = now()
         WHERE id = $1",
    )
    .bind(&user_id)
    .execute(&state.pool)
    .await?;

    issue_session(&state, &user_id, None).await.map(Json)
}

/// `POST /v1/auth/pin/reset/start`
pub async fn start_pin_reset(
    State(state): State<AppState>,
    Json(request): Json<StartPinReset>,
) -> Result<Json<ChallengeResponse>, ServerError> {
    let phone = parse_mobile(&request.phone)?;

    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE phone = $1")
        .bind(phone.e164())
        .fetch_optional(&state.pool)
        .await?;

    if exists.is_none() {
        return Err(LeosError::NotFound("no wallet for that number".to_string()).into());
    }

    let challenge = create_challenge(&state, &phone, "pin_reset").await?;
    Ok(Json(challenge))
}

/// `POST /v1/auth/pin/reset/verify`
pub async fn verify_pin_reset(
    State(state): State<AppState>,
    Json(request): Json<VerifyPinReset>,
) -> Result<Json<SessionResponse>, ServerError> {
    validate_pin_secret(&request.pin_secret)?;
    let pin_secret = &request.pin_secret;
    let phone =
        consume_challenge(&state, &request.challenge_id, &request.code, "pin_reset").await?;

    let user_id: String = sqlx::query_scalar("SELECT id FROM users WHERE phone = $1")
        .bind(phone.e164())
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| LeosError::NotFound("no wallet for that number".to_string()))?;

    let (verifier, salt) = hash_secret(pin_secret)?;
    sqlx::query(
        "UPDATE users SET pin_verifier = $2, pin_salt = $3, failed_attempts = 0,
                locked_until = NULL, updated_at = now()
         WHERE id = $1",
    )
    .bind(&user_id)
    .bind(&verifier)
    .bind(&salt)
    .execute(&state.pool)
    .await?;

    issue_session(&state, &user_id, None).await.map(Json)
}

/// `POST /v1/auth/refresh`
pub async fn refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<SessionResponse>, ServerError> {
    let refresh_hash = hash_token(&request.refresh_token);

    let row = sqlx::query(
        "SELECT id, user_id, expires_at, revoked_at FROM sessions WHERE refresh_hash = $1",
    )
    .bind(&refresh_hash)
    .fetch_optional(&state.pool)
    .await?;

    let Some(row) = row else {
        return Err(LeosError::Unauthorized("unknown refresh token".to_string()).into());
    };

    let revoked_at: Option<DateTime<Utc>> = row.get("revoked_at");
    if revoked_at.is_some() {
        return Err(LeosError::Unauthorized("session has been revoked".to_string()).into());
    }

    let expires_at: DateTime<Utc> = row.get("expires_at");
    if expires_at <= Utc::now() {
        return Err(LeosError::Unauthorized("session has expired".to_string()).into());
    }

    let user_id: String = row.get("user_id");
    let old_session_id: String = row.get("id");

    // Rotate both tokens so a leaked refresh token is only good once.
    let access_token = random_token();
    let refresh_token = random_token();

    sqlx::query("UPDATE sessions SET id = $2, refresh_hash = $3, expires_at = $4 WHERE id = $1")
        .bind(&old_session_id)
        .bind(hash_token(&access_token))
        .bind(hash_token(&refresh_token))
        .bind(Utc::now() + state.config.session_ttl)
        .execute(&state.pool)
        .await?;

    let user = load_user(&state, &user_id).await?;
    Ok(Json(SessionResponse {
        access_token,
        refresh_token,
        expires_in: state.config.session_ttl.as_secs() as i64,
        user,
    }))
}

/// `POST /v1/auth/logout`
pub async fn logout(
    State(state): State<AppState>,
    user: crate::auth::AuthUser,
) -> Result<Json<serde_json::Value>, ServerError> {
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE id = $1")
        .bind(&user.session_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Normalizes a phone number and insists it is a mobile number.
fn parse_mobile(raw: &str) -> Result<PhoneNumber, ServerError> {
    let phone =
        PhoneNumber::parse(raw).map_err(|e| LeosError::InvalidPhoneNumber(e.to_string()))?;
    if !phone.is_mobile() {
        return Err(LeosError::InvalidPhoneNumber(
            "Leos wallets are registered with a mobile number".to_string(),
        )
        .into());
    }
    Ok(phone)
}

/// Checks the shape of a client derived PIN secret.
pub fn validate_pin_secret(secret: &str) -> Result<(), ServerError> {
    if secret.len() != PIN_SECRET_LEN || !secret.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(LeosError::InvalidPin("the PIN secret is malformed".to_string()).into());
    }
    Ok(())
}

/// Creates a one-time passcode challenge.
async fn create_challenge(
    state: &AppState,
    phone: &PhoneNumber,
    purpose: &str,
) -> Result<ChallengeResponse, ServerError> {
    let code = random_otp(leos_core::policy::OTP_LEN);
    let challenge_id = new_id("chl");
    let expires_at = Utc::now() + state.config.otp_ttl;

    sqlx::query(
        "INSERT INTO auth_challenges (id, phone, code_hash, purpose, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&challenge_id)
    .bind(phone.e164())
    .bind(hash_code(&code))
    .bind(purpose)
    .bind(expires_at)
    .execute(&state.pool)
    .await?;

    // In production this is where the SMS gateway would be called.
    tracing::info!(challenge = %challenge_id, phone = %phone.masked(), purpose, "one-time passcode issued");
    if state.config.dev_mode {
        tracing::info!(code, "development one-time passcode");
    }

    Ok(ChallengeResponse {
        challenge_id,
        expires_at,
        dev_code: state.config.dev_mode.then_some(code),
    })
}

/// Verifies a one-time passcode and consumes the challenge.
async fn consume_challenge(
    state: &AppState,
    challenge_id: &str,
    code: &str,
    purpose: &str,
) -> Result<PhoneNumber, ServerError> {
    let row = sqlx::query(
        "SELECT phone, code_hash, purpose, attempts, expires_at, consumed_at
         FROM auth_challenges WHERE id = $1",
    )
    .bind(challenge_id)
    .fetch_optional(&state.pool)
    .await?;

    let Some(row) = row else {
        return Err(LeosError::NotFound("unknown challenge".to_string()).into());
    };

    let stored_purpose: String = row.get("purpose");
    if stored_purpose != purpose {
        return Err(LeosError::InvalidOtp.into());
    }

    let consumed_at: Option<DateTime<Utc>> = row.get("consumed_at");
    if consumed_at.is_some() {
        return Err(LeosError::InvalidOtp.into());
    }

    let expires_at: DateTime<Utc> = row.get("expires_at");
    if expires_at <= Utc::now() {
        return Err(LeosError::InvalidOtp.into());
    }

    let attempts: i32 = row.get("attempts");
    if attempts >= 5 {
        return Err(LeosError::Locked("too many incorrect codes".to_string()).into());
    }

    let stored_hash: String = row.get("code_hash");
    if !constant_time_eq(&stored_hash, &hash_code(code)) {
        sqlx::query("UPDATE auth_challenges SET attempts = attempts + 1 WHERE id = $1")
            .bind(challenge_id)
            .execute(&state.pool)
            .await?;
        return Err(LeosError::InvalidOtp.into());
    }

    sqlx::query("UPDATE auth_challenges SET consumed_at = now() WHERE id = $1")
        .bind(challenge_id)
        .execute(&state.pool)
        .await?;

    let phone_raw: String = row.get("phone");
    PhoneNumber::parse(&phone_raw)
        .map_err(|e| LeosError::Internal(format!("stored phone number is invalid: {e}")).into())
}

/// Issues a session for a user.
pub async fn issue_session(
    state: &AppState,
    user_id: &str,
    device_label: Option<String>,
) -> Result<SessionResponse, ServerError> {
    let access_token = random_token();
    let refresh_token = random_token();

    sqlx::query(
        "INSERT INTO sessions (id, user_id, refresh_hash, device_label, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(hash_token(&access_token))
    .bind(user_id)
    .bind(hash_token(&refresh_token))
    .bind(device_label)
    .bind(Utc::now() + state.config.session_ttl)
    .execute(&state.pool)
    .await?;

    let user = load_user(state, user_id).await?;

    Ok(SessionResponse {
        access_token,
        refresh_token,
        expires_in: state.config.session_ttl.as_secs() as i64,
        user,
    })
}

/// Loads a user for the API.
pub async fn load_user(state: &AppState, user_id: &str) -> Result<UserDto, ServerError> {
    let row = sqlx::query("SELECT id, phone, display_name, status FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?;

    let row = row.ok_or_else(|| LeosError::NotFound("user not found".to_string()))?;

    Ok(UserDto {
        id: row.get("id"),
        phone: row.get("phone"),
        display_name: row.get("display_name"),
        status: row.get("status"),
    })
}

/// Compares two strings without leaking their contents through timing.
fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in left.bytes().zip(right.bytes()) {
        difference |= a ^ b;
    }
    difference == 0
}

/// A verifier for numbers that are not registered, used to keep the time
/// taken to answer a login the same either way.
const DUMMY_VERIFIER: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHRzb21lc2FsdA$Ry9J0Yy0lW2jL3VwQ0h6bFZ2Y3R0eXc";

/// Checks a PIN against the policy. Exposed for the partner tooling.
pub fn check_pin_policy(pin: &str) -> Result<(), ServerError> {
    validate_pin(pin).map_err(|e| LeosError::InvalidPin(e.to_string()).into())
}

/// Verifies the PIN secret supplied with a money moving request.
///
/// The same rules apply as at login: too many wrong attempts lock the
/// wallet, and the account is locked out for a cooling off period.
pub async fn verify_pin(
    state: &AppState,
    user_id: &str,
    pin_secret: &str,
) -> Result<(), ServerError> {
    let row =
        sqlx::query("SELECT pin_verifier, failed_attempts, locked_until FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&state.pool)
            .await?;

    let Some(row) = row else {
        return Err(LeosError::Unauthorized("invalid credentials".to_string()).into());
    };

    let locked_until: Option<DateTime<Utc>> = row.get("locked_until");
    if let Some(until) = locked_until {
        if until > Utc::now() {
            return Err(LeosError::Locked(
                "too many incorrect attempts, try again later".to_string(),
            )
            .into());
        }
    }

    let verifier: String = row.get("pin_verifier");
    if !verify_secret(pin_secret, &verifier) {
        let failed: i32 = row.get("failed_attempts");
        let attempts = failed + 1;
        let lock = attempts >= MAX_PIN_ATTEMPTS;
        sqlx::query(
            "UPDATE users SET failed_attempts = $2, locked_until = $3, updated_at = now()
             WHERE id = $1",
        )
        .bind(user_id)
        .bind(attempts)
        .bind(if lock {
            Some(Utc::now() + LOCKOUT)
        } else {
            None
        })
        .execute(&state.pool)
        .await?;

        if lock {
            return Err(LeosError::Locked(
                "too many incorrect attempts, try again later".to_string(),
            )
            .into());
        }
        return Err(LeosError::Unauthorized("incorrect PIN".to_string()).into());
    }

    sqlx::query(
        "UPDATE users SET failed_attempts = 0, locked_until = NULL, updated_at = now()
         WHERE id = $1",
    )
    .bind(user_id)
    .execute(&state.pool)
    .await?;

    Ok(())
}
