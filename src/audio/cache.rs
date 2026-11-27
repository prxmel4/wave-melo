use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, Notify};

use crate::error::BotError;

/// Default upper bound for cached track metadata entries.
pub const MAX_CACHE_ENTRIES: usize = 128;
/// Default metadata TTL. This does not apply to ephemeral stream URLs.
pub const CACHE_TTL: Duration = Duration::from_secs(20 * 60);

#[cfg(test)]
const FAILURE_TTL: Duration = Duration::from_millis(50);
#[cfg(not(test))]
const FAILURE_TTL: Duration = Duration::from_secs(2);

/// Cache identity for a resolved track.
///
/// The query keeps case-sensitive URL components and multilingual search text.
/// The authentication context prevents cookie-backed results from being reused
/// for a different cookie configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    query: String,
    auth_context: String,
}

impl CacheKey {
    pub fn new(query: &str, auth_context: &str) -> Self {
        Self {
            query: query.to_owned(),
            auth_context: auth_context.to_owned(),
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }
}

/// Stable track metadata. No audio bytes or signed stream URLs are retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMetadata {
    pub title: String,
    pub url: String,
    pub duration: Option<Duration>,
    inserted_at: Instant,
    expires_at: Instant,
}

impl ResolvedMetadata {
    pub fn new(title: String, url: String, duration: Option<Duration>, now: Instant) -> Self {
        Self {
            title,
            url,
            duration,
            inserted_at: now,
            expires_at: now + CACHE_TTL,
        }
    }

    #[cfg(test)]
    pub fn new_at(title: String, url: String, duration: Option<Duration>, now: Instant) -> Self {
        Self::new(title, url, duration, now)
    }

    pub fn is_fresh_at(&self, now: Instant) -> bool {
        now < self.expires_at
    }

    pub fn age_at(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.inserted_at)
    }
}

#[derive(Debug, Default)]
struct CacheInner {
    entries: HashMap<CacheKey, ResolvedMetadata>,
    order: VecDeque<CacheKey>,
    failures: HashMap<CacheKey, (Instant, Arc<str>)>,
    inflight: HashMap<CacheKey, Arc<Notify>>,
}

impl CacheInner {
    fn touch(&mut self, key: &CacheKey) {
        if let Some(position) = self.order.iter().position(|candidate| candidate == key) {
            if let Some(entry) = self.order.remove(position) {
                self.order.push_back(entry);
            }
        }
    }

