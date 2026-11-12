//! The Leos wallet server.

use leos_core::error::LeosError;
use leos_server::config::Config;
use leos_server::error::ServerError;

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "leos_server=info,leos_core=info".into()),
        )
        .init();

    let config = Config::from_env().map_err(LeosError::Internal)?;
    leos_server::serve(config).await
}
