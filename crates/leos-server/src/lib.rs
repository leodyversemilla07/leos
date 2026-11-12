//! # leos-server
//!
//! The Leos wallet API. The server owns the ledger: it decides what a
//! balance is, it checks the limits, and it records every unit that moves.
//! The mobile app is a client of this API, not a holder of truth.
//!
//! Balances are derived from postings rather than stored as a single
//! mutable number, and every transaction is posted as a balanced journal, so
//! the money in the system always adds up.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod auth;
pub mod config;
pub mod error;
pub mod idempotency;
pub mod ledger;
pub mod routes;
pub mod seed;
pub mod state;

use crate::error::ServerError;
use axum::Router;
use axum::routing::{get, post};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use leos_core::error::LeosError;

use crate::config::Config;
use crate::state::AppState;

/// A simple health check.
///
/// It also says what this is, so that anything talking to the API is told
/// straight away that it is talking to a simulation.
async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "service": "leos-server",
        "version": env!("CARGO_PKG_VERSION"),
        "mode": "simulation",
        "currency": leos_core::currency::LEOS_CREDITS.code,
        "notice": leos_core::currency::SIMULATION_NOTICE,
    }))
}

/// What the wallet is, for anyone who wants to know before they use it.
async fn info() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "mode": "simulation",
        "notice": leos_core::currency::SIMULATION_NOTICE,
        "badge": leos_core::currency::SIMULATION_BADGE,
        "currency": {
            "code": leos_core::currency::LEOS_CREDITS.code,
            "name": leos_core::currency::LEOS_CREDITS.name,
            "symbol": leos_core::currency::LEOS_CREDITS.symbol,
        },
    }))
}

/// Builds the HTTP router.
pub fn router(state: AppState) -> Router {
    let cors = if state.config.allow_any_origin {
        CorsLayer::permissive()
    } else {
        CorsLayer::default()
    };

    Router::new()
        .route("/health", get(health))
        .route("/v1/info", get(info))
        // Registration, login and sessions.
        .route(
            "/v1/auth/register/start",
            post(routes::auth::start_registration),
        )
        .route(
            "/v1/auth/register/verify",
            post(routes::auth::verify_registration),
        )
        .route("/v1/auth/login", post(routes::auth::login))
        .route(
            "/v1/auth/pin/reset/start",
            post(routes::auth::start_pin_reset),
        )
        .route(
            "/v1/auth/pin/reset/verify",
            post(routes::auth::verify_pin_reset),
        )
        .route("/v1/auth/refresh", post(routes::auth::refresh))
        .route("/v1/auth/logout", post(routes::auth::logout))
        // The wallet itself.
        .route("/v1/me", get(routes::wallet::me))
        .route("/v1/wallet/balance", get(routes::wallet::balance))
        .route("/v1/transactions", get(routes::wallet::history))
        // Money movement.
        .route("/v1/transfers", post(routes::transfer::transfer))
        .route("/v1/agents", get(routes::cash::list_agents))
        .route("/v1/agents/cash-out", post(routes::cash::cash_out))
        .route("/v1/agents/redeem", post(routes::cash::redeem_voucher))
        .route("/v1/merchants", get(routes::merchant::list_merchants))
        .route("/v1/merchants/pay", post(routes::merchant::pay))
        .route("/v1/topups", post(routes::topup::create_topup))
        .route("/v1/topups", get(routes::topup::list_topups))
        // The partner side of the rails.
        .route(
            "/v1/partner/agents/cash-in",
            post(routes::partner::issue_cash_in),
        )
        .route(
            "/v1/partner/agents/payouts",
            get(routes::partner::list_pending_payouts),
        )
        .route(
            "/v1/partner/agents/payouts/confirm",
            post(routes::partner::confirm_payout),
        )
        .route(
            "/v1/partner/bank/topups/{reference}/confirm",
            post(routes::partner::confirm_bank_topup),
        )
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

/// Connects to the database, runs migrations and serves the API.
pub async fn serve(config: Config) -> Result<(), ServerError> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(16)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("database is up to date");

    if std::env::var("LEOS_SEED").unwrap_or_default() == "true" {
        seed::seed(&pool).await?;
        tracing::info!("demo data is ready");
    }

    let state = AppState {
        pool: pool.clone(),
        config: std::sync::Arc::new(config.clone()),
    };

    let app = router(state);

    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .map_err(|e| LeosError::Internal(format!("could not bind {}: {e}", config.bind_addr)))?;

    tracing::info!(address = %config.bind_addr, "leos-server is listening");
    axum::serve(listener, app)
        .await
        .map_err(|e| LeosError::Internal(format!("server error: {e}")))?;

    Ok(())
}
