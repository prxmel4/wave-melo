use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex, RwLock as StdRwLock};
use std::time::Instant;

use lavalink_rs::player_context::TrackInQueue;
use lavalink_rs::prelude::LavalinkClient;
use serenity::model::id::GuildId;
use tokio::sync::Mutex;

use crate::audio::autoplay::AutoplayControl;
use crate::audio::idle::IdleControl;
use crate::config::AppConfig;

/// Hard cap for cached loop snapshots and track history.
const MAX_CACHED_TRACKS: usize = 200;
const MAX_RECENT_TRACKS: usize = 20;

/// Repeat behavior for a guild's playback session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoopMode {
    #[default]
    Off,
    Track,
    Queue,
}

impl LoopMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Track => "track",
            Self::Queue => "queue",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "track" => Some(Self::Track),
            "queue" => Some(Self::Queue),
            _ => None,
        }
    }

    fn as_u8(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Track => 1,
            Self::Queue => 2,
        }
    }

    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Off),
            1 => Some(Self::Track),
            2 => Some(Self::Queue),
            _ => None,
        }
    }
}

/// Pure state-machine transition used by `/loop`.
pub fn next_loop_mode(current: LoopMode) -> LoopMode {
    match current {
        LoopMode::Off => LoopMode::Track,
        LoopMode::Track => LoopMode::Queue,
        LoopMode::Queue => LoopMode::Off,
    }
}

/// Pure decision for a natural track completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopDecision {
    Advance,
    Replay,
    RestartQueue,
    Stop,
}

pub fn loop_decision(mode: LoopMode, is_last: bool, queue_len: usize) -> LoopDecision {
    if !is_last {
        return LoopDecision::Advance;
    }
    match mode {
        LoopMode::Off => LoopDecision::Stop,
        LoopMode::Track => LoopDecision::Replay,
        LoopMode::Queue if queue_len > 0 => LoopDecision::RestartQueue,
        LoopMode::Queue => LoopDecision::Stop,
    }
}

/// Synchronous loop controls shared with Lavalink's event dispatcher.
///
/// The mode is an atomic so `on_track_end` can read it without awaiting. The
/// queue snapshot is protected by a short-lived standard mutex that is never
/// held across an `.await` point or network operation.
#[derive(Debug, Clone)]
pub struct LoopControl {
    mode: Arc<AtomicU8>,
    snapshot: Arc<StdMutex<VecDeque<TrackInQueue>>>,
}

impl Default for LoopControl {
    fn default() -> Self {
        Self {
            mode: Arc::new(AtomicU8::new(LoopMode::Off.as_u8())),
            snapshot: Arc::new(StdMutex::new(VecDeque::new())),
        }
    }
}

impl LoopControl {
    /// Read the current mode without blocking or awaiting.
    pub fn mode(&self) -> LoopMode {
        LoopMode::from_u8(self.mode.load(Ordering::SeqCst)).unwrap_or(LoopMode::Off)
    }

    /// Change the mode and clear the snapshot when looping is disabled.
    pub fn set_mode(&self, mode: LoopMode) {
        self.mode.store(mode.as_u8(), Ordering::SeqCst);
        if mode == LoopMode::Off {
            self.clear_snapshot();
        }
    }

    /// Return a bounded clone of the saved queue snapshot.
    pub fn snapshot(&self) -> VecDeque<TrackInQueue> {
        self.snapshot
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    /// Replace the saved queue snapshot with a bounded copy.
    pub fn set_snapshot(&self, mut tracks: VecDeque<TrackInQueue>) {
        tracks.truncate(MAX_CACHED_TRACKS);
        *self
            .snapshot
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = tracks;
    }

    /// Clear the saved queue snapshot.
    pub fn clear_snapshot(&self) {
        self.snapshot
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
    }
}

/// Coordination metadata that Lavalink does not expose.
///
/// Remote playback truth remains in Lavalink; this struct stores local state
/// such as 24/7 mode, autoplay, and bounded history. Loop mode and its queue
/// snapshot live in [`LoopControl`] so event handlers can read them safely.
#[derive(Debug, Clone)]
pub struct GuildState {
    pub volume: u16,
    pub active_filter: Option<String>,
    pub mode_247: bool,
    pub autoplay: bool,
    /// Consecutive autoplay resolution failures (resets on success or toggle).
    pub autoplay_failures: u8,
    pub recent_tracks: VecDeque<String>,
    /// Identifier-based history for autoplay dedup (bounded, separate from titles).
    pub recent_identifiers: VecDeque<String>,
    pub idle_generation: u64,
    /// Invalidates interactive controls after a playback transition.
    pub control_generation: u64,
    /// Set by manual transitions so event handlers do not treat them as natural ends.
    pub manual_transition: bool,
    /// Deduplicates a repeated TrackStart for the same injected loop copy.
    pub last_loop_start: Option<(String, LoopMode, Instant)>,
}

impl Default for GuildState {
    fn default() -> Self {
        Self {
            volume: 100,
            active_filter: None,
            mode_247: false,
            autoplay: false,
            autoplay_failures: 0,
            recent_tracks: VecDeque::new(),
            recent_identifiers: VecDeque::new(),
            idle_generation: 0,
            control_generation: 0,
            manual_transition: false,
            last_loop_start: None,
        }
    }
}

impl GuildState {
    /// Remember a track identifier for autoplay dedup with a fixed upper bound.
    pub fn remember_identifier(&mut self, identifier: String) {
        if self.recent_identifiers.back() == Some(&identifier) {
            return;
        }
        self.recent_identifiers.push_back(identifier);
        while self.recent_identifiers.len() > MAX_RECENT_TRACKS {
            self.recent_identifiers.pop_front();
        }
    }

