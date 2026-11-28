//! Slash command for Lavalink audio-filter presets.

use poise::CreateReply;

use crate::audio::filters;
use crate::error::BotError;
use crate::playback::ops::require_same_voice;
use crate::state::BotData;
use crate::ui::embeds;

/// Apply an audio filter preset to the current player.
#[poise::command(slash_command, guild_only)]
pub async fn filter(
    ctx: poise::Context<'_, BotData, BotError>,
    #[description = "Preset: bassboost, nightcore, vaporwave, 8d, karaoke, clear"]
    #[autocomplete = "crate::autocomplete::filter_preset"]
    preset: String,
) -> Result<(), BotError> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))?;
    require_same_voice(
        ctx.cache(),
        guild_id,
        ctx.author().id,
        ctx.cache().current_user().id,
    )?;

    let (label, applied) =
        filters::resolve(&preset).ok_or_else(|| BotError::InvalidQuery(preset.clone()))?;

    let data = ctx.data();
    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;
    let player = data
        .lavalink
        .get_player_context(guild_id.get())
        .ok_or(BotError::NoActivePlayer)?;
    player
        .set_filters(applied)
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;

    let session = data.guild_states.session(guild_id.get());
    let mut details = session.details.lock().await;
    details.active_filter = if label == "clear" {
        None
    } else {
        Some(label.to_owned())
    };
    drop(details);
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Filter applied",
        format!("Playback filter is now **{label}**."),
    )))
    .await?;
    Ok(())
}
