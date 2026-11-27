use std::time::{Duration, Instant};

use poise::CreateReply;

use crate::audio::cache::CacheKey;
use crate::audio::events::attach_lifecycle_events;
use crate::audio::prefetch::schedule_next;
use crate::audio::query::classify;
use crate::audio::queue::{enqueue, is_idle};
use crate::audio::source::{build_input, resolve_metadata};
use crate::audio::voice::{author_voice_channel, channel_is_cached};
use crate::error::BotError;
use crate::state::{BotData, CurrentTrack, QueuedTrack};
use crate::ui::embeds;

/// Play a URL or search a YouTube query; queue it if something is playing.
#[poise::command(slash_command, guild_only)]
pub async fn play(
    ctx: poise::Context<'_, BotData, BotError>,
    #[description = "YouTube URL or search query"] query: String,
) -> Result<(), BotError> {
    let command_started = Instant::now();
    ctx.defer().await?;
    let defer_ms = command_started.elapsed();

    let (guild_id, channel_id) = author_voice_channel(ctx)?;
    if !channel_is_cached(ctx.cache(), guild_id, channel_id) {
        return Err(BotError::Voice("voice channel is not available".to_owned()));
    }

    let query = classify(&query)?;
    let data = ctx.data();
    let requester = ctx.author().name.clone();

    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;

    let key = CacheKey::new(
        query.as_str(),
        data.config.youtube_cookies_path.as_deref().unwrap_or(""),
    );
    let cache_hit = data.metadata_cache.get_fresh(&key).await.is_some();

    let join_started = Instant::now();
    let resolve_started = Instant::now();
    let needs_join = match data.voice.get(guild_id) {
        Some(call) => call.lock().await.current_channel() != Some(channel_id.into()),
        None => true,
    };

    let (resolved, join_ms, resolve_ms, voice_reused) = if needs_join {
        let join_future = async {
            let result = data.voice.join(guild_id, channel_id).await;
            (result, join_started.elapsed())
        };
        let resolve_future = async {
            let result = resolve_metadata(data, &query).await;
            (result, resolve_started.elapsed())
        };

        let (join_result, resolve_result) = tokio::join!(join_future, resolve_future);
        let (join_outcome, measured_join_ms) = join_result;
        let (resolve_outcome, measured_resolve_ms) = resolve_result;

        if let Err(error) = join_outcome {
            return Err(BotError::Voice(error.to_string()));
        }

        match resolve_outcome {
            Ok(metadata) => (metadata, measured_join_ms, measured_resolve_ms, false),
            Err(error) => {
                if let Err(leave_error) = data.voice.leave(guild_id).await {
                    tracing::debug!(error = ?leave_error, "cleanup after failed resolution failed");
                }
                return Err(error);
            }
        }
    } else {
        let resolved = resolve_metadata(data, &query).await?;
        (resolved, Duration::ZERO, resolve_started.elapsed(), true)
    };

    if !is_idle(data, guild_id).await {
        let queued = QueuedTrack {
            title: resolved.title.clone(),
            url: resolved.url.clone(),
            requester: requester.clone(),
            duration: resolved.duration,
        };
        let enqueue_started = Instant::now();
        enqueue(data, guild_id, queued).await;
        schedule_next(data, guild_id).await;
        let enqueue_ms = enqueue_started.elapsed();

        let reply_started = Instant::now();
        ctx.send(CreateReply::default().embed(embeds::success(
            "เพิ่มเพลงเข้าคิวแล้ว",
            format!("**{}** ถูกเพิ่มเข้าคิว", resolved.title),
        )))
        .await?;
        let reply_ms = reply_started.elapsed();

        tracing::info!(
            command = "play",
            guild_id = %guild_id,
            cache_hit,
            prefetch_hit = false,
            voice_reused,
            defer_ms = defer_ms.as_millis() as u64,
            join_ms = join_ms.as_millis() as u64,
            resolve_ms = resolve_ms.as_millis() as u64,
            enqueue_ms = enqueue_ms.as_millis() as u64,
            reply_ms = reply_ms.as_millis() as u64,
            total_ms = command_started.elapsed().as_millis() as u64,
            outcome = "queued",
            "play command completed"
        );
        return Ok(());
    }

    let input = build_input(data, &query);
    let call = data
        .voice
        .get(guild_id)
        .ok_or_else(|| BotError::Voice("no active voice call".to_owned()))?;

    let play_started = Instant::now();
    let handle = {
        let mut guard = call.lock().await;
        guard.play_input(input)
    };
    let playback_start_ms = play_started.elapsed();

    let generation = data.bump_generation(guild_id).await;
    attach_lifecycle_events(&handle, guild_id, data, generation)?;

    let current = CurrentTrack {
        handle,
        metadata: QueuedTrack {
            title: resolved.title.clone(),
            url: resolved.url.clone(),
            requester: requester.clone(),
            duration: resolved.duration,
        },
    };
    data.current.lock().await.insert(guild_id, current);
    schedule_next(data, guild_id).await;

    let embed = embeds::now_playing(
        &resolved.title,
        Some(&resolved.url),
        &requester,
        resolved.duration,
        None,
    );

    let reply_started = Instant::now();
    ctx.send(CreateReply::default().embed(embed)).await?;
    let reply_ms = reply_started.elapsed();

    tracing::info!(
        command = "play",
        guild_id = %guild_id,
        cache_hit,
        prefetch_hit = false,
        voice_reused,
        defer_ms = defer_ms.as_millis() as u64,
        join_ms = join_ms.as_millis() as u64,
        resolve_ms = resolve_ms.as_millis() as u64,
        playback_start_ms = playback_start_ms.as_millis() as u64,
        reply_ms = reply_ms.as_millis() as u64,
        total_ms = command_started.elapsed().as_millis() as u64,
        outcome = "playing",
        "play command completed"
    );
    Ok(())
}