    /// Remember a display title with a fixed upper bound.
    pub fn remember_track(&mut self, title: String) {
        if self.recent_tracks.back() == Some(&title) {
            return;
        }
        self.recent_tracks.push_back(title);
        while self.recent_tracks.len() > MAX_RECENT_TRACKS {
            self.recent_tracks.pop_front();
        }
    }

    /// Mark the most recent transition as manual.
    pub fn mark_manual(&mut self) {
        self.manual_transition = true;
    }

    /// Read the generation embedded in interactive controls.
    pub fn control_generation(&self) -> u64 {
        self.control_generation
    }

    /// Invalidate every previously issued interactive control.
    pub fn bump_control_generation(&mut self) {
        self.control_generation = self.control_generation.wrapping_add(1);
    }

    /// Consume the manual-transition flag.
    pub fn take_manual(&mut self) -> bool {
        std::mem::take(&mut self.manual_transition)
    }
}

/// All state belonging to one guild.
#[derive(Debug, Clone)]
pub struct GuildSession {
    pub loop_control: LoopControl,
    pub details: Arc<Mutex<GuildState>>,
    /// Cancellable idle countdown for auto-leave.
    pub idle: IdleControl,
    /// Cancellable autoplay task for related-track resolution.
    pub autoplay: AutoplayControl,
}

impl Default for GuildSession {
    fn default() -> Self {
        Self {
            loop_control: LoopControl::default(),
            details: Arc::new(Mutex::new(GuildState::default())),
            idle: IdleControl::default(),
            autoplay: AutoplayControl::default(),
        }
    }
}

/// Runtime handles shared with background tasks and event handlers.
///
/// Serenity's `Context` is per-event; these Arc-wrapped clones let timers
/// and Lavalink callbacks reach the cache and shard messenger.
#[derive(Clone)]
pub struct RuntimeHandles {
    pub shard: serenity::gateway::ShardMessenger,
    pub cache: Arc<serenity::cache::Cache>,
    pub guild_ops: Arc<Mutex<HashMap<GuildId, Arc<Mutex<()>>>>>,
}

impl std::fmt::Debug for RuntimeHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeHandles").finish_non_exhaustive()
    }
}

/// Per-guild session registry shared with Lavalink event handlers.
#[derive(Clone, Default)]
pub struct GuildStateRegistry {
    sessions: Arc<StdRwLock<HashMap<u64, Arc<GuildSession>>>>,
    /// Runtime handles installed after the bot is ready.
    handles: Arc<StdRwLock<Option<RuntimeHandles>>>,
}

impl GuildStateRegistry {
    /// Return the session for a guild, creating it on first use.
    pub fn session(&self, guild_id: u64) -> Arc<GuildSession> {
        {
            let sessions = self
                .sessions
                .read()
                .unwrap_or_else(|error| error.into_inner());
            if let Some(session) = sessions.get(&guild_id) {
                return session.clone();
            }
        }

        let mut sessions = self
            .sessions
            .write()
            .unwrap_or_else(|error| error.into_inner());
        sessions
            .entry(guild_id)
            .or_insert_with(|| Arc::new(GuildSession::default()))
            .clone()
    }

    /// Return an existing session without creating one.
    pub fn existing_session(&self, guild_id: u64) -> Option<Arc<GuildSession>> {
        self.sessions
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .get(&guild_id)
            .cloned()
    }

    /// Drop all state for a guild after a disconnect.
    pub fn remove(&self, guild_id: u64) {
        if let Some(session) = self
            .sessions
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&guild_id)
        {
            session.idle.cancel();
            session.autoplay.cancel();
        }
    }

    /// Install runtime handles so event handlers and timers can reach the cache.
    pub fn install_handles(&self, handles: RuntimeHandles) {
        *self
            .handles
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Some(handles);
    }

    /// Read the installed runtime handles, if any.
    pub fn runtime(&self) -> Option<RuntimeHandles> {
        self.handles
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

impl std::fmt::Debug for GuildStateRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuildStateRegistry").finish_non_exhaustive()
    }
}

