//! Smart autoplay: resolve related tracks when a natural finish leaves the queue empty.
//!
//! Resolution runs in a background Tokio task so the Lavalink event handler is
//! never blocked by network I/O. A per-guild generation token plus the shared
//! guild operation lock ensure a stale task can never override a user action.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use lavalink_rs::model::events::TrackEndReason;
use lavalink_rs::model::track::TrackData;
use lavalink_rs::prelude::LavalinkClient;
use serenity::model::id::GuildId;
use tokio::task::JoinHandle;

use crate::audio::loader::{load_tracks, track_url, LoadedTracks};
use crate::audio::query::normalize_search;
use crate::audio::Query;
use crate::state::{GuildStateRegistry, LoopMode};

/// Maximum consecutive resolution failures before autoplay pauses itself.
pub const MAX_AUTOPLAY_FAILURES: u8 = 3;

/// Extract a YouTube video ID from a URL or bare identifier.
///
/// Only YouTube hosts are accepted, and the fixed length and character set
/// prevent query injection into Mix URLs.
pub fn video_id(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if is_video_id(trimmed) {
        return Some(trimmed.to_owned());
    }
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    let candidate = match host.to_ascii_lowercase().as_str() {
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com" => {
            if let Some((_, query)) = path.split_once('?') {
                query
                    .split('&')
                    .find_map(|pair| pair.strip_prefix("v="))?
                    .split(['?', '#'])
                    .next()?
                    .to_owned()
            } else {
                path.strip_prefix("shorts/")?
                    .split(['?', '#'])
                    .next()?
                    .to_owned()
            }
        }
        "youtu.be" => path.split(['?', '#']).next()?.to_owned(),
        _ => return None,
    };
    if is_video_id(&candidate) {
        Some(candidate)
    } else {
        None
    }
}

fn is_video_id(value: &str) -> bool {
    value.len() == 11
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// Build the YouTube Mix URL used as the primary related-track source.
pub fn mix_url(id: &str) -> Option<String> {
    if !is_video_id(id) {
        return None;
    }
    Some(format!("https://www.youtube.com/watch?v={id}&list=RD{id}"))
}

/// Build the verified search fallback for a finished track.
pub fn search_fallback(title: &str, author: &str) -> String {
    let query = if author.is_empty() || author.eq_ignore_ascii_case("unknown") {
        title.trim().to_owned()
    } else {
        format!("{} {}", title.trim(), author.trim())
    };
    normalize_search(&query)
}

/// Whether autoplay should react to this track-end event.
pub fn should_autoplay(
    autoplay: bool,
    reason: &TrackEndReason,
    loop_mode: LoopMode,
    queue_len: usize,
) -> bool {
    autoplay && *reason == TrackEndReason::Finished && loop_mode == LoopMode::Off && queue_len == 0
}

/// Pick the first candidate that is playable and not in recent history.
pub fn pick_candidate(candidates: Vec<TrackData>, recent: &VecDeque<String>) -> Option<TrackData> {
    candidates.into_iter().find(|track| {
        !track.info.is_stream
            && !track.info.identifier.is_empty()
            && !recent.contains(&track.info.identifier)
    })
}

/// Extract playable tracks from a loaded result.
pub fn loaded_candidates(loaded: LoadedTracks) -> Vec<TrackData> {
    match loaded {
        LoadedTracks::Single(track) | LoadedTracks::SearchResult(track) => vec![track],
        LoadedTracks::Playlist { tracks, .. } => tracks,
        LoadedTracks::Empty => Vec::new(),
    }
}

/// Pure dedup helper: remove identifiers already present in bounded history.
pub fn dedupe(identifiers: Vec<String>, recent: &VecDeque<String>) -> Vec<String> {
    identifiers
        .into_iter()
        .filter(|id| !recent.contains(id))
        .collect()
}

/// Cancellable per-guild autoplay task slot.
///
/// Generation checks let a background task detect that the user stopped,
/// disconnected, or queued a track while the related lookup was in flight.
#[derive(Debug, Clone, Default)]
pub struct AutoplayControl {
    inner: Arc<AutoplayInner>,
}

#[derive(Debug, Default)]
struct AutoplayInner {
    generation: AtomicU64,
    task: StdMutex<Option<JoinHandle<()>>>,
}

impl AutoplayControl {
    /// Advance the generation and install a fresh task handle.
    pub fn arm(&self, spawn: impl FnOnce(u64) -> JoinHandle<()>) -> u64 {
        let mut slot = self
            .inner
            .task
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let generation = self
            .inner
            .generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1);
        let handle = spawn(generation);
        if let Some(previous) = slot.replace(handle) {
            previous.abort();
        }
        generation
    }

    /// Invalidate the current generation and abort its task.
    pub fn cancel(&self) {
        let mut slot = self
            .inner
            .task
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        self.inner.generation.fetch_add(1, Ordering::SeqCst);
        if let Some(previous) = slot.take() {
            previous.abort();
        }
    }

    /// True while the captured generation is still authoritative.
    pub fn is_current(&self, generation: u64) -> bool {
        self.inner.generation.load(Ordering::SeqCst) == generation
    }

    /// True while the installed task has not completed.
    pub fn is_armed(&self) -> bool {
        self.inner
            .task
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }
}

