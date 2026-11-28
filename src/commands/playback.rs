use std::time::Instant;

use poise::CreateReply;
use serenity::model::id::GuildId;

use crate::audio::looping::{build_queue_cycle, loop_copy, strip_loop_markers};
use crate::error::BotError;
use crate::playback::ops::{require_same_voice, set_paused, set_volume};
use crate::state::{BotData, LoopMode};
use crate::ui::embeds;
use crate::ui::now_playing::{self, NowPlayingView};

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

/// Seek to an absolute position in the current track.
#[poise::command(slash_command, guild_only)]
pub async fn seek(
    ctx: poise::Context<'_, BotData, BotError>,
    #[description = "Position as 90, 01:30, or 00:01:30"] position: String,
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
    let player_ctx = crate::playback::ops::require_player(data, guild_id)?;
    let player = player_ctx
        .get_player()
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    let track = player.track.ok_or(BotError::NoActivePlayer)?;
    let target = crate::playback::ops::resolve_seek_input(
        &position,
        track.info.is_seekable,
        track.info.is_stream,
        track.info.length,
    )?;
    let applied = crate::playback::ops::seek_to(data, guild_id, target).await?;
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Position updated",
        format!("Seeked to {}", crate::utils::format_duration_ms(applied)),
    )))
    .await?;
    Ok(())
}

/// Show the current track, playback position, volume, loop mode, and filter.
#[poise::command(slash_command, guild_only)]
pub async fn now_playing(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = command_guild(ctx)?;
    let data = ctx.data();
    let player_ctx = crate::playback::ops::require_player(data, guild_id)?;
    let player = player_ctx
        .get_player()
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    let track = player.track.ok_or(BotError::NoActivePlayer)?;

    let requester = track
        .user_data
        .as_ref()
        .and_then(|data| data.get("requester"))
        .and_then(|value| value.as_str())
        .unwrap_or("Unknown user")
        .to_owned();
    let length_ms = (!track.info.is_stream && track.info.length > 0).then_some(track.info.length);
    let session = data.guild_states.session(guild_id.get());
    let loop_mode = match session.loop_control.mode() {
        LoopMode::Off => "Off",
        LoopMode::Track => "Track",
        LoopMode::Queue => "Queue",
    };
    let active_filter = {
        let details = session.details.lock().await;
        details
            .active_filter
            .clone()
            .unwrap_or_else(|| "None".to_owned())
    };
    let view = NowPlayingView {
        title: track.info.title.clone(),
        url: Some(crate::audio::loader::track_url(&track)),
        author: track.info.author.clone(),
        requester,
        position_ms: player.state.position,
        length_ms,
        volume: player.volume,
        loop_mode: loop_mode.to_owned(),
        active_filter,
        paused: player.paused,
        connected: player.state.connected,
    };

    ctx.send(CreateReply::default().embed(now_playing::build(&view)))
        .await?;
    Ok(())
}

/// Set the guild's repeat mode: off, track, or queue.
#[poise::command(slash_command, guild_only)]
pub async fn loop_mode(
    ctx: poise::Context<'_, BotData, BotError>,
    #[description = "Loop mode: off, track, or queue"] mode: String,
) -> Result<(), BotError> {
    let guild_id = command_guild(ctx)?;
    let parsed = LoopMode::parse(&mode).ok_or_else(|| BotError::InvalidLoopMode(mode.clone()))?;
    require_same_voice(
        ctx.cache(),
        guild_id,
        ctx.author().id,
        ctx.cache().current_user().id,
    )?;

    let data = ctx.data();
    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;
    let player_ctx = crate::playback::ops::require_player(data, guild_id)?;
    let player = player_ctx
        .get_player()
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    let current = player.track.clone();
    let queue_ref = player_ctx.get_queue();
    let mut queued = queue_ref
        .get_queue()
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;

    // Remove any injected copies left by a previous mode before applying the
    // new one. This keeps the user's real queue authoritative.
    let removed = strip_loop_markers(&mut queued);
    if removed > 0 {
        queue_ref
            .replace(queued.clone())
            .map_err(|error| BotError::Lavalink(error.to_string()))?;
    }

    let session = data.guild_states.session(guild_id.get());
    session.loop_control.set_mode(parsed);

    if current.is_none() {
        session.loop_control.set_mode(LoopMode::Off);
        return Err(BotError::NoActivePlayer);
    }

    match parsed {
        LoopMode::Off => session.loop_control.clear_snapshot(),
        LoopMode::Track => {
            let track = current.as_ref().ok_or(BotError::NoActivePlayer)?;
            // Two buffered copies cover the short window between Lavalink's
            // automatic advance and this handler's replenishment.
            queue_ref
                .push_to_front(loop_copy(track))
                .map_err(|error| BotError::Lavalink(error.to_string()))?;
            queue_ref
                .push_to_front(loop_copy(track))
                .map_err(|error| BotError::Lavalink(error.to_string()))?;
        }
        LoopMode::Queue => {
            let cycle = build_queue_cycle(current.as_ref(), &queued);
            session.loop_control.set_snapshot(cycle.clone());
            queue_ref
                .append(cycle.clone())
                .map_err(|error| BotError::Lavalink(error.to_string()))?;
            queue_ref
                .append(cycle)
                .map_err(|error| BotError::Lavalink(error.to_string()))?;
        }
    }

    let mut details = session.details.lock().await;
    details.mark_manual();
    drop(details);
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Loop mode updated",
        format!("Repeat mode is now **{}**.", parsed.as_str()),
    )))
    .await?;
    Ok(())
}
