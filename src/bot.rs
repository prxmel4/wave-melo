use std::sync::Arc;

use poise::{FrameworkContext, FrameworkError};
use serenity::client::FullEvent;
use serenity::model::gateway::GatewayIntents;
use songbird::SerenityInit;

use crate::config::AppConfig;
use crate::error::BotError;
use crate::state::BotData;

/// Build Poise framework options for the bot.
pub fn framework_options() -> poise::FrameworkOptions<BotData, BotError> {
    poise::FrameworkOptions {
        commands: crate::commands::commands(),
        on_error: |error| Box::pin(on_error(error)),
        event_handler: |ctx, event| Box::pin(event_handler(ctx, event)),
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

/// Log safe startup diagnostics when the gateway becomes ready.
pub async fn event_handler(
    framework: FrameworkContext<'_, BotData, BotError>,
    event: &FullEvent,
) -> Result<(), BotError> {
    if let FullEvent::Ready { data_about_bot } = event {
        tracing::info!(
            bot_user = %data_about_bot.user.tag(),
            guild_count = data_about_bot.guilds.len(),
            shard_id = %framework.serenity_context.shard_id,
            "wavemelo ready"
        );
    }
    Ok(())
}

/// Log internal failures and reply privately without leaking details.
pub async fn on_error(error: FrameworkError<'_, BotData, BotError>) {
    match error {
        FrameworkError::Command { ctx, error, .. } => {
            tracing::error!(error = ?error, command = %ctx.command().name, "command failed");
            let reply = poise::CreateReply::default()
                .content(error.user_message())
                .ephemeral(true);
            if let Err(reply_error) = ctx.send(reply).await {
                tracing::warn!(error = ?reply_error, "failed to send error reply");
            }
        }
        FrameworkError::Setup { error, .. } => {
            tracing::error!(error = ?error, "framework setup failed");
        }
        FrameworkError::EventHandler { error, .. } => {
            tracing::error!(error = ?error, "event handler failed");
        }
        other => {
            tracing::error!(error = ?other, "unhandled framework error");
        }
    }
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
