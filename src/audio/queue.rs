use serenity::model::id::GuildId;

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

/// Append a track to a guild's queue.
pub async fn enqueue(data: &BotData, guild_id: GuildId, track: QueuedTrack) {
    data.queues
        .lock()
        .await
        .entry(guild_id)
        .or_default()
        .push_back(track);
}

/// Remove all queued tracks for a guild.
pub async fn clear_queue(data: &BotData, guild_id: GuildId) {
    data.queues.lock().await.remove(&guild_id);
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
