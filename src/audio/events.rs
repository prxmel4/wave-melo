use lavalink_rs::model::events::{
    Ready, TrackEnd, TrackEndReason, TrackException, TrackStart, TrackStuck, WebSocketClosed,
};
use lavalink_rs::prelude::LavalinkClient;
use serenity::client::FullEvent;

use crate::audio::looping::{has_loop_marker, has_queue_cycle_marker, loop_copy};
use crate::state::{BotData, GuildStateRegistry, LoopMode};

/// Owned, `'static` future returned by every Lavalink event handler.
///
/// Handlers must clone any data they need out of the borrowed event before
/// constructing the future, because `Events` stores plain function pointers
/// returning `BoxFuture<'static, ()>`.
type HandlerFuture = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;

/// Lavalink session ready. Nothing to do beyond logging.
pub fn on_ready(_client: LavalinkClient, session_id: String, event: &Ready) -> HandlerFuture {
    let resumed = event.resumed;
    Box::pin(async move {
        tracing::info!(
            session_id = %session_id,
            resumed = resumed,
            "lavalink session ready"
        );
    })
}

/// A track started playing on the Lavalink node.
pub fn on_track_start(
    client: LavalinkClient,
    _session_id: String,
    event: &TrackStart,
) -> HandlerFuture {
    let guild_id = event.guild_id.0;
    let title = event.track.info.title.clone();
    Box::pin(async move {
        tracing::info!(
            event = "track_start",
            guild_id = %guild_id,
            title = %title,
            "track started"
        );
        if let Ok(registry) = client.data::<GuildStateRegistry>() {
            let session = registry.session(guild_id);
            let mut details = session.details.lock().await;
            details.remember_track(title);
            details.idle_generation = details.idle_generation.wrapping_add(1);
            details.bump_control_generation();
            drop(details);
            // A track started: re-evaluate the idle countdown.
            if let Some(deps) = crate::audio::idle::IdleDeps::from_registry(&client, &registry) {
                crate::audio::idle::refresh(deps, guild_id).await;
            }
        }
    })
}

/// A track ended. lavalink-rs advances its own queue via `player.finish()`
/// before dispatching this event, so loop copies are injected synchronously
/// here: they are queued before the player's `StartTrack` message is handled.
pub fn on_track_end(
    client: LavalinkClient,
    _session_id: String,
    event: &TrackEnd,
) -> HandlerFuture {
    let guild_id = event.guild_id.0;
    let reason = event.reason.clone();
    let track = event.track.clone();
    Box::pin(async move {
        tracing::info!(
            event = "track_end",
            guild_id = %guild_id,
            reason = ?reason,
            "track ended"
        );

        let Ok(registry) = client.data::<GuildStateRegistry>() else {
            return;
        };
        let Some(session) = registry.existing_session(guild_id) else {
            return;
        };

        // Loop replenishment only applies to natural ends; the idle refresh
        // below applies to every end reason (including manual stops).
        if reason == TrackEndReason::Finished {
            if let Some(player_ctx) = client.get_player_context(guild_id) {
                let queue = player_ctx.get_queue();
                let data = track.user_data.as_ref();
                match session.loop_control.mode() {
                    LoopMode::Off => {}
                    LoopMode::Track => {
                        // Only replenish injected copies; the first natural end is
                        // covered by the two copies queued when track loop was enabled.
                        if has_loop_marker(data) {
                            if let Err(error) = queue.push_to_front(loop_copy(&track)) {
                                tracing::warn!(guild_id = %guild_id, error = %error, "failed to replenish track loop");
                            }
                        }
                    }
                    LoopMode::Queue => {
                        // The final entry of each injected cycle carries the marker.
                        if has_queue_cycle_marker(data) {
                            let cycle = session.loop_control.snapshot();
                            if cycle.is_empty() {
                                tracing::warn!(guild_id = %guild_id, "queue loop snapshot is empty");
                            } else if let Err(error) = queue.append(cycle) {
                                tracing::warn!(guild_id = %guild_id, error = %error, "failed to replenish queue loop");
                            }
                        }
                    }
                }
            }
        }

        // Any track end can leave the player idle: re-evaluate the countdown.
        if let Some(deps) = crate::audio::idle::IdleDeps::from_registry(&client, &registry) {
            crate::audio::idle::refresh(deps, guild_id).await;
        }
    })
}

/// A track threw an exception on the Lavalink node.
pub fn on_track_exception(
    _client: LavalinkClient,
    _session_id: String,
    event: &TrackException,
) -> HandlerFuture {
    let guild_id = event.guild_id.0;
    let message = event.exception.message.clone();
    let severity = event.exception.severity.clone();
    Box::pin(async move {
        tracing::warn!(
            event = "track_exception",
            guild_id = %guild_id,
            severity = %severity,
            message = %message,
            "track exception"
        );
    })
}

/// A track got stuck (buffer underrun exceeded threshold).
pub fn on_track_stuck(
    _client: LavalinkClient,
    _session_id: String,
    event: &TrackStuck,
) -> HandlerFuture {
    let guild_id = event.guild_id.0;
    let threshold = event.threshold_ms;
    Box::pin(async move {
        tracing::warn!(
            event = "track_stuck",
            guild_id = %guild_id,
            threshold_ms = threshold,
            "track stuck"
        );
    })
}

/// The audio WebSocket to Discord was closed.
pub fn on_websocket_closed(
    _client: LavalinkClient,
    _session_id: String,
    event: &WebSocketClosed,
) -> HandlerFuture {
    let guild_id = event.guild_id.0;
    let code = event.code;
    let reason = event.reason.clone();
    let by_remote = event.by_remote;
    Box::pin(async move {
        tracing::warn!(
            event = "websocket_closed",
            guild_id = %guild_id,
            code = code,
            reason = %reason,
            by_remote = by_remote,
            "discord voice websocket closed"
        );
    })
}

/// Forward Discord voice gateway events into lavalink-rs.
///
/// lavalink-rs filters voice state updates by its own user ID, accumulates the
/// session ID, endpoint and token per guild, and exposes them through
/// `LavalinkClient::get_connection_info`.
pub async fn handle_voice_event(
    event: &FullEvent,
    ctx: &serenity::client::Context,
    data: &BotData,
    bot_id: serenity::model::id::UserId,
) {
    match event {
        FullEvent::VoiceStateUpdate { new, .. } => {
            let Some(guild_id) = new.guild_id else {
                return;
            };

            data.lavalink.handle_voice_state_update(
                guild_id,
                new.channel_id,
                new.user_id,
                new.session_id.clone(),
            );

            // If the bot itself was forcibly disconnected, drop its player.
            // Other users' disconnects must never touch the bot's player.
            if new.user_id == bot_id
                && new.channel_id.is_none()
                && data.lavalink.get_player_context(guild_id.get()).is_some()
            {
                tracing::info!(guild_id = %guild_id, "bot removed from voice channel");
                if let Err(error) = data.lavalink.delete_player(guild_id.get()).await {
                    tracing::warn!(guild_id = %guild_id, error = %error, "failed to clean up player");
                }
            }

            // Any voice state change may affect the idle countdown.
            let deps = crate::audio::idle::IdleDeps::from_bot_data(data, ctx);
            crate::audio::idle::refresh(deps, guild_id.get()).await;
        }
        FullEvent::VoiceServerUpdate { event } => {
            let Some(guild_id) = event.guild_id else {
                return;
            };
            data.lavalink.handle_voice_server_update(
                guild_id,
                event.token.clone(),
                event.endpoint.clone(),
            );
        }
        _ => {}
    }
}
