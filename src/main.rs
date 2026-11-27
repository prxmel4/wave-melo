use std::path::Path;

use wavemelo::config::AppConfig;
use wavemelo::error::BotError;
use wavemelo::state::BotData;

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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), BotError> {
    let _config_type: Option<AppConfig> = None;
    let _bot_data_type: Option<BotData> = None;
    init_environment()?;
    wavemelo::logging::init_logging()?;
    let _gateway_intents = gateway_intents();
    let _framework_options = framework_options();
    tracing::info!("wavemelo bootstrap ready");
    Ok(())
}
