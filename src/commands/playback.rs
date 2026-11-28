use std::time::Instant;

use poise::CreateReply;
use serenity::model::id::GuildId;

use crate::error::BotError;
use crate::playback::ops::{require_same_voice, set_paused, set_volume};
use crate::state::BotData;
use crate::ui::embeds;

fn command_guild(ctx: poise::Context<'_, BotData, BotError>) -> Result<GuildId, BotError> {
    ctx.guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))
}

/// Skip the current track and play the next queued track.
#[poise::command(slash_command, guild_only)]
pub async fn skip(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let command_started = Instant::now();
    let guild_id = command_guild(ctx)?;
    let data = ctx.data();

    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;

    let player_ctx = data
        .lavalink
        .get_player_context(guild_id.get())
        .ok_or(BotError::QueueEmpty)?;

    // Skip advances to the next track in the built-in queue.
    player_ctx
        .skip()
        .map_err(|e| BotError::Track(e.to_string()))?;
    drop(_guard);

    let reply_started = Instant::now();
    ctx.send(CreateReply::default().embed(embeds::success(
        "Track skipped",
        "Now playing the next track in the queue.",
    )))
    .await?;
    let reply_ms = reply_started.elapsed();

    tracing::info!(
        command = "skip",
        guild_id = %guild_id,
        reply_ms = reply_ms.as_millis() as u64,
        total_ms = command_started.elapsed().as_millis() as u64,
        outcome = "skipped",
        "skip command completed"
    );
    Ok(())
}

/// Stop playback and clear the queue, staying in the voice channel.
#[poise::command(slash_command, guild_only)]
pub async fn stop(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let command_started = Instant::now();
    let guild_id = command_guild(ctx)?;
    let data = ctx.data();

    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;

    if let Some(player_ctx) = data.lavalink.get_player_context(guild_id.get()) {
        // Clear the queue first to prevent auto-advance, then stop the track.
        let queue_ref = player_ctx.get_queue();
        let _ = queue_ref.clear();
        if let Err(error) = player_ctx.stop_now().await {
            tracing::warn!(guild_id = %guild_id, error = %error, "failed to stop playback");
        }
    }
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Playback stopped",
        "Queue cleared. Use /leave to disconnect from the voice channel.",
    )))
    .await?;

    tracing::info!(
        command = "stop",
        guild_id = %guild_id,
        total_ms = command_started.elapsed().as_millis() as u64,
        outcome = "stopped",
        "stop command completed"
    );
    Ok(())
}

/// Set the player volume (0 muted, 100 default, 150 maximum).
#[poise::command(slash_command, guild_only)]
pub async fn volume(
    ctx: poise::Context<'_, BotData, BotError>,
    #[description = "Volume level from 0 to 150 (100 = default)"] level: u16,
) -> Result<(), BotError> {
    let guild_id = command_guild(ctx)?;
    require_same_voice(
        ctx.cache(),
        guild_id,
        ctx.author().id,
        ctx.cache().current_user().id,
    )?;

    let data = ctx.data();
    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;
    let applied = set_volume(data, guild_id, level).await?;
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Volume updated",
        format!("Playback volume is now **{applied}** (100 = default)."),
    )))
    .await?;
    Ok(())
}

/// Pause the current track.
#[poise::command(slash_command, guild_only)]
pub async fn pause(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = command_guild(ctx)?;
    require_same_voice(
        ctx.cache(),
        guild_id,
        ctx.author().id,
        ctx.cache().current_user().id,
    )?;

    let data = ctx.data();
    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;
    let changed = set_paused(data, guild_id, true).await?;
    drop(_guard);

    let (title, description) = if changed {
        ("Playback paused", "The current track is paused.")
    } else {
        ("Already paused", "The player is already paused.")
    };
    ctx.send(CreateReply::default().embed(embeds::info(title, description)))
        .await?;
    Ok(())
}

/// Resume a paused track.
#[poise::command(slash_command, guild_only)]
pub async fn resume(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = command_guild(ctx)?;
    require_same_voice(
        ctx.cache(),
        guild_id,
        ctx.author().id,
        ctx.cache().current_user().id,
    )?;

    let data = ctx.data();
    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;
    let changed = set_paused(data, guild_id, false).await?;
    drop(_guard);

    let (title, description) = if changed {
        ("Playback resumed", "The current track is playing again.")
    } else {
        ("Already playing", "The player is not paused.")
    };
    ctx.send(CreateReply::default().embed(embeds::info(title, description)))
        .await?;
    Ok(())
}
