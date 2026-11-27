use std::time::Instant;

use serenity::model::id::GuildId;

use crate::audio::cache::{CacheKey, ResolvedMetadata};
use crate::audio::query::classify;
use crate::audio::queue::peek_front;
use crate::audio::source::resolve_metadata;
use crate::state::{BotData, PrefetchedTrack};

/// Cancel the current speculative task and discard its result.
pub async fn invalidate_prefetch(data: &BotData, guild_id: GuildId) {
    if let Some((_, handle)) = data.prefetch_tasks.lock().await.remove(&guild_id) {
        handle.abort();
    }
    data.prefetch.lock().await.remove(&guild_id);
}

fn cache_key(data: &BotData, query: &str) -> CacheKey {
    CacheKey::new(
        query,
        data.config.youtube_cookies_path.as_deref().unwrap_or(""),
    )
}

/// Schedule resolution of only the next queued track for a guild.
pub async fn schedule_next(data: &BotData, guild_id: GuildId) {
    invalidate_prefetch(data, guild_id).await;

    let Some(queued) = peek_front(data, guild_id).await else {
        return;
    };
    let Ok(query) = classify(&queued.url) else {
        tracing::debug!(guild_id = %guild_id, "skipped prefetch for invalid queued URL");
        return;
    };

    let version = data.current_queue_version(guild_id).await;
    let key = cache_key(data, query.as_str());
    let task_id = data.next_prefetch_id();

    if let Some(metadata) = data.metadata_cache.get_fresh(&key).await {
        let prefetched = PrefetchedTrack {
            key,
            metadata,
            version,
            task_id,
        };
        data.prefetch.lock().await.insert(guild_id, prefetched);
        return;
    }

    let task_data = data.clone();
    let task_query = query;
    let handle = tokio::spawn(async move {
        let _permit = task_data.prefetch_semaphore.acquire().await;

        if task_data.current_queue_version(guild_id).await != version {
            return;
        }

        let resolved = match resolve_metadata(&task_data, &task_query).await {
            Ok(metadata) => metadata,
            Err(_) => {
                tracing::debug!(guild_id = %guild_id, "prefetch extraction failed");
                return;
            }
        };

        if task_data.current_queue_version(guild_id).await != version {
            return;
        }

        let prefetched = PrefetchedTrack {
            key: cache_key(&task_data, task_query.as_str()),
            metadata: resolved,
            version,
            task_id,
        };
        task_data.prefetch.lock().await.insert(guild_id, prefetched);

        let mut tasks = task_data.prefetch_tasks.lock().await;
        if tasks
            .get(&guild_id)
            .is_some_and(|(active_id, _)| *active_id == task_id)
        {
            tasks.remove(&guild_id);
        }
    });

    data.prefetch_tasks
        .lock()
        .await
        .insert(guild_id, (task_id, handle.abort_handle()));
}

/// Consume a fresh prefetch result when it still matches the queue state.
pub async fn take_prefetched(
    data: &BotData,
    guild_id: GuildId,
    key: &CacheKey,
    version: u64,
) -> Option<ResolvedMetadata> {
    let mut prefetch = data.prefetch.lock().await;
    let usable = prefetch
        .get(&guild_id)
        .is_some_and(|entry| entry.is_usable(key, version, Instant::now()));

    if usable {
        prefetch.remove(&guild_id).map(|entry| entry.metadata)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{invalidate_prefetch, take_prefetched};
    use crate::audio::cache::{CacheKey, ResolvedMetadata, CACHE_TTL};
    use crate::audio::queue::enqueue;
    use crate::config::AppConfig;
    use crate::state::{BotData, PrefetchedTrack, QueuedTrack};
    use serenity::model::id::GuildId;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    fn test_data() -> BotData {
        let values = HashMap::from([("DISCORD_TOKEN".to_owned(), "test-token".to_owned())]);
        let config = AppConfig::from_map(&values).expect("valid config");
        BotData::new(config).expect("state")
    }

    fn key() -> CacheKey {
        CacheKey::new("ytsearch:เพลงไทย 🎵", "")
    }

    fn metadata(now: Instant) -> ResolvedMetadata {
        ResolvedMetadata::new_at(
            "เพลงไทย".to_owned(),
            "https://example.com/track".to_owned(),
            Some(Duration::from_secs(120)),
            now,
        )
    }

    #[tokio::test]
    async fn prefetch_hit_returns_metadata_once() {
        let data = test_data();
        let guild_id = GuildId::new(1);
        let version = data.current_queue_version(guild_id).await;
        let entry = PrefetchedTrack {
            key: key(),
            metadata: metadata(Instant::now()),
            version,
            task_id: 1,
        };
        data.prefetch.lock().await.insert(guild_id, entry);

        let taken = take_prefetched(&data, guild_id, &key(), version).await;
        assert!(taken.is_some());
        assert!(take_prefetched(&data, guild_id, &key(), version)
            .await
            .is_none());
    }

    #[tokio::test]
    async fn queue_mutation_invalidates_prefetch() {
        let data = test_data();
        let guild_id = GuildId::new(2);
        let version = data.current_queue_version(guild_id).await;
        let entry = PrefetchedTrack {
            key: key(),
            metadata: metadata(Instant::now()),
            version,
            task_id: 2,
        };
        data.prefetch.lock().await.insert(guild_id, entry);

        enqueue(
            &data,
            guild_id,
            QueuedTrack {
                title: "next".to_owned(),
                url: "https://example.com/next".to_owned(),
                requester: "user".to_owned(),
                duration: None,
            },
        )
        .await;

        assert!(take_prefetched(&data, guild_id, &key(), version)
            .await
            .is_none());
        assert!(data.prefetch.lock().await.get(&guild_id).is_none());
    }

    #[tokio::test]
    async fn expired_prefetch_is_not_reused() {
        let data = test_data();
        let guild_id = GuildId::new(3);
        let version = data.current_queue_version(guild_id).await;
        let expired = metadata(Instant::now() - CACHE_TTL - Duration::from_millis(1));
        let entry = PrefetchedTrack {
            key: key(),
            metadata: expired,
            version,
            task_id: 3,
        };
        data.prefetch.lock().await.insert(guild_id, entry);

        assert!(take_prefetched(&data, guild_id, &key(), version)
            .await
            .is_none());
    }

    #[tokio::test]
    async fn invalidation_cancels_and_clears_task_state() {
        let data = test_data();
        let guild_id = GuildId::new(4);
        let task = tokio::spawn(async {
            std::future::pending::<()>().await;
        });
        data.prefetch_tasks
            .lock()
            .await
            .insert(guild_id, (9, task.abort_handle()));
        data.prefetch.lock().await.insert(
            guild_id,
            PrefetchedTrack {
                key: key(),
                metadata: metadata(Instant::now()),
                version: 0,
                task_id: 9,
            },
        );

        invalidate_prefetch(&data, guild_id).await;

        assert!(data.prefetch_tasks.lock().await.is_empty());
        assert!(data.prefetch.lock().await.is_empty());
    }
}
