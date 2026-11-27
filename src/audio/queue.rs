use serenity::model::id::GuildId;

use crate::audio::prefetch::invalidate_prefetch;
use crate::state::{BotData, CurrentTrack, QueuedTrack};
use crate::ui::embeds::{format_queue_entry, QueueEntryView};

/// Return a copy of the guild's queued tracks.
pub async fn queue_snapshot(data: &BotData, guild_id: GuildId) -> Vec<QueuedTrack> {
    data.queues
        .lock()
        .await
        .get(&guild_id)
        .map_or_else(Vec::new, |queue| queue.iter().cloned().collect())
}

/// Return metadata for the currently playing track, if any.
pub async fn current_snapshot(data: &BotData, guild_id: GuildId) -> Option<QueuedTrack> {
    data.current
        .lock()
        .await
        .get(&guild_id)
        .map(|current| current.metadata.clone())
}

/// Append a track to a guild's queue and invalidate stale prefetch work.
pub async fn enqueue(data: &BotData, guild_id: GuildId, track: QueuedTrack) {
    data.bump_queue_version(guild_id).await;
    invalidate_prefetch(data, guild_id).await;
    data.queues
        .lock()
        .await
        .entry(guild_id)
        .or_default()
        .push_back(track);
}

/// Remove all queued tracks for a guild and cancel speculative work.
pub async fn clear_queue(data: &BotData, guild_id: GuildId) {
    data.bump_queue_version(guild_id).await;
    invalidate_prefetch(data, guild_id).await;
    data.queues.lock().await.remove(&guild_id);
}

/// Look at the next queued track without mutating the queue.
pub async fn peek_front(data: &BotData, guild_id: GuildId) -> Option<QueuedTrack> {
    data.queues
        .lock()
        .await
        .get(&guild_id)
        .and_then(|queue| queue.front().cloned())
}

/// Pop the next queued track and invalidate prefetch work for the new head.
pub async fn pop_front(data: &BotData, guild_id: GuildId) -> Option<QueuedTrack> {
    let queued = {
        let mut queues = data.queues.lock().await;
        let track = queues
            .get_mut(&guild_id)
            .and_then(|queue| queue.pop_front());
        if queues.get(&guild_id).is_some_and(|queue| queue.is_empty()) {
            queues.remove(&guild_id);
        }
        track
    };

    if queued.is_some() {
        data.bump_queue_version(guild_id).await;
        invalidate_prefetch(data, guild_id).await;
    }
    queued
}

/// Return true when no track is currently marked as playing.
pub async fn is_idle(data: &BotData, guild_id: GuildId) -> bool {
    !data.current.lock().await.contains_key(&guild_id)
}

/// Remove and return the current track handle so callers can stop it.
pub async fn take_current(data: &BotData, guild_id: GuildId) -> Option<CurrentTrack> {
    data.current.lock().await.remove(&guild_id)
}

/// Format one queue row from stored metadata.
pub fn format_queue_line(index: usize, track: &QueuedTrack) -> String {
    format_queue_entry(
        index,
        &QueueEntryView {
            title: &track.title,
            url: Some(&track.url),
            requester: &track.requester,
            duration: track.duration,
        },
    )
}

/// Report whether a guild has any queued tracks.
pub async fn has_queue(data: &BotData, guild_id: GuildId) -> bool {
    data.queues
        .lock()
        .await
        .get(&guild_id)
        .is_some_and(|queue| !queue.is_empty())
}
