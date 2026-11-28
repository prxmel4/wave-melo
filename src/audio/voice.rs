use std::time::Duration;

use lavalink_rs::model::http::UpdatePlayer;
use lavalink_rs::model::player::ConnectionInfo;
use lavalink_rs::prelude::PlayerContext;
use serenity::cache::Cache;
use serenity::model::id::{ChannelId, GuildId, UserId};
use tokio_tungstenite::tungstenite::Message;

use crate::error::BotError;
use crate::state::BotData;

/// How long to wait for Discord's voice handshake events.
const VOICE_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Return the voice channel a user currently occupies in a guild, if any.
pub fn current_voice_channel(
    cache: &Cache,
    guild_id: GuildId,
    user_id: UserId,
) -> Option<ChannelId> {
    cache.guild(guild_id).and_then(|guild| {
        guild
            .voice_states
            .get(&user_id)
            .and_then(|state| state.channel_id)
    })
}

/// Validate that a cached guild knows the given voice channel.
pub fn channel_is_cached(cache: &Cache, guild_id: GuildId, channel_id: ChannelId) -> bool {
    cache
        .guild(guild_id)
        .is_some_and(|guild| guild.channels.contains_key(&channel_id))
}

/// Send a voice state update (opcode 4) to join, move, or leave a voice channel.
pub fn send_voice_state_update(
    ctx: &serenity::client::Context,
    guild_id: GuildId,
    channel_id: Option<ChannelId>,
    self_deaf: bool,
    self_mute: bool,
) -> Result<(), BotError> {
    let payload = serde_json::json!({
        "op": 4,
        "d": {
            "guild_id": guild_id.get().to_string(),
            "channel_id": channel_id.map(|channel| channel.get().to_string()),
            "self_mute": self_mute,
            "self_deaf": self_deaf
        }
    });

    let json =
        serde_json::to_string(&payload).map_err(|error| BotError::Voice(error.to_string()))?;
    ctx.shard.websocket_message(Message::Text(json));
    Ok(())
}

/// Wait for both voice gateway events and return fresh connection info.
async fn wait_connection_info(
    data: &BotData,
    guild_id: GuildId,
) -> Result<ConnectionInfo, BotError> {
    data.lavalink
        .get_connection_info(guild_id, VOICE_HANDSHAKE_TIMEOUT)
        .await
        .map_err(|error| BotError::Voice(format!("voice handshake failed: {error}")))
}

/// Ensure a Lavalink player context exists for the guild.
///
/// - No context yet: join the channel and create the player (with its queue).
/// - Context exists but the bot is in another channel: move the bot and
///   refresh the player's voice connection, preserving queue and track.
/// - Context exists in the requested channel: reuse it unchanged.
pub async fn ensure_player_context(
    ctx: &serenity::client::Context,
    data: &BotData,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<PlayerContext, BotError> {
    if let Some(player) = data.lavalink.get_player_context(guild_id.get()) {
        let bot_id = ctx.cache.current_user().id;
        if current_voice_channel(&ctx.cache, guild_id, bot_id) == Some(channel_id) {
            return Ok(player);
        }

        // Move the bot; Discord re-sends the voice handshake events, which
        // lavalink-rs accumulates for `get_connection_info`.
        send_voice_state_update(ctx, guild_id, Some(channel_id), true, false)?;
        let info = wait_connection_info(data, guild_id).await?;
        data.lavalink
            .update_player(
                guild_id,
                &UpdatePlayer {
                    voice: Some(info),
                    ..Default::default()
                },
                true,
            )
            .await
            .map_err(|error| BotError::Lavalink(error.to_string()))?;
        return Ok(player);
    }

    send_voice_state_update(ctx, guild_id, Some(channel_id), true, false)?;
    let info = wait_connection_info(data, guild_id).await?;
    data.lavalink
        .create_player_context(guild_id, info)
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))
}
