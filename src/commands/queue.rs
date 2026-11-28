use std::collections::VecDeque;

use lavalink_rs::model::track::TrackData;
use poise::CreateReply;
use serenity::all::{CreateActionRow, CreateEmbed};
use serenity::model::id::GuildId;

use crate::audio::loader::{track_duration, track_url};
use crate::error::BotError;
use crate::playback::ops::require_same_voice;
use crate::queue::{
    clamp_page, jump_plan, move_indices, page_bounds, page_count, reordered, shuffle, time_seed,
    zero_based,
};
use crate::state::{BotData, LoopMode};
use crate::ui::components::queue_rows;
use crate::ui::embeds::{self, QueueEntryView};

/// Owned display data for one queue row.
pub type Row = (String, String, String, Option<std::time::Duration>);

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

/// Owned snapshot of everything needed to render any queue page.
pub struct QueueSnapshot {
    pub current: Option<Row>,
    pub rows: Vec<Row>,
    pub total: usize,
}

/// Read the current track and queued rows from Lavalink.
pub async fn collect_snapshot(
    data: &BotData,
    guild_id: GuildId,
) -> Result<QueueSnapshot, BotError> {
    let (current, queued) = match data.lavalink.get_player_context(guild_id.get()) {
        Some(player_ctx) => {
            let player = player_ctx
                .get_player()
                .await
                .map_err(|error| BotError::Lavalink(error.to_string()))?;
            let queued = player_ctx
                .get_queue()
                .get_queue()
                .await
                .map_err(|error| BotError::Lavalink(error.to_string()))?;
            (player.track, queued)
        }
        None => (None, VecDeque::new()),
    };
    Ok(QueueSnapshot {
        current: current.as_ref().map(row_of),
        total: queued.len(),
        rows: queued.iter().map(|item| row_of(&item.track)).collect(),
    })
}

/// Render a bounded queue page and its navigation buttons for a guild.
pub fn render_page_for(
    snapshot: &QueueSnapshot,
    page: usize,
    guild_id: u64,
) -> (CreateEmbed, Vec<CreateActionRow>) {
    let pages = page_count(snapshot.total);
    let page = clamp_page(page, pages);
    let (start, end) = page_bounds(page, snapshot.total);
    let current_view = snapshot.current.as_ref().map(view_of);
    let visible: Vec<QueueEntryView<'_>> = snapshot.rows[start..end].iter().map(view_of).collect();
    let mut embed = embeds::queue(current_view.as_ref(), &visible, snapshot.total);
    embed = embed.footer(serenity::all::CreateEmbedFooter::new(format!(
        "Page {page} of {pages} | {} tracks in queue",
        snapshot.total
    )));
    (embed, queue_rows(guild_id, page as u64, pages as u64))
}

fn view_of(row: &Row) -> QueueEntryView<'_> {
    QueueEntryView {
        title: &row.0,
        url: Some(&row.1),
        requester: &row.2,
        duration: row.3,
    }
}

/// Show the current track and the first page of queued tracks.
#[poise::command(slash_command, guild_only)]
pub async fn queue(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = command_guild(ctx)?;
    let data = ctx.data();
    let snapshot = collect_snapshot(data, guild_id).await?;
    let (embed, rows) = render_page_for(&snapshot, 1, guild_id.get());

    ctx.send(CreateReply::default().embed(embed).components(rows))
        .await?;
    Ok(())
}

fn command_guild(
    ctx: poise::Context<'_, BotData, BotError>,
) -> Result<serenity::model::id::GuildId, BotError> {
    ctx.guild_id()
        .ok_or_else(|| BotError::Voice("this command must be used in a guild".to_owned()))
}

/// Remove one queued track by its 1-based position.
#[poise::command(slash_command, guild_only)]
pub async fn queueremove(
    ctx: poise::Context<'_, BotData, BotError>,
    #[description = "Queue position to remove, starting at 1"] index: u16,
) -> Result<(), BotError> {
    let guild_id = command_guild(ctx)?;
    require_same_voice(
        ctx.cache(),
        guild_id,
        ctx.author().id,
        ctx.cache().current_user().id,
    )?;

    let data = ctx.data();
    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;
    let player_ctx = data
        .lavalink
        .get_player_context(guild_id.get())
        .ok_or(BotError::QueueEmpty)?;
    let queue_ref = player_ctx.get_queue();
    let queued = queue_ref
        .get_queue()
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    let zero_based = zero_based(usize::from(index), queued.len())?;
    queue_ref
        .remove(zero_based)
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Track removed",
        format!("Removed queue entry #{}.", zero_based + 1),
    )))
    .await?;
    Ok(())
}

