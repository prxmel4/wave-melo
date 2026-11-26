use std::path::Path;
use std::sync::Arc;

use thiserror::Error;

// Songbird needs Symphonia's registry to register the enabled audio codecs.
use symphonia as _;

#[derive(Debug, Error)]
enum StartupError {
    #[error("failed to load the environment file")]
    Environment(#[from] dotenvy::Error),
    #[error("failed to build the HTTP client")]
    HttpClient(#[from] reqwest::Error),
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

fn gateway_intents() -> serenity::model::gateway::GatewayIntents {
    serenity::model::gateway::GatewayIntents::GUILDS
        | serenity::model::gateway::GatewayIntents::GUILD_VOICE_STATES
}

fn framework_options() -> poise::FrameworkOptions<(), StartupError> {
    poise::FrameworkOptions::default()
}

fn http_client() -> Result<reqwest::Client, StartupError> {
    Ok(reqwest::Client::builder().build()?)
}

fn voice_manager() -> Arc<songbird::Songbird> {
    songbird::Songbird::serenity()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), StartupError> {
    init_environment()?;
    init_logging()?;
    let _gateway_intents = gateway_intents();
    let _framework_options = framework_options();
    let _http_client = http_client()?;
    let _voice_manager = voice_manager();
    tracing::info!("wavemelo bootstrap ready");
    Ok(())
}
