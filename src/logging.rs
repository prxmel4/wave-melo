use tracing_subscriber::EnvFilter;

/// Initialize structured JSON logging controlled by `RUST_LOG`.
///
/// Returns an error message when a global subscriber was already installed.
pub fn init_logging() -> Result<(), String> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .try_init()
        .map_err(|error| error.to_string())
}
