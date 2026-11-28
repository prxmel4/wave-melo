use std::sync::Arc;

use lavalink_rs::model::events::Events;
use lavalink_rs::prelude::{LavalinkClient, NodeBuilder, NodeDistributionStrategy};

use crate::audio::events as audio_events;
use crate::config::AppConfig;
use crate::error::BotError;
use crate::state::GuildStateRegistry;

/// Build and connect a Lavalink client using the supplied configuration.
///
/// The registry is shared with event handlers so they can coordinate loop
/// injection and track-end decisions without holding Discord state.
pub async fn build_lavalink_client(
    config: &AppConfig,
    user_id: u64,
    registry: Arc<GuildStateRegistry>,
) -> Result<LavalinkClient, BotError> {
    let events = Events {
        ready: Some(audio_events::on_ready),
        track_start: Some(audio_events::on_track_start),
        track_end: Some(audio_events::on_track_end),
        track_exception: Some(audio_events::on_track_exception),
        track_stuck: Some(audio_events::on_track_stuck),
        websocket_closed: Some(audio_events::on_websocket_closed),
        ..Default::default()
    };

    let node = NodeBuilder {
        hostname: config.lavalink_address(),
        is_ssl: config.lavalink_secure,
        // Handlers are registered on the client only; registering them on both
        // node and client would dispatch every event twice.
        events: Events::default(),
        password: config.lavalink_password.clone(),
        user_id: lavalink_rs::prelude::UserId(user_id),
        session_id: None,
    };

    let client = LavalinkClient::new_with_data(
        events,
        vec![node],
        NodeDistributionStrategy::MainFallback,
        registry,
    )
    .await;

    tracing::info!(
        address = %config.lavalink_address(),
        secure = config.lavalink_secure,
        user_id = user_id,
        "lavalink client initialized"
    );
    Ok(client)
}
