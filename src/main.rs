use std::path::Path;
use std::sync::Arc;

use wavemelo::config::AppConfig;
use wavemelo::error::BotError;

// Songbird needs Symphonia's registry to register the enabled audio codecs.
use symphonia as _;

fn init_environment() -> Result<(), BotError> {
    if Path::new(".env").exists() {
        dotenvy::dotenv()?;
    }
    Ok(())
}

fn gateway_intents() -> serenity::model::gateway::GatewayIntents {
    serenity::model::gateway::GatewayIntents::GUILDS
        | serenity::model::gateway::GatewayIntents::GUILD_VOICE_STATES
}

fn framework_options() -> poise::FrameworkOptions<(), BotError> {
    poise::FrameworkOptions::default()
}

fn http_client() -> Result<reqwest::Client, BotError> {
    Ok(reqwest::Client::builder().build()?)
}

fn voice_manager() -> Arc<songbird::Songbird> {
    songbird::Songbird::serenity()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), BotError> {
    let _config_type: Option<AppConfig> = None;
    init_environment()?;
    wavemelo::logging::init_logging()?;
    let _gateway_intents = gateway_intents();
    let _framework_options = framework_options();
    let _http_client = http_client()?;
    let _voice_manager = voice_manager();
    tracing::info!("wavemelo bootstrap ready");
    Ok(())
}
