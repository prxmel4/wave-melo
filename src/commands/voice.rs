use poise::CreateReply;
use serenity::model::id::GuildId;

use crate::audio::voice::{
    channel_is_cached, current_voice_channel, ensure_player_context, send_voice_state_update,
};
use crate::error::BotError;
use crate::state::BotData;
use crate::ui::embeds;

fn command_guild(ctx: poise::Context<'_, BotData, BotError>) -> Result<GuildId, BotError> {
    ctx.guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))
}

/// Join your current voice channel.
#[poise::command(slash_command, guild_only)]
pub async fn join(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    ctx.defer().await?;

    let guild_id = command_guild(ctx)?;
    let channel_id = current_voice_channel(ctx.cache(), guild_id, ctx.author().id)
        .ok_or(BotError::NotInVoice)?;
    if !channel_is_cached(ctx.cache(), guild_id, channel_id) {
        return Err(BotError::Voice("voice channel is not available".to_owned()));
    }

    let data = ctx.data();
    ensure_player_context(ctx.serenity_context(), data, guild_id, channel_id).await?;

    ctx.send(CreateReply::default().embed(embeds::success(
        "Joined voice channel",
        "Ready to play music with /play.",
    )))
    .await?;
    Ok(())
}

/// Leave the voice channel and clear the queue.
#[poise::command(slash_command, guild_only)]
pub async fn leave(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = command_guild(ctx)?;
    let data = ctx.data();

    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;

    // `delete_player` closes the local context and deletes the server-side
    // player in one step. Closing the context manually first would double-close
    // its channel and make `delete_player` fail before its HTTP call.
    if let Err(error) = data.lavalink.delete_player(guild_id.get()).await {
        tracing::warn!(guild_id = %guild_id, error = %error, "failed to delete lavalink player");
    }

    // Send voice state update to disconnect from the channel.
    if let Err(error) =
        send_voice_state_update(&ctx.serenity_context().shard, guild_id, None, false, false)
    {
        tracing::warn!(guild_id = %guild_id, error = %error, "failed to send voice disconnect");
    }

    drop(_guard);
    data.remove_guild_state(guild_id).await;

    ctx.send(CreateReply::default().embed(embeds::success("Left voice channel", "Queue cleared.")))
        .await?;
    Ok(())
}
