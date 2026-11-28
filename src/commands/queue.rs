use std::collections::VecDeque;

use lavalink_rs::model::track::TrackData;
use poise::CreateReply;

use crate::audio::loader::{track_duration, track_url};
use crate::error::BotError;
use crate::state::BotData;
use crate::ui::embeds::{self, QueueEntryView};

const PAGE_SIZE: usize = 10;

/// Owned display data for one queue row.
type Row = (String, String, String, Option<std::time::Duration>);

fn requester_of(track: &TrackData) -> String {
    track
        .user_data
        .as_ref()
        .and_then(|data| data.get("requester"))
        .and_then(|value| value.as_str())
        .unwrap_or("Unknown user")
        .to_owned()
}

fn row_of(track: &TrackData) -> Row {
    (
        track.info.title.clone(),
        track_url(track),
        requester_of(track),
        track_duration(track),
    )
}

/// Show the current track and the next queued tracks.
#[poise::command(slash_command, guild_only)]
pub async fn queue(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))?;

    let data = ctx.data();
    let (current, queued) = match data.lavalink.get_player_context(guild_id.get()) {
        Some(player_ctx) => {
            let player = player_ctx
                .get_player()
                .await
                .map_err(|e| BotError::Lavalink(e.to_string()))?;
            let queued = player_ctx
                .get_queue()
                .get_queue()
                .await
                .map_err(|e| BotError::Lavalink(e.to_string()))?;
            (player.track, queued)
        }
        None => (None, VecDeque::new()),
    };

    let current_row = current.as_ref().map(row_of);
    let queued_rows: Vec<Row> = queued
        .iter()
        .take(PAGE_SIZE)
        .map(|item| row_of(&item.track))
        .collect();

    let current_view = current_row
        .as_ref()
        .map(|(title, url, requester, duration)| QueueEntryView {
            title,
            url: Some(url),
            requester,
            duration: *duration,
        });
    let views: Vec<QueueEntryView<'_>> = queued_rows
        .iter()
        .map(|(title, url, requester, duration)| QueueEntryView {
            title,
            url: Some(url),
            requester,
            duration: *duration,
        })
        .collect();

    ctx.send(CreateReply::default().embed(embeds::queue(
        current_view.as_ref(),
        &views,
        queued.len(),
    )))
    .await?;
    Ok(())
}