/// Resolve a related track: YouTube Mix first, `ytsearch:` fallback second.
///
/// Mix playlists usually start with the seed video itself, so every candidate
/// list is filtered against the bounded identifier history before selection.
async fn resolve_related(
    client: &LavalinkClient,
    guild_id: u64,
    finished: &TrackData,
    recent: &VecDeque<String>,
) -> Option<TrackData> {
    let source = video_id(&finished.info.identifier).or_else(|| video_id(&track_url(finished)));

    if let Some(url) = source.as_deref().and_then(mix_url) {
        match load_tracks(client, guild_id, &Query::Url(url)).await {
            Ok(loaded) => {
                if let Some(track) = pick_candidate(loaded_candidates(loaded), recent) {
                    return Some(track);
                }
            }
            Err(error) => {
                tracing::debug!(guild_id = %guild_id, error = %error, "autoplay mix load failed");
            }
        }
    }

    // Verified fallback: works even for non-YouTube finished tracks.
    let query = search_fallback(&finished.info.title, &finished.info.author);
    match load_tracks(client, guild_id, &Query::Search(query)).await {
        Ok(loaded) => pick_candidate(loaded_candidates(loaded), recent),
        Err(error) => {
            tracing::warn!(guild_id = %guild_id, error = %error, "autoplay fallback failed");
            None
        }
    }
}

/// Background task body. Re-checks state after every network await and
/// serializes the final play with slash commands via the guild op lock.
async fn run_autoplay(
    client: LavalinkClient,
    registry: Arc<GuildStateRegistry>,
    guild_id: u64,
    finished: TrackData,
    generation: u64,
) {
    let Some(session) = registry.existing_session(guild_id) else {
        return;
    };
    let Some(player_ctx) = client.get_player_context(guild_id) else {
        return;
    };

    let recent = {
        let details = session.details.lock().await;
        if !session.autoplay.is_current(generation) || !details.autoplay {
            return;
        }
        if details.autoplay_failures >= MAX_AUTOPLAY_FAILURES {
            tracing::warn!(guild_id = %guild_id, "autoplay paused after repeated failures");
            return;
        }
        details.recent_identifiers.clone()
    };

    let Some(mut related) = resolve_related(&client, guild_id, &finished, &recent).await else {
        let mut details = session.details.lock().await;
        details.autoplay_failures = details.autoplay_failures.saturating_add(1);
        tracing::warn!(
            guild_id = %guild_id,
            failures = details.autoplay_failures,
            "autoplay found no related track"
        );
        return;
    };

    // Serialize with slash commands, then re-verify under the lock. The user
    // may have stopped, disconnected, or queued a track during resolution.
    let Some(handles) = registry.runtime() else {
        return;
    };
    let guild = GuildId::new(guild_id);
    let lock = handles
        .guild_ops
        .lock()
        .await
        .entry(guild)
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    let _guard = lock.lock().await;

    if !session.autoplay.is_current(generation) {
        return;
    }
    let Ok(player) = player_ctx.get_player().await else {
        return;
    };
    let Ok(queue_len) = player_ctx.get_queue().get_count().await else {
        return;
    };
    {
        let details = session.details.lock().await;
        if !details.autoplay
            || session.loop_control.mode() != LoopMode::Off
            || details
                .recent_identifiers
                .contains(&related.info.identifier)
        {
            return;
        }
    }
    if player.track.is_some() || queue_len != 0 {
        return;
    }

    let identifier = related.info.identifier.clone();
    let title = related.info.title.clone();
    related.user_data = Some(serde_json::json!({ "requester": "autoplay" }));
    if let Err(error) = player_ctx.play_now(&related).await {
        let mut details = session.details.lock().await;
        details.autoplay_failures = details.autoplay_failures.saturating_add(1);
        tracing::warn!(guild_id = %guild_id, error = %error, "autoplay play failed");
        return;
    }
    let mut details = session.details.lock().await;
    details.autoplay_failures = 0;
    details.remember_identifier(identifier);
    details.remember_track(title.clone());
    tracing::info!(guild_id = %guild_id, title = %title, "autoplay started a related track");
}

