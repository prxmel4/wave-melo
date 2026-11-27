use async_trait::async_trait;
use serenity::model::id::GuildId;
use songbird::events::{Event, EventContext, EventHandler, TrackEvent};
use songbird::tracks::{PlayMode, TrackHandle};

use crate::audio::query::classify;
use crate::audio::source::build_source;
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
                        if let Err(error) = play_next(&self.data, self.guild_id).await {
                            tracing::warn!(error = ?error, "failed to advance queue");
                        }
                    }
                    LifecycleKind::Error => {
                        if let PlayMode::Errored(error) = &state.playing {
                            tracing::warn!(guild_id = %self.guild_id, error = ?error, "track errored");
                        } else {
                            tracing::warn!(guild_id = %self.guild_id, "track errored");
                        }
                        if let Err(error) = play_next(&self.data, self.guild_id).await {
                            tracing::warn!(error = ?error, "failed to advance queue after error");
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
                },
            )
            .map_err(|error| BotError::Track(error.to_string()))?;
    }
    Ok(())
}

/// Pop and play the next queued track for a guild.
///
/// Returns `false` when the queue was empty.
pub async fn play_next(data: &BotData, guild_id: GuildId) -> Result<bool, BotError> {
    let queued = {
        let mut queues = data.queues.lock().await;
        queues
            .get_mut(&guild_id)
            .and_then(|queue| queue.pop_front())
    };

    let Some(queued) = queued else {
        data.current.lock().await.remove(&guild_id);
        return Ok(false);
    };

    let query = classify(&queued.url)?;
    let (input, metadata) = build_source(
        data.http_client.clone(),
        data.ytdlp_binary,
        &query,
        &queued.requester,
        data.config.youtube_cookies_path.as_deref(),
    )
    .await?;

    let call = data
        .voice
        .get(guild_id)
        .ok_or_else(|| BotError::Voice("no active voice call".to_owned()))?;
    let handle = {
        let mut guard = call.lock().await;
        guard.play_input(input)
    };
    attach_lifecycle_events(&handle, guild_id, data)?;

    let current = CurrentTrack {
        handle,
        metadata: QueuedTrack::from(metadata),
    };
    data.current.lock().await.insert(guild_id, current);
    Ok(true)
}
