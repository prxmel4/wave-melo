//! Smart auto-leave: a cancellable 180-second inactivity timer per guild.
//!
//! The countdown is armed when the bot is alone in its voice channel or idle
//! with an empty queue, and cancelled the moment a listener returns, a track
//! starts, or 24/7 mode is enabled. Every armed task carries a generation so
//! stale timers can never disconnect an active session.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use lavalink_rs::prelude::LavalinkClient;
use serenity::model::id::{ChannelId, GuildId, UserId};
use tokio::task::JoinHandle;

use crate::audio::voice::{current_voice_channel, send_voice_state_update};
use crate::state::{BotData, GuildSession, GuildStateRegistry, RuntimeHandles};

/// How long an inactive session may linger before the bot leaves.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(180);

/// Pure decision: should an idle countdown be running for this session?
///
/// Alone in the channel always counts down (even mid-track); with listeners
/// present only a fully idle player with an empty queue counts down. 24/7
/// mode suppresses the countdown entirely.
pub fn leave_required(mode_247: bool, listeners: usize, playing: bool, queue_len: usize) -> bool {
    !mode_247 && (listeners == 0 || (!playing && queue_len == 0))
}

/// Count non-bot listeners in a channel from raw voice-state tuples of
/// `(user_id, channel_id, is_bot)`. Unknown members are treated as human so
/// the bot never leaves while someone might still be listening.
pub fn count_listeners(
    states: impl IntoIterator<Item = (u64, Option<u64>, bool)>,
    channel_id: u64,
    bot_id: u64,
) -> usize {
    states
        .into_iter()
        .filter(|(user, channel, is_bot)| {
            *user != bot_id && !is_bot && *channel == Some(channel_id)
        })
        .count()
}

/// Read the listener count from the cache; `None` when the guild is unknown.
fn listeners_in_cache(
    cache: &serenity::cache::Cache,
    guild_id: GuildId,
    channel_id: ChannelId,
    bot_id: UserId,
) -> Option<usize> {
    let guild = cache.guild(guild_id)?;
    Some(count_listeners(
        guild.voice_states.values().map(|state| {
            (
                state.user_id.get(),
                state.channel_id.map(|id| id.get()),
                state.member.as_ref().is_some_and(|member| member.user.bot),
            )
        }),
        channel_id.get(),
        bot_id.get(),
    ))
}

/// Cancellable timer slot shared between commands and event handlers.
///
/// The generation is an atomic so expiry checks never block; the task slot
/// uses a short-lived standard mutex that is never held across an `.await`.
#[derive(Debug, Clone, Default)]
pub struct IdleControl {
    inner: Arc<IdleInner>,
}

#[derive(Debug, Default)]
struct IdleInner {
    generation: AtomicU64,
    task: StdMutex<Option<JoinHandle<()>>>,
}

impl IdleControl {
    /// Current generation; every arm or cancel advances it.
    pub fn generation(&self) -> u64 {
        self.inner.generation.load(Ordering::SeqCst)
    }

    /// Whether a captured generation still owns the countdown.
    pub fn is_current(&self, generation: u64) -> bool {
        self.generation() == generation
    }

    /// Whether a live countdown task is installed.
    pub fn is_armed(&self) -> bool {
        self.inner
            .task
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    /// Arm a fresh countdown, aborting any previous timer atomically.
    ///
    /// The closure receives the new generation and spawns the timer task;
    /// it runs synchronously under the slot lock so no stale install can
    /// interleave with a concurrent cancel.
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

    /// Cancel any armed countdown and invalidate its generation.
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
}

/// Owned handles a background timer needs; no borrows escape the event.
#[derive(Clone)]
pub struct IdleDeps {
    pub lavalink: LavalinkClient,
    pub registry: Arc<GuildStateRegistry>,
    pub handles: Arc<RuntimeHandles>,
}

impl IdleDeps {
    /// Build from command/gateway context (Serenity handles are Arc-cheap).
    pub fn from_bot_data(data: &BotData, ctx: &serenity::client::Context) -> Self {
        Self {
            lavalink: data.lavalink.clone(),
            registry: data.guild_states.clone(),
            handles: Arc::new(RuntimeHandles {
                shard: ctx.shard.clone(),
                cache: ctx.cache.clone(),
                guild_ops: data.guild_ops.clone(),
            }),
        }
    }

