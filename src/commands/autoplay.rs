//! Toggle autoplay: keep the music going with related tracks.

use poise::CreateReply;

use crate::error::BotError;
use crate::playback::ops::require_same_voice;
use crate::state::BotData;
use crate::ui::embeds;

/// Toggle autoplay to keep playing related tracks when the queue runs out.
#[poise::command(slash_command, guild_only)]
pub async fn autoplay(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))?;
    require_same_voice(
        ctx.cache(),
        guild_id,
        ctx.author().id,
        ctx.cache().current_user().id,
    )?;

    let data = ctx.data();
    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;

    let session = data.guild_states.session(guild_id.get());
    let enabled = {
        let mut details = session.details.lock().await;
        details.autoplay = !details.autoplay;
        if details.autoplay {
            // Re-enabling clears the failure budget for a fresh start.
            details.autoplay_failures = 0;
        }
        details.autoplay
    };
    if !enabled {
        session.autoplay.cancel();
    }
    drop(_guard);

    let (title, description) = if enabled {
        (
            "Autoplay enabled",
            "Related tracks will play automatically when the queue runs out.",
        )
    } else {
        (
            "Autoplay disabled",
            "Playback will stop when the queue runs out.",
        )
    };

    ctx.send(CreateReply::default().embed(embeds::success(title, description)))
        .await?;
    Ok(())
}
