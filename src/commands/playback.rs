use std::time::Instant;

use poise::CreateReply;

use crate::error::BotError;
use crate::state::BotData;
use crate::ui::embeds;

/// Skip the current track and play the next queued track.
#[poise::command(slash_command, guild_only)]
pub async fn skip(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let command_started = Instant::now();
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))?;
    let data = ctx.data();

    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;

    let player_ctx = data
        .lavalink
        .get_player_context(guild_id.get())
        .ok_or(BotError::QueueEmpty)?;

    // Skip advances to the next track in the built-in queue
    player_ctx
        .skip()
        .map_err(|e| BotError::Track(e.to_string()))?;
    drop(_guard);

    let reply_started = Instant::now();
    ctx.send(CreateReply::default().embed(embeds::success("ข้ามเพลงแล้ว", "กำลังเล่นเพลงถัดไปในคิว")))
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
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))?;
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
        "หยุดเล่นเพลงแล้ว",
        "ล้างคิวเพลงเรียบร้อย ใช้ /leave เพื่อออกจากห้องเสียง",
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
