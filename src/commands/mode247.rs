//! Toggle 24/7 mode: keep the bot connected even when idle.

use poise::CreateReply;

use crate::audio::idle;
use crate::error::BotError;
use crate::playback::ops::require_same_voice;
use crate::state::BotData;
use crate::ui::embeds;

/// Toggle 24/7 mode to keep the bot connected even when idle.
#[poise::command(slash_command, guild_only, rename = "247")]
pub async fn mode_247(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))?;
    require_same_voice(
        ctx.cache(),
        guild_id,
        ctx.author().id,
        ctx.cache().current_user().id,
    )?;

    let data = ctx.data();
    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;

    let session = data.guild_states.session(guild_id.get());
    let enabled = {
        let mut details = session.details.lock().await;
        details.mode_247 = !details.mode_247;
        details.mode_247
    };

    // Enabling cancels any armed countdown; disabling re-evaluates it.
    let deps = idle::IdleDeps::from_bot_data(data, ctx.serenity_context());
    idle::refresh(deps, guild_id.get()).await;
    drop(_guard);

    let (title, description) = if enabled {
        (
            "24/7 mode enabled",
            "The bot will stay connected even when idle or alone.",
        )
    } else {
        (
            "24/7 mode disabled",
            "The bot will leave after 180 seconds of inactivity.",
        )
    };

    ctx.send(CreateReply::default().embed(embeds::success(title, description)))
        .await?;
    Ok(())
}