    /// Build inside a Lavalink event handler using the installed runtime.
    pub fn from_registry(
        client: &LavalinkClient,
        registry: &Arc<GuildStateRegistry>,
    ) -> Option<Self> {
        Some(Self {
            lavalink: client.clone(),
            registry: registry.clone(),
            handles: Arc::new(registry.runtime()?),
        })
    }
}

/// Re-evaluate the idle countdown after a voice or playback transition.
pub async fn refresh(deps: IdleDeps, guild_id: u64) {
    let Some(session) = deps.registry.existing_session(guild_id) else {
        return;
    };
    let Some(player_ctx) = deps.lavalink.get_player_context(guild_id) else {
        session.idle.cancel();
        return;
    };
    let mode_247 = session.details.lock().await.mode_247;
    if mode_247 {
        session.idle.cancel();
        return;
    }

    let guild = GuildId::new(guild_id);
    let bot_id = deps.handles.cache.current_user().id;
    let Some(channel_id) = current_voice_channel(&deps.handles.cache, guild, bot_id) else {
        session.idle.cancel();
        return;
    };
    let Some(listeners) = listeners_in_cache(&deps.handles.cache, guild, channel_id, bot_id) else {
        return;
    };

    // Alone: the countdown applies regardless of playback state, so no
    // remote reads are needed. With listeners present, only a fully idle
    // player with an empty queue qualifies.
    let required = if listeners == 0 {
        true
    } else {
        let Ok(player) = player_ctx.get_player().await else {
            return;
        };
        let Ok(queue_len) = player_ctx.get_queue().get_count().await else {
            return;
        };
        leave_required(false, listeners, player.track.is_some(), queue_len)
    };

    if required {
        // Keep an existing countdown running so repeated voice events do
        // not endlessly postpone the deadline.
        if !session.idle.is_armed() {
            let generation = session.idle.arm(|generation| {
                let deps = deps.clone();
                let session = session.clone();
                tokio::spawn(run_timer(deps, session, guild_id, generation))
            });
            tracing::info!(guild_id = %guild_id, generation, "idle countdown armed");
        }
    } else {
        session.idle.cancel();
    }
}

/// Per-guild operation lock from the shared map (same one commands use).
async fn guild_op_lock(handles: &RuntimeHandles, guild_id: GuildId) -> Arc<tokio::sync::Mutex<()>> {
    handles
        .guild_ops
        .lock()
        .await
        .entry(guild_id)
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// The 180-second countdown body. Re-verifies everything before disconnecting.
async fn run_timer(deps: IdleDeps, session: Arc<GuildSession>, guild_id: u64, generation: u64) {
    tokio::time::sleep(IDLE_TIMEOUT).await;

    // Stale timers must never touch a live or rebuilt session.
    if !session.idle.is_current(generation) {
        return;
    }
    let Some(current) = deps.registry.existing_session(guild_id) else {
        return;
    };
    if !Arc::ptr_eq(&current, &session) {
        return;
    }
    let Some(player_ctx) = deps.lavalink.get_player_context(guild_id) else {
        return;
    };

    // Serialize with slash commands, then re-verify under the lock.
    let guild = GuildId::new(guild_id);
    let lock = guild_op_lock(&deps.handles, guild).await;
    let _guard = lock.lock().await;
    if !session.idle.is_current(generation) {
        return;
    }
    let mode_247 = session.details.lock().await.mode_247;
    if mode_247 {
        return;
    }
    let bot_id = deps.handles.cache.current_user().id;
    let Some(channel_id) = current_voice_channel(&deps.handles.cache, guild, bot_id) else {
        return;
    };
    let Some(listeners) = listeners_in_cache(&deps.handles.cache, guild, channel_id, bot_id) else {
        return;
    };
    let Ok(player) = player_ctx.get_player().await else {
        return;
    };
    let Ok(queue_len) = player_ctx.get_queue().get_count().await else {
        return;
    };
    if !leave_required(mode_247, listeners, player.track.is_some(), queue_len) {
        return;
    }

    tracing::info!(guild_id = %guild_id, "idle timeout reached; leaving voice");
    if let Err(error) = deps.lavalink.delete_player(guild_id).await {
        tracing::warn!(guild_id = %guild_id, error = %error, "failed to delete idle player");
    }
    if let Err(error) = send_voice_state_update(&deps.handles.shard, guild, None, false, false) {
        tracing::warn!(guild_id = %guild_id, error = %error, "failed to send idle disconnect");
    }
    drop(_guard);
    deps.handles.guild_ops.lock().await.remove(&guild);
    deps.registry.remove(guild_id);
}

#[cfg(test)]
mod tests {
    use super::{count_listeners, leave_required, IdleControl, IDLE_TIMEOUT};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn timeout_is_three_minutes() {
        assert_eq!(IDLE_TIMEOUT, Duration::from_secs(180));
    }

    #[test]
    fn decision_table_covers_every_trigger() {
        // 24/7 mode suppresses every countdown.
        assert!(!leave_required(true, 0, false, 0));
        assert!(!leave_required(true, 0, true, 5));
        // Alone always counts down, even mid-track.
        assert!(leave_required(false, 0, true, 5));
        assert!(leave_required(false, 0, false, 0));
        // Listeners present: only a fully idle, empty session counts down.
        assert!(leave_required(false, 2, false, 0));
        assert!(!leave_required(false, 2, false, 1));
        assert!(!leave_required(false, 2, true, 0));
        assert!(!leave_required(false, 1, true, 9));
    }

    #[test]
    fn listener_count_excludes_bots_and_other_channels() {
        let states = [
            (1, Some(10), false),  // human in target channel
            (2, Some(10), true),   // bot in target channel
            (3, Some(20), false),  // human elsewhere
            (4, None, false),      // disconnected human
            (99, Some(10), false), // the bot itself
            (5, Some(10), false),  // another human in target
        ];
        assert_eq!(count_listeners(states, 10, 99), 2);
        assert_eq!(count_listeners([], 10, 99), 0);
    }

    #[tokio::test]
    async fn arming_replaces_and_invalidates_the_previous_generation() {
        let control = IdleControl::default();
        assert!(!control.is_armed());

        let first = control.arm(|_| tokio::spawn(std::future::pending::<()>()));
        assert!(control.is_armed());
        assert!(control.is_current(first));

        let second = control.arm(|_| tokio::spawn(std::future::pending::<()>()));
        assert_ne!(first, second);
        assert!(!control.is_current(first));
        assert!(control.is_current(second));
        assert!(control.is_armed());
    }

    #[tokio::test]
    async fn cancel_aborts_the_armed_task() {
        struct DropFlag(Arc<AtomicBool>);
        impl Drop for DropFlag {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let control = IdleControl::default();
        let dropped = Arc::new(AtomicBool::new(false));
        let flag = DropFlag(dropped.clone());
        let generation = control.arm(|_| {
            tokio::spawn(async move {
                let _flag = flag;
                std::future::pending::<()>().await;
            })
        });
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert!(!dropped.load(Ordering::SeqCst));

        control.cancel();
        assert!(!control.is_current(generation));
        assert!(!control.is_armed());
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert!(dropped.load(Ordering::SeqCst), "cancelled task was dropped");
    }

    #[tokio::test]
    async fn stale_generations_never_match_after_rearm() {
        let control = IdleControl::default();
        let stale = control.arm(|_| tokio::spawn(std::future::pending::<()>()));
        control.cancel();
        let fresh = control.arm(|_| tokio::spawn(std::future::pending::<()>()));
        assert!(!control.is_current(stale));
        assert!(control.is_current(fresh));
        control.cancel();
        assert!(!control.is_current(fresh));
    }

    #[tokio::test]
    async fn finished_tasks_do_not_count_as_armed() {
        let control = IdleControl::default();
        control.arm(|_| tokio::spawn(async {}));
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert!(!control.is_armed());
    }

    #[tokio::test]
    async fn registry_removal_cancels_the_countdown() {
        let registry = crate::state::GuildStateRegistry::default();
        let session = registry.session(7);
        let generation = session
            .idle
            .arm(|_| tokio::spawn(std::future::pending::<()>()));
        assert!(session.idle.is_armed());

        registry.remove(7);
        assert!(!session.idle.is_armed());
        assert!(!session.idle.is_current(generation));
    }
}
