//! Recently played track history, sourced from bounded in-process state.

use poise::CreateReply;

use crate::error::BotError;
use crate::state::BotData;
use crate::ui::embeds;
use crate::utils::truncate;

/// Show the most recently started tracks for this guild.
#[poise::command(slash_command, guild_only)]
pub async fn history(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))?;
    let session = ctx.data().guild_states.session(guild_id.get());
    let recent: Vec<String> = {
        let details = session.details.lock().await;
        details.recent_tracks.iter().rev().cloned().collect()
    };

    let description = if recent.is_empty() {
        "*No tracks have played in this session yet.*".to_owned()
    } else {
        recent
            .iter()
            .enumerate()
            .map(|(index, title)| format!("{}. {}", index + 1, truncate(title, 80)))
            .collect::<Vec<_>>()
            .join("\n")
    };

    ctx.send(CreateReply::default().embed(embeds::info("Recent tracks", description)))
        .await?;
    Ok(())
}
