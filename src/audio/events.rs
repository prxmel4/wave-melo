use std::time::Instant;

use async_trait::async_trait;
use serenity::model::id::GuildId;
use songbird::events::{Event, EventContext, EventHandler, TrackEvent};
use songbird::tracks::{PlayMode, TrackHandle};

use crate::audio::cache::CacheKey;
use crate::audio::prefetch::{invalidate_prefetch, schedule_next, take_prefetched};
use crate::audio::query::classify;
use crate::audio::queue::{peek_front, pop_front, take_current};
use crate::audio::source::{build_input, resolve_metadata};
use crate::error::BotError;
use crate::state::{BotData, CurrentTrack, QueuedTrack};

#[derive(Debug, Clone, Copy)]
enum LifecycleKind {
    Start,
    End,
    Error,
}

struct TrackLifecycle {
    guild_id: GuildId,
    data: BotData,
    kind: LifecycleKind,
    generation: u64,
}

#[async_trait]
impl EventHandler for TrackLifecycle {
    async fn act(&self, context: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(tracks) = context {
            for (state, _handle) in *tracks {
                match self.kind {
                    LifecycleKind::Start => {
                        tracing::info!(guild_id = %self.guild_id, "track started");
                    }
                    LifecycleKind::End => {
                        tracing::info!(guild_id = %self.guild_id, "track ended");
                        if let Err(error) =
                            advance(&self.data, self.guild_id, self.generation, false, "end").await
                        {
                            tracing::debug!(guild_id = %self.guild_id, error = ?error, "queue advance failed");
                        }
                    }
                    LifecycleKind::Error => {
                        if let PlayMode::Errored(error) = &state.playing {
                            tracing::warn!(guild_id = %self.guild_id, error = ?error, "track errored");
                        } else {
                            tracing::warn!(guild_id = %self.guild_id, "track errored");
                        }
                        if let Err(error) =
                            advance(&self.data, self.guild_id, self.generation, false, "error")
                                .await
                        {
                            tracing::debug!(guild_id = %self.guild_id, error = ?error, "queue advance after error failed");
                        }
                    }
                }
            }
        }
        None
    }
}

/// Attach start, end, and error logging to a playing track.
pub fn attach_lifecycle_events(
    handle: &TrackHandle,
    guild_id: GuildId,
    data: &BotData,
    generation: u64,
) -> Result<(), BotError> {
    for (event, kind) in [
        (Event::Track(TrackEvent::Playable), LifecycleKind::Start),
        (Event::Track(TrackEvent::End), LifecycleKind::End),
        (Event::Track(TrackEvent::Error), LifecycleKind::Error),
    ] {
        handle
            .add_event(
                event,
                TrackLifecycle {
                    guild_id,
                    data: data.clone(),
                    kind,
                    generation,
                },
            )
            .map_err(|error| BotError::Track(error.to_string()))?;
    }
    Ok(())
}

/// Advance the queue while the caller holds the guild operation lock.
pub(crate) async fn advance_locked(
    data: &BotData,
    guild_id: GuildId,
    expected_generation: u64,
    stop_current: bool,
    reason: &'static str,
) -> Result<bool, BotError> {
    let transition_started = Instant::now();

    if data.current_generation(guild_id).await != expected_generation {
        tracing::debug!(
            command = "transition",
            guild_id = %guild_id,
            outcome = "stale_event",
            total_ms = transition_started.elapsed().as_millis() as u64,
            "ignored stale transition"
        );
        return Ok(false);
    }

    if let Some(current) = take_current(data, guild_id).await {
        if stop_current && current.handle.stop().is_err() {
            tracing::debug!(guild_id = %guild_id, "stopping previous track failed");
        }
    }

    let generation = data.bump_generation(guild_id).await;
    let version = data.current_queue_version(guild_id).await;
    let Some(queued) = peek_front(data, guild_id).await else {
        invalidate_prefetch(data, guild_id).await;
        tracing::info!(
            command = "transition",
            guild_id = %guild_id,
            reason,
            outcome = "queue_empty",
            total_ms = transition_started.elapsed().as_millis() as u64,
            "queue transition finished"
        );
        return Ok(false);
    };

    let query = classify(&queued.url)?;
    let key = CacheKey::new(
        query.as_str(),
        data.config.youtube_cookies_path.as_deref().unwrap_or(""),
    );
    let cache_hit = data.metadata_cache.get_fresh(&key).await.is_some();

    let resolve_started = Instant::now();
    let prefetched = take_prefetched(data, guild_id, &key, version).await;
    let prefetch_hit = prefetched.is_some();
    let resolved = match prefetched {
        Some(metadata) => metadata,
        None => resolve_metadata(data, &query).await?,
    };
    let resolve_ms = resolve_started.elapsed();

    let _ = pop_front(data, guild_id).await;

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

    attach_lifecycle_events(&handle, guild_id, data, generation)?;

    let current = CurrentTrack {
        handle,
        metadata: QueuedTrack {
            title: resolved.title,
            url: resolved.url,
            requester: queued.requester,
            duration: resolved.duration,
        },
    };
    data.current.lock().await.insert(guild_id, current);
    schedule_next(data, guild_id).await;

    tracing::info!(
        command = "transition",
        guild_id = %guild_id,
        reason,
        cache_hit,
        prefetch_hit,
        voice_reused = true,
        resolve_ms = resolve_ms.as_millis() as u64,
        playback_start_ms = playback_start_ms.as_millis() as u64,
        transition_gap_ms = transition_started.elapsed().as_millis() as u64,
        total_ms = transition_started.elapsed().as_millis() as u64,
        outcome = "started",
        "queue transition"
    );
    Ok(true)
}

/// Advance the queue with per-guild serialization.
pub async fn advance(
    data: &BotData,
    guild_id: GuildId,
    expected_generation: u64,
    stop_current: bool,
    reason: &'static str,
) -> Result<bool, BotError> {
    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;
    advance_locked(data, guild_id, expected_generation, stop_current, reason).await
}

/// Pop and play the next queued track for a guild.
pub async fn play_next(data: &BotData, guild_id: GuildId) -> Result<bool, BotError> {
    let generation = data.current_generation(guild_id).await;
    advance(data, guild_id, generation, false, "end").await
}
