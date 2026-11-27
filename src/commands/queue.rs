use poise::CreateReply;

use crate::audio::queue::queue_snapshot;
use crate::error::BotError;
use crate::state::BotData;
use crate::ui::embeds::{self, QueueEntryView};

const PAGE_SIZE: usize = 10;

/// Show the next queued tracks.
#[poise::command(slash_command, guild_only)]
pub async fn queue(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))?;

    let tracks = queue_snapshot(ctx.data(), guild_id).await;
    let views = tracks
        .iter()
        .take(PAGE_SIZE)
        .map(|track| QueueEntryView {
            title: &track.title,
            url: Some(&track.url),
            requester: &track.requester,
            duration: track.duration,
        })
        .collect::<Vec<_>>();

    ctx.send(CreateReply::default().embed(embeds::queue(&views, 1, 1)))
        .await?;
    Ok(())
}
