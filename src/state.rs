use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serenity::model::id::GuildId;
use songbird::tracks::TrackHandle;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::AbortHandle;

use crate::audio::cache::{CacheKey, MetadataCache, ResolvedMetadata};
use crate::config::AppConfig;
use crate::error::BotError;

/// Foreground metadata extractions allowed at once.
pub const FOREGROUND_EXTRACTIONS: usize = 4;
/// Speculative next-track extractions allowed at once, globally.
pub const PREFETCH_EXTRACTIONS: usize = 2;

/// A lightweight queue entry that can be rebuilt into an audio source later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedTrack {
    pub title: String,
    pub url: String,
    pub requester: String,
    pub duration: Option<Duration>,
}

/// The currently playing track plus the handle needed to control it.
pub struct CurrentTrack {
    pub handle: TrackHandle,
    pub metadata: QueuedTrack,
}

/// Fresh metadata prepared ahead of a queue transition.
#[derive(Debug, Clone)]
pub struct PrefetchedTrack {
    pub key: CacheKey,
    pub metadata: ResolvedMetadata,
    pub version: u64,
    pub task_id: u64,
}

impl PrefetchedTrack {
    pub fn is_usable(&self, key: &CacheKey, version: u64, now: Instant) -> bool {
        self.key == *key && self.version == version && self.metadata.is_fresh_at(now)
    }
}

/// Shared process-wide state passed to every Poise command.
#[derive(Clone)]
pub struct BotData {
    pub config: Arc<AppConfig>,
    pub http_client: reqwest::Client,
    pub voice: Arc<songbird::Songbird>,
    /// Leaked once at startup so Songbird sources can borrow a `'static` binary name.
    pub ytdlp_binary: &'static str,
    pub queues: Arc<Mutex<HashMap<GuildId, VecDeque<QueuedTrack>>>>,
    pub current: Arc<Mutex<HashMap<GuildId, CurrentTrack>>>,
    pub metadata_cache: Arc<MetadataCache>,
    pub extract_semaphore: Arc<Semaphore>,
    pub prefetch_semaphore: Arc<Semaphore>,
    pub prefetch: Arc<Mutex<HashMap<GuildId, PrefetchedTrack>>>,
    pub prefetch_tasks: Arc<Mutex<HashMap<GuildId, (u64, AbortHandle)>>>,
    pub next_prefetch_id: Arc<AtomicU64>,
    pub queue_versions: Arc<Mutex<HashMap<GuildId, u64>>>,
    pub generations: Arc<Mutex<HashMap<GuildId, u64>>>,
    pub guild_ops: Arc<Mutex<HashMap<GuildId, Arc<Mutex<()>>>>>,
}

impl std::fmt::Debug for BotData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BotData")
            .field("config", &self.config)
            .field("ytdlp_binary", &self.ytdlp_binary)
            .finish_non_exhaustive()
    }
}

