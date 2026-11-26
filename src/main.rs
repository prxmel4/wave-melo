use std::path::Path;

use thiserror::Error;

#[derive(Debug, Error)]
enum StartupError {
    #[error("failed to load the environment file")]
    Environment(#[from] dotenvy::Error),
    #[error("failed to initialize logging: {0}")]
    Logging(String),
}

fn init_environment() -> Result<(), StartupError> {
    if Path::new(".env").exists() {
        dotenvy::dotenv()?;
    }
    Ok(())
}

fn init_logging() -> Result<(), StartupError> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .map_err(|error| StartupError::Logging(error.to_string()))?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), StartupError> {
    init_environment()?;
    init_logging()?;
    tracing::info!("wavemelo bootstrap ready");
    Ok(())
}
