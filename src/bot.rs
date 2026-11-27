use std::sync::Arc;

use serenity::model::gateway::GatewayIntents;
use songbird::SerenityInit;

use crate::config::AppConfig;
use crate::error::BotError;
use crate::state::BotData;

/// Build Poise framework options for the bot.
pub fn framework_options() -> poise::FrameworkOptions<BotData, BotError> {
    poise::FrameworkOptions {
        commands: Vec::new(),
        ..Default::default()
    }
}

/// Minimal intents required for slash commands and Songbird voice state tracking.
pub fn gateway_intents() -> GatewayIntents {
    GatewayIntents::GUILDS | GatewayIntents::GUILD_VOICE_STATES
}

/// Create the shared Songbird manager used by the Serenity client.
pub fn voice_manager() -> Arc<songbird::Songbird> {
    songbird::Songbird::serenity()
}

/// Construct a Serenity client with Poise and Songbird registered.
pub async fn build_client(config: &AppConfig, data: BotData) -> Result<serenity::Client, BotError> {
    let setup_data = data.clone();
    let framework = poise::Framework::builder()
        .options(framework_options())
        .setup(move |_ctx, _ready, _framework| {
            let data = setup_data.clone();
            Box::pin(async move { Ok(data) })
        })
        .build();

    let client =
        serenity::client::ClientBuilder::new(config.discord_token.clone(), gateway_intents())
            .register_songbird_with(data.voice.clone())
            .framework(framework)
            .await?;
    Ok(client)
}
