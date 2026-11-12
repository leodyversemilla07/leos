//! Server configuration, read from the environment.

use std::net::SocketAddr;
use std::time::Duration;

/// Runtime configuration for the Leos API.
#[derive(Debug, Clone)]
pub struct Config {
    /// The Postgres connection string.
    pub database_url: String,
    /// The address to bind the HTTP server to.
    pub bind_addr: SocketAddr,
    /// When true, one-time passcodes are returned in API responses so the
    /// flow can be exercised without an SMS gateway. Never enable in
    /// production.
    pub dev_mode: bool,
    /// How long a one-time passcode stays valid.
    pub otp_ttl: Duration,
    /// How long an access token stays valid.
    pub session_ttl: Duration,
    /// How long a refresh token stays valid.
    pub refresh_ttl: Duration,
    /// Whether the API accepts requests from any origin.
    pub allow_any_origin: bool,
}

impl Config {
    /// Reads the configuration from the environment, applying defaults.
    pub fn from_env() -> Result<Self, String> {
        let database_url = std::env::var("LEOS_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .map_err(|_| "LEOS_DATABASE_URL (or DATABASE_URL) must be set".to_string())?;

        let bind_addr = std::env::var("LEOS_BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
            .parse()
            .map_err(|e| format!("LEOS_BIND_ADDR is not a valid socket address: {e}"))?;

        let otp_ttl = Duration::from_secs(env_u64("LEOS_OTP_TTL_SECONDS", 300));
        let session_ttl = Duration::from_secs(env_u64("LEOS_SESSION_TTL_SECONDS", 900));
        let refresh_ttl =
            Duration::from_secs(env_u64("LEOS_REFRESH_TTL_SECONDS", 60 * 60 * 24 * 30));

        Ok(Self {
            database_url,
            bind_addr,
            dev_mode: env_bool("LEOS_DEV_MODE", false),
            otp_ttl,
            session_ttl,
            refresh_ttl,
            allow_any_origin: env_bool("LEOS_ALLOW_ANY_ORIGIN", true),
        })
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(value) => value.eq_ignore_ascii_case("true") || value == "1",
        Err(_) => default,
    }
}
