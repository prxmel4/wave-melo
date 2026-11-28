use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::error::BotError;

/// Initialize structured JSON logging with environment-based filtering.
///
/// Respects the `RUST_LOG` environment variable for filter directives.
/// Defaults to `info,wavemelo=debug` if not set.
pub fn init_logging() -> Result<(), BotError> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,wavemelo=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().json().with_target(true).with_level(true))
        .try_init()
        .map_err(|e| BotError::Logging(e.to_string()))?;

    Ok(())
}
