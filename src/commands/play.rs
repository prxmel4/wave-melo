use std::time::Instant;

use poise::CreateReply;

use crate::audio::loader::{load_tracks, track_duration, track_url, LoadedTracks};
use crate::audio::query::classify;
use crate::audio::voice::{channel_is_cached, current_voice_channel, ensure_player_context};
use crate::error::BotError;
use crate::state::BotData;
use crate::ui::embeds;

/// Maximum tracks allowed in the queue per guild.
const MAX_QUEUE_SIZE: usize = 200;

/// Play a URL or search a YouTube query; queue it if something is playing.
#[poise::command(slash_command, guild_only)]
pub async fn play(
    ctx: poise::Context<'_, BotData, BotError>,
    #[description = "YouTube URL or search query"] query: String,
) -> Result<(), BotError> {
    let command_started = Instant::now();
    ctx.defer().await?;

    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))?;
    let channel_id = current_voice_channel(ctx.cache(), guild_id, ctx.author().id)
        .ok_or(BotError::NotInVoice)?;
    if !channel_is_cached(ctx.cache(), guild_id, channel_id) {
        return Err(BotError::Voice("voice channel is not available".to_owned()));
    }

    let query = classify(&query)?;
    let data = ctx.data();
    let requester = ctx.author().name.clone();

    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;

    // Ensure a player context exists (joins voice if needed).
    let player_ctx =
        ensure_player_context(ctx.serenity_context(), data, guild_id, channel_id).await?;

    // Load tracks from Lavalink.
    let resolve_started = Instant::now();
    let loaded = load_tracks(&data.lavalink, guild_id.get(), &query).await?;
    let resolve_ms = resolve_started.elapsed();

    match loaded {
        LoadedTracks::Single(track) | LoadedTracks::SearchResult(track) => {
            let title = track.info.title.clone();
            let url = track_url(&track);
            let duration = track_duration(&track);

            // Attach requester metadata.
            let mut track_with_data = track;
            track_with_data.user_data = Some(serde_json::json!({ "requester": requester }));

            // Check if player is idle.
            let player = player_ctx
                .get_player()
                .await
                .map_err(|e| BotError::Lavalink(e.to_string()))?;
            let is_idle = player.track.is_none();

            if is_idle {
                player_ctx
                    .play_now(&track_with_data)
                    .await
                    .map_err(|e| BotError::Lavalink(e.to_string()))?;
                let embed = embeds::now_playing(&title, Some(&url), &requester, duration, None);
                ctx.send(CreateReply::default().embed(embed)).await?;
            } else {
                let queue_ref = player_ctx.get_queue();
                let count = queue_ref
                    .get_count()
                    .await
                    .map_err(|e| BotError::Lavalink(e.to_string()))?;
                if count >= MAX_QUEUE_SIZE {
                    ctx.send(CreateReply::default().embed(embeds::error(
                        "Queue is full",
                        format!("The queue is limited to {MAX_QUEUE_SIZE} tracks."),
                    )))
                    .await?;
                    return Ok(());
                }
                queue_ref
                    .push_to_back(track_with_data)
                    .map_err(|e| BotError::Lavalink(e.to_string()))?;
                ctx.send(CreateReply::default().embed(embeds::success(
                    "Track added to queue",
                    format!("**{title}** was added to the queue"),
                )))
                .await?;
            }

            tracing::info!(
                command = "play",
                guild_id = %guild_id,
                resolve_ms = resolve_ms.as_millis() as u64,
                total_ms = command_started.elapsed().as_millis() as u64,
                outcome = if is_idle { "playing" } else { "queued" },
                "play command completed"
            );
        }
        LoadedTracks::Playlist { meta, tracks } => {
            let total = tracks.len();
            let queue_ref = player_ctx.get_queue();
            let existing = queue_ref
                .get_count()
                .await
                .map_err(|e| BotError::Lavalink(e.to_string()))?;
            let capacity = MAX_QUEUE_SIZE.saturating_sub(existing);

            let mut added = 0usize;
            for track in tracks.into_iter().take(capacity) {
                let mut track_with_data = track;
                track_with_data.user_data = Some(serde_json::json!({ "requester": requester }));
                if queue_ref.push_to_back(track_with_data).is_ok() {
                    added += 1;
                }
            }

            // If nothing was playing, start playback from the queue.
            let player = player_ctx
                .get_player()
                .await
                .map_err(|e| BotError::Lavalink(e.to_string()))?;
            if player.track.is_none() {
                let _ = player_ctx.skip();
            }

            let description = if added < total {
                format!("Added **{added}** of {total} tracks from **{}**", meta.name)
            } else {
                format!("Added {added} tracks from **{}**", meta.name)
            };

            ctx.send(CreateReply::default().embed(embeds::success("Playlist added", description)))
                .await?;

            tracing::info!(
                command = "play",
                guild_id = %guild_id,
                playlist_name = %meta.name,
                tracks_total = total,
                tracks_added = added,
                resolve_ms = resolve_ms.as_millis() as u64,
                total_ms = command_started.elapsed().as_millis() as u64,
                outcome = "playlist",
                "play command completed"
            );
        }
        LoadedTracks::Empty => {
            ctx.send(CreateReply::default().embed(embeds::error(
                "No results found",
                "No tracks matched your search.",
            )))
            .await?;
        }
    }

    Ok(())
}
