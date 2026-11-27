use poise::Context;
use serenity::cache::Cache;
use serenity::model::id::{ChannelId, GuildId, UserId};

use crate::error::BotError;
use crate::state::BotData;

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

/// Resolve the command author's voice channel, failing clearly when absent.
pub fn author_voice_channel(
    ctx: Context<'_, BotData, BotError>,
) -> Result<(GuildId, ChannelId), BotError> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))?;
    let channel_id = current_voice_channel(ctx.cache(), guild_id, ctx.author().id)
        .ok_or(BotError::NotInVoice)?;
    Ok((guild_id, channel_id))
}

/// Validate that a cached guild knows the given voice channel.
pub fn channel_is_cached(cache: &Cache, guild_id: GuildId, channel_id: ChannelId) -> bool {
    cache
        .guild(guild_id)
        .and_then(|guild| guild.channels.get(&channel_id).cloned())
        .is_some()
}
