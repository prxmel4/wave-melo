use tracing_subscriber::EnvFilter;

use crate::error::BotError;

/// Initialize structured JSON logging controlled by `RUST_LOG`.
///
/// Returns an error when a global subscriber was already installed.
pub fn init_logging() -> Result<(), BotError> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .try_init()
        .map_err(|error| BotError::Logging(error.to_string()))
}