/// Spawn a background autoplay task if the preconditions hold.
///
/// Only state reads and a local queue count happen here; all track resolution
/// runs in the spawned Tokio task.
pub async fn maybe_start(
    client: &LavalinkClient,
    registry: &Arc<GuildStateRegistry>,
    guild_id: u64,
    reason: &TrackEndReason,
    track: &TrackData,
) {
    let Some(session) = registry.existing_session(guild_id) else {
        return;
    };
    let (autoplay, failures) = {
        let details = session.details.lock().await;
        (details.autoplay, details.autoplay_failures)
    };
    if !autoplay || failures >= MAX_AUTOPLAY_FAILURES {
        return;
    }
    let Some(player_ctx) = client.get_player_context(guild_id) else {
        return;
    };
    let Ok(queue_len) = player_ctx.get_queue().get_count().await else {
        return;
    };
    if !should_autoplay(autoplay, reason, session.loop_control.mode(), queue_len) {
        return;
    }

    let client = client.clone();
    let registry = registry.clone();
    let track = track.clone();
    session.autoplay.arm(|generation| {
        tokio::spawn(async move {
            run_autoplay(client, registry, guild_id, track, generation).await;
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use lavalink_rs::model::track::TrackInfo;

    fn track(id: &str, title: &str) -> TrackData {
        TrackData {
            info: TrackInfo {
                identifier: id.to_owned(),
                title: title.to_owned(),
                author: "Artist".to_owned(),
                ..TrackInfo::default()
            },
            ..TrackData::default()
        }
    }

    #[test]
    fn video_id_accepts_standard_urls_and_bare_ids() {
        assert_eq!(video_id("dQw4w9WgXcQ").as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(
            video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            video_id("https://youtu.be/dQw4w9WgXcQ?t=30").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            video_id("https://music.youtube.com/watch?v=dQw4w9WgXcQ&list=RD").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            video_id("https://www.youtube.com/shorts/dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert!(video_id("https://example.com/watch?v=dQw4w9WgXcQ").is_none());
        assert!(video_id("https://evil.com/youtu.be/dQw4w9WgXcQ").is_none());
        assert!(video_id("too-short").is_none());
    }

    #[test]
    fn mix_url_rejects_invalid_ids() {
        assert_eq!(
            mix_url("dQw4w9WgXcQ").as_deref(),
            Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=RDdQw4w9WgXcQ")
        );
        assert!(mix_url("bad id with spaces").is_none());
        assert!(mix_url("").is_none());
    }

    #[test]
    fn search_fallback_preserves_original_language() {
        assert_eq!(
            search_fallback("ボヘミアン・ラプソディ", "Queen"),
            "ytsearch:ボヘミアン・ラプソディ Queen"
        );
        assert_eq!(search_fallback("Song", "Unknown"), "ytsearch:Song");
    }

    #[test]
    fn should_autoplay_requires_natural_end_empty_queue_and_no_loop() {
        assert!(should_autoplay(
            true,
            &TrackEndReason::Finished,
            LoopMode::Off,
            0
        ));
        for reason in [
            TrackEndReason::Stopped,
            TrackEndReason::Replaced,
            TrackEndReason::Cleanup,
            TrackEndReason::LoadFailed,
        ] {
            assert!(!should_autoplay(true, &reason, LoopMode::Off, 0));
        }
        assert!(!should_autoplay(
            false,
            &TrackEndReason::Finished,
            LoopMode::Off,
            0
        ));
        assert!(!should_autoplay(
            true,
            &TrackEndReason::Finished,
            LoopMode::Track,
            0
        ));
        assert!(!should_autoplay(
            true,
            &TrackEndReason::Finished,
            LoopMode::Off,
            1
        ));
    }

    #[test]
    fn pick_candidate_skips_recent_and_streams() {
        let mut recent = VecDeque::new();
        recent.push_back("known".to_owned());
        let mut stream = track("stream", "Live");
        stream.info.is_stream = true;
        let candidates = vec![track("known", "Recent"), stream, track("fresh", "New")];
        let picked = pick_candidate(candidates, &recent).unwrap();
        assert_eq!(picked.info.identifier, "fresh");
    }

    #[test]
    fn dedupe_filters_bounded_history() {
        let recent = VecDeque::from(vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(
            dedupe(
                vec!["a".to_owned(), "c".to_owned(), "b".to_owned()],
                &recent
            ),
            vec!["c"]
        );
    }

    #[test]
    fn failure_budget_is_bounded() {
        // A zero budget would disable autoplay entirely; guard at compile time.
        const { assert!(MAX_AUTOPLAY_FAILURES > 0) };
        // Failure counting saturates at u8::MAX instead of wrapping to zero.
        let failures = u8::MAX.saturating_add(1);
        assert_eq!(failures, u8::MAX);
        assert!(failures >= MAX_AUTOPLAY_FAILURES);
    }

    #[tokio::test]
    async fn autoplay_control_rearms_and_cancels() {
        let control = AutoplayControl::default();
        let stale = control.arm(|_| tokio::spawn(std::future::pending::<()>()));
        assert!(control.is_current(stale));
        assert!(control.is_armed());
        control.cancel();
        assert!(!control.is_current(stale));
        assert!(!control.is_armed());
        let fresh = control.arm(|_| tokio::spawn(std::future::pending::<()>()));
        assert!(control.is_current(fresh));
        assert!(!control.is_current(stale));
    }

    #[test]
    fn identifier_history_is_bounded_and_drives_dedup() {
        let mut state = crate::state::GuildState::default();
        for index in 0..100 {
            state.remember_identifier(format!("id{index}"));
        }
        assert!(state.recent_identifiers.len() <= 20);
        assert!(!state.recent_identifiers.contains(&"id0".to_owned()));

        // The seed video of a Mix is recent history, so it must be skipped.
        let candidates = vec![track("id99", "Seed"), track("fresh", "Related")];
        let picked = pick_candidate(candidates, &state.recent_identifiers).unwrap();
        assert_eq!(picked.info.identifier, "fresh");
    }
}
