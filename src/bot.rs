use std::sync::Arc;

use poise::{FrameworkContext, FrameworkError};
use serenity::client::FullEvent;
use serenity::gateway::ActivityData;
use serenity::model::gateway::GatewayIntents;
use serenity::model::user::OnlineStatus;
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
        FrameworkError::GuildOnly { .. } => "คำสั่งนี้ใช้ได้เฉพาะในเซิร์ฟเวอร์",
        FrameworkError::MissingBotPermissions { .. } => "บอทไม่มีสิทธิ์ที่จำเป็นสำหรับคำสั่งนี้",
        FrameworkError::MissingUserPermissions { .. } => "คุณไม่มีสิทธิ์ที่จำเป็นสำหรับคำสั่งนี้",
        FrameworkError::ArgumentParse { .. } => "รูปแบบคำสั่งหรือข้อมูลที่ป้อนไม่ถูกต้อง",
        FrameworkError::CommandPanic { .. } => "เกิดข้อผิดพลาดภายใน กรุณาลองใหม่ในภายหลัง",
        _ => "เกิดข้อผิดพลาด กรุณาลองใหม่ในภายหลัง",
    };

    let reply = poise::CreateReply::default()
        .content(message)
        .ephemeral(true);
    if let Err(reply_error) = ctx.send(reply).await {
        tracing::warn!(error = ?reply_error, "failed to send error reply");
    }
}

/// Construct a Serenity client with Poise and Songbird registered.
pub async fn build_client(config: &AppConfig, data: BotData) -> Result<serenity::Client, BotError> {
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
            .register_songbird_with(data.voice.clone())
            .framework(framework)
            .await?;
    Ok(client)
}