/// Shared process-wide state passed to every Poise command.
#[derive(Clone)]
pub struct BotData {
    pub config: Arc<AppConfig>,
    pub lavalink: LavalinkClient,
    /// Per-guild operation locks used to serialize playback transitions.
    pub guild_ops: Arc<Mutex<HashMap<GuildId, Arc<Mutex<()>>>>>,
    /// Per-guild application state, also available to Lavalink event handlers.
    pub guild_states: Arc<GuildStateRegistry>,
}

impl std::fmt::Debug for BotData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BotData")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl BotData {
    /// Return the per-guild operation lock, creating it on first use.
    pub async fn guild_lock(&self, guild_id: GuildId) -> Arc<Mutex<()>> {
        self.guild_ops
            .lock()
            .await
            .entry(guild_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Drop all mutable state associated with a guild after disconnect.
    pub async fn remove_guild_state(&self, guild_id: GuildId) {
        self.guild_ops.lock().await.remove(&guild_id);
        self.guild_states.remove(guild_id.get());
    }
}

#[cfg(test)]
mod tests {
    use super::{
        loop_decision, next_loop_mode, GuildState, GuildStateRegistry, LoopControl, LoopDecision,
        LoopMode, MAX_CACHED_TRACKS,
    };
    use lavalink_rs::model::track::TrackData;
    use lavalink_rs::player_context::TrackInQueue;
    use std::collections::VecDeque;
    use std::sync::Arc;

    #[test]
    fn loop_mode_parses_and_cycles() {
        for (input, expected) in [
            ("off", LoopMode::Off),
            (" TRACK ", LoopMode::Track),
            ("Queue", LoopMode::Queue),
        ] {
            assert_eq!(LoopMode::parse(input), Some(expected));
        }
        assert_eq!(LoopMode::parse("invalid"), None);

        assert_eq!(next_loop_mode(LoopMode::Off), LoopMode::Track);
        assert_eq!(next_loop_mode(LoopMode::Track), LoopMode::Queue);
        assert_eq!(next_loop_mode(LoopMode::Queue), LoopMode::Off);
    }

    #[test]
    fn loop_decisions_cover_boundaries() {
        assert_eq!(
            loop_decision(LoopMode::Off, false, 3),
            LoopDecision::Advance
        );
        assert_eq!(loop_decision(LoopMode::Off, true, 0), LoopDecision::Stop);
        assert_eq!(
            loop_decision(LoopMode::Track, true, 0),
            LoopDecision::Replay
        );
        assert_eq!(
            loop_decision(LoopMode::Queue, true, 2),
            LoopDecision::RestartQueue
        );
        assert_eq!(loop_decision(LoopMode::Queue, true, 0), LoopDecision::Stop);
    }

    #[test]
    fn loop_control_mode_is_atomic_and_snapshot_is_bounded() {
        let control = LoopControl::default();
        assert_eq!(control.mode(), LoopMode::Off);

        control.set_mode(LoopMode::Queue);
        assert_eq!(control.mode(), LoopMode::Queue);

        let tracks: VecDeque<TrackInQueue> = (0..MAX_CACHED_TRACKS + 10)
            .map(|index| {
                let track = TrackData {
                    encoded: index.to_string(),
                    ..TrackData::default()
                };
                track.into()
            })
            .collect();
        control.set_snapshot(tracks);
        assert_eq!(control.snapshot().len(), MAX_CACHED_TRACKS);

        control.set_mode(LoopMode::Off);
        assert!(control.snapshot().is_empty());
    }

    #[test]
    fn registry_reuses_sessions_and_forgets_them() {
        let registry = GuildStateRegistry::default();
        let first = registry.session(42);
        let second = registry.session(42);
        assert!(Arc::ptr_eq(&first, &second));
        assert!(registry.existing_session(42).is_some());

        registry.remove(42);
        assert!(registry.existing_session(42).is_none());
    }

    #[test]
    fn recent_tracks_are_bounded_and_deduplicated() {
        let mut state = GuildState::default();
        for index in 0..100 {
            state.remember_track(index.to_string());
        }
        assert!(state.recent_tracks.len() <= 20);
        assert_eq!(state.recent_tracks.back().unwrap(), "99");
    }

    #[test]
    fn control_generation_invalidates_previous_buttons() {
        let mut state = GuildState::default();
        assert_eq!(state.control_generation(), 0);
        state.bump_control_generation();
        assert_eq!(state.control_generation(), 1);
        let second = state.control_generation();
        state.bump_control_generation();
        assert_ne!(state.control_generation(), second);
    }
}
