use poise::CreateReply;

use crate::audio::voice::{author_voice_channel, channel_is_cached};
use crate::error::BotError;
use crate::state::BotData;
use crate::ui::embeds;

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
