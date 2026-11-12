//! Authentication: PIN secrets, one-time passcodes and session tokens.
//!
//! The server never receives a wallet PIN. The device derives a high entropy
//! secret from the PIN with Argon2id and a device bound salt, and sends that
//! secret. The server stores a verifier — a second Argon2id hash of the
//! secret — so a database leak does not hand over the PIN, and a leaked
//! verifier cannot be replayed against another device because each device
//! salts differently.

use crate::error::ServerError;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;
use sha2::{Digest, Sha256};

use leos_core::error::LeosError;
use leos_core::phone::PhoneNumber;

use crate::state::AppState;

/// How many wrong PIN attempts before the account is locked.
pub const MAX_PIN_ATTEMPTS: i32 = 5;

/// How long an account stays locked after too many wrong attempts.
pub const LOCKOUT: chrono::Duration = chrono::Duration::minutes(15);

/// Argon2id parameters for the server side verifier.
///
/// The secret arriving from the device is already high entropy, so these
/// parameters only need to be slow enough to make an offline attack against a
/// stolen database impractical.
fn verifier_params() -> Params {
    Params::new(19 * 1024, 2, 1, None).expect("valid argon2 parameters")
}

/// Hashes a PIN secret into a verifier string.
pub fn hash_secret(secret: &str) -> Result<(String, String), ServerError> {
    let salt_bytes: [u8; 16] = rand::rng().random();
    let salt = SaltString::encode_b64(&salt_bytes)
        .map_err(|_| LeosError::Internal("could not build salt".to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, verifier_params());
    let hash = argon2
        .hash_password(secret.as_bytes(), &salt)
        .map_err(|_| LeosError::Internal("could not hash secret".to_string()))?;

    Ok((hash.to_string(), salt.to_string()))
}

/// Checks a PIN secret against a stored verifier.
pub fn verify_secret(secret: &str, verifier: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(verifier) else {
        return false;
    };
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, verifier_params());
    argon2.verify_password(secret.as_bytes(), &parsed).is_ok()
}

/// Hashes a one-time passcode with SHA-256.
///
/// The code is short lived and never leaves the database in the clear, and a
/// fast hash is enough here because the code itself is only valid for a few
/// minutes and is rate limited.
pub fn hash_code(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    hex(&hasher.finalize())
}

/// Generates a cryptographically random token.
pub fn random_token() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Hashes a token so the database never holds a usable credential.
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex(&hasher.finalize())
}

/// Encodes bytes as lowercase hex.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The authenticated caller.
#[derive(Debug, Clone)]
pub struct AuthUser {
    /// The user's identifier.
    pub user_id: String,
    /// The user's mobile number.
    pub phone: PhoneNumber,
    /// The session the request came in on.
    pub session_id: String,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ServerError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .ok_or_else(|| LeosError::Unauthorized("missing authorization header".to_string()))?
            .to_str()
            .map_err(|_| LeosError::Unauthorized("malformed authorization header".to_string()))?;

        let token = header
            .strip_prefix("Bearer ")
            .or_else(|| header.strip_prefix("bearer "))
            .ok_or_else(|| {
                LeosError::Unauthorized("authorization header must be a bearer token".to_string())
            })?
            .trim();

        if token.is_empty() {
            return Err(LeosError::Unauthorized("empty bearer token".to_string()).into());
        }

        state.authenticate(token).await
    }
}
