use poise::CreateReply;
use serenity::model::id::GuildId;

use crate::audio::queue::{clear_queue, take_current};
use crate::audio::voice::{author_voice_channel, channel_is_cached};
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
    let (guild_id, channel_id) = author_voice_channel(ctx)?;
    if !channel_is_cached(ctx.cache(), guild_id, channel_id) {
        return Err(BotError::Voice("voice channel is not available".to_owned()));
    }

    ctx.data()
        .voice
        .join(guild_id, channel_id)
        .await
        .map_err(|error| BotError::Voice(error.to_string()))?;

    ctx.send(
        CreateReply::default().embed(embeds::success("เข้าร่วมห้องเสียงแล้ว", "พร้อมเล่นเพลงด้วย /play")),
    )
    .await?;
    Ok(())
}

/// Leave the voice channel and clear the queue.
#[poise::command(slash_command, guild_only)]
pub async fn leave(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = command_guild(ctx)?;
    let data = ctx.data();

    clear_queue(data, guild_id).await;
    if let Some(current) = take_current(data, guild_id).await {
        current
            .handle
            .stop()
            .map_err(|error| BotError::Track(error.to_string()))?;
    }
    data.voice
        .leave(guild_id)
        .await
        .map_err(|error| BotError::Voice(error.to_string()))?;

    ctx.send(CreateReply::default().embed(embeds::success("ออกจากห้องเสียงแล้ว", "ล้างคิวเพลงเรียบร้อย")))
        .await?;
    Ok(())
}