/// Play a queued track immediately by its 1-based position.
#[poise::command(slash_command, guild_only)]
pub async fn queuejump(
    ctx: poise::Context<'_, BotData, BotError>,
    #[description = "Queue position to play next, starting at 1"] index: u16,
) -> Result<(), BotError> {
    let guild_id = command_guild(ctx)?;
    require_same_voice(
        ctx.cache(),
        guild_id,
        ctx.author().id,
        ctx.cache().current_user().id,
    )?;

    let data = ctx.data();
    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;
    let player_ctx = data
        .lavalink
        .get_player_context(guild_id.get())
        .ok_or(BotError::QueueEmpty)?;
    let queue_ref = player_ctx.get_queue();
    let queued = queue_ref
        .get_queue()
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    let (target, rest) = jump_plan(queued, usize::from(index))?;
    let title = target.track.info.title.clone();
    queue_ref
        .replace(rest)
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    queue_ref
        .push_to_front(target)
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    player_ctx
        .skip()
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    {
        let session = data.guild_states.session(guild_id.get());
        session.details.lock().await.bump_control_generation();
    }
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Track moved to front",
        format!("Now playing **{}**.", crate::utils::truncate(&title, 80)),
    )))
    .await?;
    Ok(())
}

/// Move a queued track from one 1-based position to another.
#[poise::command(slash_command, guild_only)]
pub async fn queuemove(
    ctx: poise::Context<'_, BotData, BotError>,
    #[description = "Current queue position, starting at 1"] from: u16,
    #[description = "New queue position, starting at 1"] to: u16,
) -> Result<(), BotError> {
    let guild_id = command_guild(ctx)?;
    require_same_voice(
        ctx.cache(),
        guild_id,
        ctx.author().id,
        ctx.cache().current_user().id,
    )?;

    let data = ctx.data();
    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;
    let player_ctx = data
        .lavalink
        .get_player_context(guild_id.get())
        .ok_or(BotError::QueueEmpty)?;
    let queue_ref = player_ctx.get_queue();
    let queued = queue_ref
        .get_queue()
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    let (from_index, to_index) = move_indices(queued.len(), usize::from(from), usize::from(to))?;
    queue_ref
        .replace(reordered(queued, from_index, to_index))
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Queue reordered",
        format!("Moved entry #{from} to #{to}."),
    )))
    .await?;
    Ok(())
}

/// Clear all queued tracks without disconnecting from voice.
#[poise::command(slash_command, guild_only)]
pub async fn queueclear(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = command_guild(ctx)?;
    require_same_voice(
        ctx.cache(),
        guild_id,
        ctx.author().id,
        ctx.cache().current_user().id,
    )?;

    let data = ctx.data();
    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;
    if let Some(player_ctx) = data.lavalink.get_player_context(guild_id.get()) {
        player_ctx
            .get_queue()
            .clear()
            .map_err(|error| BotError::Lavalink(error.to_string()))?;
    }
    data.guild_states
        .session(guild_id.get())
        .loop_control
        .clear_snapshot();
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Queue cleared",
        "All queued tracks were removed. The current track keeps playing.",
    )))
    .await?;
    Ok(())
}

/// Randomize the order of upcoming tracks.
#[poise::command(slash_command, guild_only)]
pub async fn shuffleq(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    let guild_id = command_guild(ctx)?;
    require_same_voice(
        ctx.cache(),
        guild_id,
        ctx.author().id,
        ctx.cache().current_user().id,
    )?;

    let data = ctx.data();
    let lock = data.guild_lock(guild_id).await;
    let _guard = lock.lock().await;
    let player_ctx = data
        .lavalink
        .get_player_context(guild_id.get())
        .ok_or(BotError::QueueEmpty)?;

    // Shuffling injected loop copies would break marker-based replenishment.
    if data
        .guild_states
        .session(guild_id.get())
        .loop_control
        .mode()
        != LoopMode::Off
    {
        return Err(BotError::Voice(
            "turn off loop mode before shuffling".to_owned(),
        ));
    }

    let queue_ref = player_ctx.get_queue();
    let mut queued = queue_ref
        .get_queue()
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    if queued.len() < 2 {
        return Err(BotError::QueueEmpty);
    }
    let count = queued.len();
    shuffle(&mut queued, time_seed());
    queue_ref
        .replace(queued)
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    drop(_guard);

    ctx.send(CreateReply::default().embed(embeds::success(
        "Queue shuffled",
        format!("Randomized {count} upcoming tracks."),
    )))
    .await?;
    Ok(())
}
