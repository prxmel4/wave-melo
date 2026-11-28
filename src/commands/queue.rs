use std::collections::VecDeque;

use lavalink_rs::model::track::TrackData;
use poise::CreateReply;

use crate::audio::loader::{track_duration, track_url};
use crate::error::BotError;
use crate::playback::ops::require_same_voice;
use crate::queue::{jump_plan, move_indices, reordered, zero_based};
use crate::state::BotData;
use crate::ui::embeds::{self, QueueEntryView};

const PAGE_SIZE: usize = 10;

/// Owned display data for one queue row.
type Row = (String, String, String, Option<std::time::Duration>);

fn requester_of(track: &TrackData) -> String {
    track
        .user_data
        .as_ref()
        .and_then(|data| data.get("requester"))
        .and_then(|value| value.as_str())
        .unwrap_or("Unknown user")
        .to_owned()
}

fn row_of(track: &TrackData) -> Row {
    (
        track.info.title.clone(),
        track_url(track),
        requester_of(track),
        track_duration(track),
    )
}

/// Show the current track and the next queued tracks.
#[poise::command(slash_command, guild_only)]
pub async fn queue(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))?;

    let data = ctx.data();
    let (current, queued) = match data.lavalink.get_player_context(guild_id.get()) {
        Some(player_ctx) => {
            let player = player_ctx
                .get_player()
                .await
                .map_err(|e| BotError::Lavalink(e.to_string()))?;
            let queued = player_ctx
                .get_queue()
                .get_queue()
                .await
                .map_err(|e| BotError::Lavalink(e.to_string()))?;
            (player.track, queued)
        }
        None => (None, VecDeque::new()),
    };

    let current_row = current.as_ref().map(row_of);
    let queued_rows: Vec<Row> = queued
        .iter()
        .take(PAGE_SIZE)
        .map(|item| row_of(&item.track))
        .collect();

    let current_view = current_row
        .as_ref()
        .map(|(title, url, requester, duration)| QueueEntryView {
            title,
            url: Some(url),
            requester,
            duration: *duration,
        });
    let views: Vec<QueueEntryView<'_>> = queued_rows
        .iter()
        .map(|(title, url, requester, duration)| QueueEntryView {
            title,
            url: Some(url),
            requester,
            duration: *duration,
        })
        .collect();

    ctx.send(CreateReply::default().embed(embeds::queue(
        current_view.as_ref(),
        &views,
        queued.len(),
    )))
    .await?;
    Ok(())
}

fn command_guild(
    ctx: poise::Context<'_, BotData, BotError>,
) -> Result<serenity::model::id::GuildId, BotError> {
    ctx.guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))
}

/// Remove one queued track by its 1-based position.
#[poise::command(slash_command, guild_only)]
pub async fn queueremove(
    ctx: poise::Context<'_, BotData, BotError>,
    #[description = "Queue position to remove, starting at 1"] index: u16,
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
    let player_ctx = data
        .lavalink
        .get_player_context(guild_id.get())
        .ok_or(BotError::QueueEmpty)?;
    let queue_ref = player_ctx.get_queue();
    let queued = queue_ref
        .get_queue()
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    let zero_based = zero_based(usize::from(index), queued.len())?;
    queue_ref
        .remove(zero_based)
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Track removed",
        format!("Removed queue entry #{}.", zero_based + 1),
    )))
    .await?;
    Ok(())
}

/// Play a queued track immediately by its 1-based position.
#[poise::command(slash_command, guild_only)]
pub async fn queuejump(
    ctx: poise::Context<'_, BotData, BotError>,
    #[description = "Queue position to play next, starting at 1"] index: u16,
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
    let player_ctx = data
        .lavalink
        .get_player_context(guild_id.get())
        .ok_or(BotError::QueueEmpty)?;
    let queue_ref = player_ctx.get_queue();
    let queued = queue_ref
        .get_queue()
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    let (target, rest) = jump_plan(queued, usize::from(index))?;
    let title = target.track.info.title.clone();
    queue_ref
        .replace(rest)
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    queue_ref
        .push_to_front(target)
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    player_ctx
        .skip()
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Track moved to front",
        format!("Now playing **{}**.", crate::utils::truncate(&title, 80)),
    )))
    .await?;
    Ok(())
}

/// Move a queued track from one 1-based position to another.
#[poise::command(slash_command, guild_only)]
pub async fn queuemove(
    ctx: poise::Context<'_, BotData, BotError>,
    #[description = "Current queue position, starting at 1"] from: u16,
    #[description = "New queue position, starting at 1"] to: u16,
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
    let player_ctx = data
        .lavalink
        .get_player_context(guild_id.get())
        .ok_or(BotError::QueueEmpty)?;
    let queue_ref = player_ctx.get_queue();
    let queued = queue_ref
        .get_queue()
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    let (from_index, to_index) = move_indices(queued.len(), usize::from(from), usize::from(to))?;
    queue_ref
        .replace(reordered(queued, from_index, to_index))
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Queue reordered",
        format!("Moved entry #{from} to #{to}."),
    )))
    .await?;
    Ok(())
}

/// Clear all queued tracks without disconnecting from voice.
#[poise::command(slash_command, guild_only)]
pub async fn queueclear(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
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
    if let Some(player_ctx) = data.lavalink.get_player_context(guild_id.get()) {
        player_ctx
            .get_queue()
            .clear()
            .map_err(|error| BotError::Lavalink(error.to_string()))?;
    }
    data.guild_states
        .session(guild_id.get())
        .loop_control
        .clear_snapshot();
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Queue cleared",
        "All queued tracks were removed. The current track keeps playing.",
    )))
    .await?;
    Ok(())
}
