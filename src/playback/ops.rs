//! Shared playback operations used by slash commands and interactive controls.
//!
//! These functions contain the business logic only; they never build Discord
//! replies. Callers own reply rendering and per-guild serialization.

use lavalink_rs::prelude::PlayerContext;
use serenity::cache::Cache;
use serenity::model::id::{ChannelId, GuildId, UserId};

use crate::audio::voice::current_voice_channel;
use crate::error::BotError;
use crate::state::BotData;

/// Lower bound of the bot-facing volume range (0 = muted).
pub const MIN_VOLUME: u16 = 0;
/// Upper bound of the bot-facing volume range (100 = default level).
pub const MAX_VOLUME: u16 = 150;

/// Validate a bot-facing volume value.
pub fn validate_volume(volume: u16) -> Result<u16, BotError> {
    if (MIN_VOLUME..=MAX_VOLUME).contains(&volume) {
        Ok(volume)
    } else {
        Err(BotError::InvalidVolume(volume.to_string()))
    }
}

/// Parse a volume argument supplied as text (used by tests and future inputs).
pub fn parse_volume(input: &str) -> Result<u16, BotError> {
    let trimmed = input.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(BotError::InvalidVolume(trimmed.to_owned()));
    }
    let value = trimmed
        .parse::<u16>()
        .map_err(|_| BotError::InvalidVolume(trimmed.to_owned()))?;
    validate_volume(value)
}

/// Require that the actor and the bot occupy the same cached voice channel.
pub fn require_same_voice(
    cache: &Cache,
    guild_id: GuildId,
    user_id: UserId,
    bot_id: UserId,
) -> Result<ChannelId, BotError> {
    let user_channel =
        current_voice_channel(cache, guild_id, user_id).ok_or(BotError::NotInVoice)?;
    let bot_channel = current_voice_channel(cache, guild_id, bot_id)
        .ok_or_else(|| BotError::Voice("the bot is not connected to a voice channel".to_owned()))?;
    if user_channel != bot_channel {
        return Err(BotError::Voice(
            "you must be in the same voice channel as the bot".to_owned(),
        ));
    }
    Ok(user_channel)
}

/// Return the active Lavalink player context for a guild.
pub fn require_player(data: &BotData, guild_id: GuildId) -> Result<PlayerContext, BotError> {
    data.lavalink
        .get_player_context(guild_id.get())
        .ok_or(BotError::NoActivePlayer)
}

/// Set the player volume after validating the bot-facing range.
///
/// Lavalink uses the same 0..=1000 scale where 100 is the default level, so
/// bot-facing values map directly without rescaling.
pub async fn set_volume(data: &BotData, guild_id: GuildId, volume: u16) -> Result<u16, BotError> {
    let volume = validate_volume(volume)?;
    let player = require_player(data, guild_id)?;
    let updated = player
        .set_volume(volume)
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    tracing::info!(
        guild_id = %guild_id,
        volume = updated.volume,
        "volume changed"
    );
    Ok(updated.volume)
}

/// Set the paused state, returning whether a transition actually occurred.
pub async fn set_paused(data: &BotData, guild_id: GuildId, paused: bool) -> Result<bool, BotError> {
    let player_ctx = require_player(data, guild_id)?;
    let current = player_ctx
        .get_player()
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    if current.track.is_none() {
        return Err(BotError::NoActivePlayer);
    }
    if current.paused == paused {
        return Ok(false);
    }

    player_ctx
        .set_pause(paused)
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    tracing::info!(
        guild_id = %guild_id,
        paused = paused,
        "playback pause state changed"
    );
    Ok(true)
}

/// Resolve a user-facing seek string against the current track metadata.
pub fn resolve_seek_input(
    input: &str,
    is_seekable: bool,
    is_stream: bool,
    length_ms: u64,
) -> Result<u64, BotError> {
    let target = crate::playback::seek::parse_seek(input)
        .map_err(|error| BotError::InvalidSeek(format!("{error:?}")))?;
    crate::playback::seek::validate_seek_target(target, is_seekable, is_stream, length_ms)
        .map_err(|error| BotError::InvalidSeek(format!("{error:?}")))
}

/// Seek the active player to a previously validated millisecond offset.
pub async fn seek_to(data: &BotData, guild_id: GuildId, target_ms: u64) -> Result<u64, BotError> {
    let player_ctx = require_player(data, guild_id)?;
    let updated = player_ctx
        .set_position(std::time::Duration::from_millis(target_ms))
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    tracing::info!(
        guild_id = %guild_id,
        position_ms = updated.state.position,
        "seek applied"
    );
    Ok(updated.state.position)
}

#[cfg(test)]
mod tests {
    use super::{parse_volume, validate_volume, BotError, MAX_VOLUME, MIN_VOLUME};

    #[test]
    fn accepts_the_full_volume_range() {
        for value in MIN_VOLUME..=MAX_VOLUME {
            assert_eq!(validate_volume(value).unwrap(), value);
        }
    }

    #[test]
    fn rejects_out_of_range_volume() {
        for value in [MAX_VOLUME + 1, u16::MAX] {
            assert!(matches!(
                validate_volume(value),
                Err(BotError::InvalidVolume(_))
            ));
        }
    }

    #[test]
    fn parses_text_volume_strictly() {
        assert_eq!(parse_volume(" 150 ").unwrap(), 150);
        assert_eq!(parse_volume("0").unwrap(), 0);
        for invalid in ["", "  ", "-1", "+5", "12%", "151", "65536", "abc"] {
            assert!(
                matches!(parse_volume(invalid), Err(BotError::InvalidVolume(_))),
                "expected rejection for {invalid:?}"
            );
        }
    }
}