impl BotData {
    /// Build shared state from validated configuration.
    pub fn new(config: AppConfig) -> Result<Self, BotError> {
        let ytdlp_binary = Box::leak(config.ytdlp_binary.clone().into_boxed_str());

        Ok(Self {
            config: Arc::new(config),
            http_client: reqwest::Client::builder().build()?,
            voice: songbird::Songbird::serenity(),
            ytdlp_binary,
            queues: Arc::new(Mutex::new(HashMap::new())),
            current: Arc::new(Mutex::new(HashMap::new())),
            metadata_cache: Arc::new(MetadataCache::default()),
            extract_semaphore: Arc::new(Semaphore::new(FOREGROUND_EXTRACTIONS)),
            prefetch_semaphore: Arc::new(Semaphore::new(PREFETCH_EXTRACTIONS)),
            prefetch: Arc::new(Mutex::new(HashMap::new())),
            prefetch_tasks: Arc::new(Mutex::new(HashMap::new())),
            next_prefetch_id: Arc::new(AtomicU64::new(1)),
            queue_versions: Arc::new(Mutex::new(HashMap::new())),
            generations: Arc::new(Mutex::new(HashMap::new())),
            guild_ops: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Return the per-guild operation lock, creating it on first use.
    pub async fn guild_lock(&self, guild_id: GuildId) -> Arc<Mutex<()>> {
        self.guild_ops
            .lock()
            .await
            .entry(guild_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Current playback generation. Stale generations must not advance the queue.
    pub async fn current_generation(&self, guild_id: GuildId) -> u64 {
        self.generations
            .lock()
            .await
            .get(&guild_id)
            .copied()
            .unwrap_or_default()
    }

    /// Advance the playback generation, invalidating events from previous tracks.
    pub async fn bump_generation(&self, guild_id: GuildId) -> u64 {
        let mut generations = self.generations.lock().await;
        let next = generations
            .get(&guild_id)
            .copied()
            .unwrap_or_default()
            .wrapping_add(1);
        generations.insert(guild_id, next);
        next
    }

    /// Current queue version. Changes invalidate prefetched next tracks.
    pub async fn current_queue_version(&self, guild_id: GuildId) -> u64 {
        self.queue_versions
            .lock()
            .await
            .get(&guild_id)
            .copied()
            .unwrap_or_default()
    }

    /// Advance the queue version.
    pub async fn bump_queue_version(&self, guild_id: GuildId) -> u64 {
        let mut versions = self.queue_versions.lock().await;
        let next = versions
            .get(&guild_id)
            .copied()
            .unwrap_or_default()
            .wrapping_add(1);
        versions.insert(guild_id, next);
        next
    }

    /// Drop all mutable state associated with a guild after disconnect.
    pub async fn remove_guild_state(&self, guild_id: GuildId) {
        if let Some((_, handle)) = self.prefetch_tasks.lock().await.remove(&guild_id) {
            handle.abort();
        }
        self.prefetch.lock().await.remove(&guild_id);
        self.queues.lock().await.remove(&guild_id);
        self.current.lock().await.remove(&guild_id);
        self.generations.lock().await.remove(&guild_id);
        self.queue_versions.lock().await.remove(&guild_id);
        self.guild_ops.lock().await.remove(&guild_id);
    }

    /// Reserve a unique identifier for a speculative task.
    pub fn next_prefetch_id(&self) -> u64 {
        self.next_prefetch_id.fetch_add(1, Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::{BotData, FOREGROUND_EXTRACTIONS, PREFETCH_EXTRACTIONS};
    use crate::config::AppConfig;
    use serenity::model::id::GuildId;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    fn test_data() -> BotData {
        let values = HashMap::from([("DISCORD_TOKEN".to_owned(), "test-token".to_owned())]);
        let config = AppConfig::from_map(&values).expect("valid config");
        BotData::new(config).expect("state")
    }

    #[tokio::test]
    async fn generation_invalidates_stale_transitions() {
        let data = test_data();
        let guild_id = GuildId::new(10);
        let first = data.bump_generation(guild_id).await;
        let second = data.bump_generation(guild_id).await;

        assert_ne!(first, second);
        assert_eq!(data.current_generation(guild_id).await, second);
    }

    #[tokio::test]
    async fn guild_lock_serializes_operations() {
        let data = test_data();
        let guild_id = GuildId::new(11);
        let first_lock = data.guild_lock(guild_id).await;
        let second_lock = data.guild_lock(guild_id).await;
        assert!(Arc::ptr_eq(&first_lock, &second_lock));

        let guard = first_lock.lock().await;
        let acquired = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&acquired);
        let task_data = data.clone();
        let task = tokio::spawn(async move {
            let lock = task_data.guild_lock(guild_id).await;
            let _task_guard = lock.lock().await;
            flag.store(true, Ordering::SeqCst);
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(!acquired.load(Ordering::SeqCst));
        drop(guard);
        task.await.expect("guild task");
        assert!(acquired.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn extraction_concurrency_is_bounded() {
        let data = test_data();
        assert_eq!(
            data.extract_semaphore.available_permits(),
            FOREGROUND_EXTRACTIONS
        );
        assert_eq!(
            data.prefetch_semaphore.available_permits(),
            PREFETCH_EXTRACTIONS
        );
    }

    #[tokio::test]
    async fn guild_state_removal_clears_mutable_maps() {
        let data = test_data();
        let guild_id = GuildId::new(12);
        let _ = data.bump_generation(guild_id).await;
        let _ = data.bump_queue_version(guild_id).await;
        let _ = data.next_prefetch_id();

        data.remove_guild_state(guild_id).await;

        assert!(data.generations.lock().await.is_empty());
        assert!(data.queue_versions.lock().await.is_empty());
        assert!(data.prefetch.lock().await.is_empty());
        assert!(data.prefetch_tasks.lock().await.is_empty());
        assert!(data.queues.lock().await.is_empty());
        assert!(data.current.lock().await.is_empty());
    }
}