    fn insert(&mut self, key: CacheKey, metadata: ResolvedMetadata) {
        self.order.retain(|candidate| candidate != &key);
        self.order.push_back(key.clone());
        self.entries.insert(key, metadata);

        while self.entries.len() > MAX_CACHE_ENTRIES {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

/// Bounded, process-local metadata cache with singleflight resolution.
#[derive(Debug, Clone, Default)]
pub struct MetadataCache {
    inner: Arc<Mutex<CacheInner>>,
}

enum CacheAction {
    Wait(Arc<Notify>),
    Resolve(Arc<Notify>),
}

impl MetadataCache {
    /// Return a fresh entry without performing any resolution.
    pub async fn get_fresh(&self, key: &CacheKey) -> Option<ResolvedMetadata> {
        let mut inner = self.inner.lock().await;
        let now = Instant::now();
        let metadata = inner.entries.get(key).cloned().filter(|entry| {
            if entry.is_fresh_at(now) {
                true
            } else {
                inner.entries.remove(key);
                inner.order.retain(|candidate| candidate != key);
                false
            }
        });

        if metadata.is_some() {
            inner.touch(key);
        }
        metadata
    }

    /// Resolve metadata once per key, sharing the result with concurrent callers.
    pub async fn get_or_resolve<F, Fut>(
        &self,
        key: CacheKey,
        resolve: F,
    ) -> Result<ResolvedMetadata, BotError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<ResolvedMetadata, BotError>>,
    {
        loop {
            let action = {
                let mut inner = self.inner.lock().await;
                let now = Instant::now();

                let fresh = inner
                    .entries
                    .get(&key)
                    .filter(|entry| entry.is_fresh_at(now))
                    .cloned();
                if let Some(metadata) = fresh {
                    inner.touch(&key);
                    return Ok(metadata);
                }
                if inner.entries.remove(&key).is_some() {
                    inner.order.retain(|candidate| candidate != &key);
                }

                if let Some((failed_at, message)) = inner.failures.get(&key) {
                    if failed_at.elapsed() < FAILURE_TTL {
                        return Err(BotError::YtDlp(message.to_string()));
                    }
                    inner.failures.remove(&key);
                }

                if let Some(existing) = inner.inflight.get(&key) {
                    CacheAction::Wait(Arc::clone(existing))
                } else {
                    let notify = Arc::new(Notify::new());
                    inner.inflight.insert(key.clone(), Arc::clone(&notify));
                    CacheAction::Resolve(notify)
                }
            };

            let notify = match action {
                CacheAction::Wait(notify) => {
                    notify.notified().await;
                    continue;
                }
                CacheAction::Resolve(notify) => notify,
            };

            let result = resolve().await;
            let mut inner = self.inner.lock().await;
            inner.inflight.remove(&key);

            match result {
                Ok(metadata) => {
                    inner.insert(key, metadata.clone());
                    notify.notify_waiters();
                    return Ok(metadata);
                }
                Err(error) => {
                    let message: Arc<str> = match &error {
                        BotError::YtDlp(message) => Arc::from(message.as_str()),
                        _ => Arc::from("metadata resolution failed"),
                    };
                    inner.failures.insert(key, (Instant::now(), message));
                    notify.notify_waiters();
                    return Err(error);
                }
            }
        }
    }

    /// Number of live entries. Intended for diagnostics and tests.
    pub async fn len(&self) -> usize {
        self.inner.lock().await.entries.len()
    }

    /// Whether the cache has no live entries. Intended for diagnostics and tests.
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    #[cfg(test)]
    pub async fn insert_at(&self, key: CacheKey, metadata: ResolvedMetadata) {
        self.inner.lock().await.insert(key, metadata);
    }
}

#[cfg(test)]
mod tests {
    use super::{CacheKey, MetadataCache, ResolvedMetadata, FAILURE_TTL, MAX_CACHE_ENTRIES};
    use crate::error::BotError;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn key(query: &str) -> CacheKey {
        CacheKey::new(query, "")
    }

    fn metadata(title: &str) -> ResolvedMetadata {
        ResolvedMetadata::new_at(
            title.to_owned(),
            format!("https://example.com/{title}"),
            Some(Duration::from_secs(42)),
            Instant::now(),
        )
    }

    #[tokio::test]
    async fn cache_hit_avoids_second_resolution() {
        let cache = MetadataCache::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);

        let first = cache
            .get_or_resolve(key("query"), || async {
                counter.fetch_add(1, Ordering::Relaxed);
                Ok(metadata("first"))
            })
            .await
            .expect("first resolution");
        let second = cache
            .get_or_resolve(key("query"), || async {
                counter.fetch_add(1, Ordering::Relaxed);
                Ok(metadata("second"))
            })
            .await
            .expect("second resolution");

        assert_eq!(first.title, "first");
        assert_eq!(second.title, "first");
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn expired_entry_is_resolved_again() {
        let cache = MetadataCache::default();
        let expired = ResolvedMetadata::new_at(
            "old".to_owned(),
            "https://example.com/old".to_owned(),
            None,
            Instant::now() - super::CACHE_TTL - Duration::from_millis(1),
        );
        cache.insert_at(key("expired"), expired).await;

        let resolved = cache
            .get_or_resolve(key("expired"), || async { Ok(metadata("fresh")) })
            .await
            .expect("resolution");

        assert_eq!(resolved.title, "fresh");
        assert!(cache.get_fresh(&key("expired")).await.is_some());
    }

    #[tokio::test]
    async fn capacity_is_strictly_enforced() {
        let cache = MetadataCache::default();
        for index in 0..=MAX_CACHE_ENTRIES {
            cache
                .insert_at(key(&format!("key-{index}")), metadata(&format!("{index}")))
                .await;
        }

        assert_eq!(cache.len().await, MAX_CACHE_ENTRIES);
        assert!(cache.get_fresh(&key("key-0")).await.is_none());
        assert!(cache.get_fresh(&key("key-128")).await.is_some());
    }

    #[tokio::test]
    async fn eviction_is_deterministic_lru() {
        let cache = MetadataCache::default();
        for index in 0..MAX_CACHE_ENTRIES {
            cache
                .insert_at(key(&format!("key-{index}")), metadata(&format!("{index}")))
                .await;
        }

        assert!(cache.get_fresh(&key("key-0")).await.is_some());
        cache.insert_at(key("new"), metadata("new")).await;

        assert!(cache.get_fresh(&key("key-1")).await.is_none());
        assert!(cache.get_fresh(&key("key-0")).await.is_some());
        assert!(cache.get_fresh(&key("new")).await.is_some());
        assert_eq!(cache.len().await, MAX_CACHE_ENTRIES);
    }

    #[tokio::test]
    async fn replacing_an_entry_updates_the_value() {
        let cache = MetadataCache::default();
        cache.insert_at(key("same"), metadata("old")).await;
        cache.insert_at(key("same"), metadata("new")).await;

        let entry = cache.get_fresh(&key("same")).await.expect("replaced entry");
        assert_eq!(entry.title, "new");
        assert_eq!(cache.len().await, 1);
    }

    #[tokio::test]
    async fn thai_and_unicode_keys_are_preserved() {
        let cache = MetadataCache::default();
        cache
            .insert_at(key("ytsearch:เพลงไทย 🎵"), metadata("เพลงไทย"))
            .await;

        let entry = cache
            .get_fresh(&key("ytsearch:เพลงไทย 🎵"))
            .await
            .expect("unicode entry");
        assert_eq!(entry.title, "เพลงไทย");
        assert!(cache.get_fresh(&key("ytsearch:เพลงไทย 🎶")).await.is_none());
    }

    #[tokio::test]
    async fn concurrent_requests_share_one_resolution() {
        let cache = Arc::new(MetadataCache::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();

        for _ in 0..8 {
            let cache = Arc::clone(&cache);
            let calls = Arc::clone(&calls);
            tasks.push(tokio::spawn(async move {
                cache
                    .get_or_resolve(key("shared"), || async {
                        calls.fetch_add(1, Ordering::Relaxed);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Ok(metadata("shared"))
                    })
                    .await
                    .expect("shared resolution")
            }));
        }

        for task in tasks {
            let result = task.await.expect("task");
            assert_eq!(result.title, "shared");
        }
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn transient_failure_is_coalesced_then_retried() {
        let cache = MetadataCache::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);

        let first = cache
            .get_or_resolve(key("flaky"), || async {
                counter.fetch_add(1, Ordering::Relaxed);
                Err(BotError::YtDlp("metadata failed".to_owned()))
            })
            .await
            .unwrap_err();
        let second = cache
            .get_or_resolve(key("flaky"), || async {
                counter.fetch_add(1, Ordering::Relaxed);
                Err(BotError::YtDlp("metadata failed".to_owned()))
            })
            .await
            .unwrap_err();

        assert_eq!(first.to_string(), second.to_string());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(cache.get_fresh(&key("flaky")).await.is_none());

        tokio::time::sleep(FAILURE_TTL + Duration::from_millis(10)).await;
        let _ = cache
            .get_or_resolve(key("flaky"), || async {
                counter.fetch_add(1, Ordering::Relaxed);
                Err(BotError::YtDlp("metadata failed".to_owned()))
            })
            .await
            .unwrap_err();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn benchmark_cache_hit_latency() {
        let cache = MetadataCache::default();
        let cached_key = key("warm-key");
        cache.insert_at(cached_key.clone(), metadata("warm")).await;

        let mut samples = Vec::with_capacity(1_000);
        for _ in 0..samples.capacity() {
            let started = Instant::now();
            let _ = cache.get_fresh(&cached_key).await;
            samples.push(started.elapsed().as_nanos() as u64);
        }
        samples.sort_unstable();

        let p50 = samples[samples.len() / 2];
        let p95 = samples[(samples.len() * 95) / 100];
        eprintln!("cache_hit_samples=1000 p50_ns={p50} p95_ns={p95}");
        assert!(p95 < 5_000_000, "cache hit p95 exceeded 5 ms");
    }
}
