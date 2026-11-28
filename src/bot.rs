use std::sync::Arc;

use poise::{FrameworkContext, FrameworkError};
use serenity::client::FullEvent;
use serenity::gateway::ActivityData;
use serenity::model::gateway::GatewayIntents;
use serenity::model::user::OnlineStatus;

use crate::audio::client::build_lavalink_client;
use crate::audio::events::handle_voice_event;
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

/// Minimal intents required for slash commands and voice state tracking.
pub fn gateway_intents() -> GatewayIntents {
    GatewayIntents::GUILDS | GatewayIntents::GUILD_VOICE_STATES
}

/// Handle gateway events: log readiness and forward voice events to lavalink-rs.
pub async fn event_handler(
    framework: FrameworkContext<'_, BotData, BotError>,
    event: &FullEvent,
) -> Result<(), BotError> {
    let data = framework.user_data;

    match event {
        FullEvent::Ready { data_about_bot } => {
            framework
                .serenity_context
                .set_presence(Some(ActivityData::listening("/play")), OnlineStatus::Online);
            tracing::info!(
                bot_user = %data_about_bot.user.tag(),
                guild_count = data_about_bot.guilds.len(),
                shard_id = %framework.serenity_context.shard_id,
                "wavemelo ready"
            );
        }
        FullEvent::VoiceStateUpdate { .. } | FullEvent::VoiceServerUpdate { .. } => {
            handle_voice_event(event, data, framework.bot_id()).await;
        }
        _ => {}
    }
    Ok(())
}

/// Log internal failures and reply privately without leaking details.
pub async fn on_error(error: FrameworkError<'_, BotData, BotError>) {
    let context = error.ctx();

    match &error {
        FrameworkError::Command { ctx, error, .. } => {
            tracing::error!(error = ?error, command = %ctx.command().name, "command failed");
        }
        FrameworkError::Setup { error, .. } => {
            tracing::error!(error = ?error, "framework setup failed");
        }
        FrameworkError::EventHandler { error, .. } => {
            tracing::error!(error = ?error, "event handler failed");
        }
        other => {
            tracing::error!(error = ?other, "framework error");
        }
    }

    let Some(ctx) = context else {
        return;
    };

    let message = match &error {
        FrameworkError::Command { error, .. } => error.user_message(),
        FrameworkError::GuildOnly { .. } => "This command can only be used in a server.",
        FrameworkError::MissingBotPermissions { .. } => {
            "The bot lacks the permissions required for this command."
        }
        FrameworkError::MissingUserPermissions { .. } => {
            "You lack the permissions required for this command."
        }
        FrameworkError::ArgumentParse { .. } => "Invalid command syntax or input.",
        FrameworkError::CommandPanic { .. } => {
            "An internal error occurred. Please try again later."
        }
        _ => "Something went wrong. Please try again later.",
    };

    let reply = poise::CreateReply::default()
        .content(message)
        .ephemeral(true);
    if let Err(reply_error) = ctx.send(reply).await {
        tracing::warn!(error = ?reply_error, "failed to send error reply");
    }
}

/// Construct a Serenity client with Poise registered.
pub async fn build_client(config: &AppConfig) -> Result<serenity::Client, BotError> {
    // Fetch the bot's user ID via a temporary HTTP client.
    let http = serenity::http::Http::new(&config.discord_token);
    let current_user = http
        .get_current_user()
        .await
        .map_err(|e| BotError::Discord(Box::new(e)))?;
    let user_id = current_user.id.get();

    tracing::info!(user_id = user_id, "fetched bot user ID");

    // Build the Lavalink client.
    let lavalink = build_lavalink_client(config, user_id).await?;

    let data = BotData {
        config: Arc::new(config.clone()),
        lavalink,
        guild_ops: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    };

    let setup_data = data.clone();
    let setup_guild_id = config.discord_guild_id;
    let framework = poise::Framework::builder()
        .options(framework_options())
        .setup(move |ctx, _ready, framework| {
            let data = setup_data.clone();
            let command_count = framework.options().commands.len();
            let command_names = framework
                .options()
                .commands
                .iter()
                .map(|command| command.name.as_ref())
                .collect::<Vec<_>>()
                .join(", ");

            Box::pin(async move {
                if let Some(guild_id) = setup_guild_id {
                    tracing::info!(
                        count = command_count,
                        names = %command_names,
                        guild_id = %guild_id,
                        "registering slash commands in guild"
                    );
                    poise::builtins::register_in_guild(
                        ctx,
                        &framework.options().commands,
                        guild_id,
                    )
                    .await?;
                } else {
                    tracing::info!(
                        count = command_count,
                        names = %command_names,
                        "registering slash commands globally"
                    );
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                }

                tracing::info!(
                    count = command_count,
                    names = %command_names,
                    "slash commands registered"
                );
                Ok(data)
            })
        })
        .build();

    let client =
        serenity::client::ClientBuilder::new(config.discord_token.clone(), gateway_intents())
            .framework(framework)
            .await?;
    Ok(client)
}
