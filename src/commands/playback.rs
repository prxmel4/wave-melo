use poise::CreateReply;

use crate::audio::queue::take_current;
use crate::error::BotError;
use crate::state::BotData;
use crate::ui::embeds;

/// Skip the current track and play the next queued track.
#[poise::command(slash_command, guild_only)]
pub async fn skip(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))?;

    let current = take_current(ctx.data(), guild_id)
        .await
        .ok_or(BotError::QueueEmpty)?;
    current
        .handle
        .stop()
        .map_err(|error| BotError::Track(error.to_string()))?;

    ctx.send(CreateReply::default().embed(embeds::success("ข้ามเพลงแล้ว", "กำลังเล่นเพลงถัดไปในคิว")))
        .await?;
    Ok(())
}
