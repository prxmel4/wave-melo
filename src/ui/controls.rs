//! Component interaction handler for Now Playing controls.
//!
//! Every control carries the guild id and a generation number. The generation
//! is bumped on any transition that changes the current track, so old buttons
//! cannot trigger a second skip or operate on a different session.

use std::sync::Arc;

use serenity::all::{
    ComponentInteraction, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
    EditInteractionResponse,
};
use serenity::model::id::GuildId;

use crate::error::BotError;
use crate::playback::ops::{
    require_player, require_same_voice, set_paused, set_volume, MAX_VOLUME, MIN_VOLUME,
};
use crate::state::{BotData, GuildSession};
use crate::ui::components::{
    control_rows, parse_control_custom_id, parse_queue_custom_id, ControlOp,
};
use crate::ui::embeds;
use crate::ui::now_playing;

const PREFIX: &str = "wavemelo:";
const VOLUME_STEP: u16 = 10;

/// Compute the next bot-facing volume for a step operation.
pub fn next_volume(current: u16, op: ControlOp) -> u16 {
    let current = current.clamp(MIN_VOLUME, MAX_VOLUME);
    match op {
        ControlOp::VolumeUp => current.saturating_add(VOLUME_STEP).min(MAX_VOLUME),
        ControlOp::VolumeDown => current.saturating_sub(VOLUME_STEP),
        _ => current,
    }
}

/// Handle a component interaction. IDs from other applications are ignored.
pub async fn handle_component(
    ctx: &Context,
    component: &ComponentInteraction,
    data: &BotData,
) -> Result<(), BotError> {
    let raw = component.data.custom_id.as_str();
    if raw.starts_with("wavemelo:queue:") {
        return handle_queue_page(ctx, component, data).await;
    }
    if !raw.starts_with(PREFIX) {
        return Ok(());
    }

    let (target_guild, generation, op) = match parse_control_custom_id(raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            reject(ctx, component, error.user_message()).await;
            return Ok(());
        }
    };

    let Some(guild_id) = component.guild_id else {
        reject(ctx, component, "Controls are only available in a server.").await;
        return Ok(());
    };
    if guild_id.get() != target_guild {
        reject(ctx, component, "This control belongs to another server.").await;
        return Ok(());
    }

    let Some(session) = data.guild_states.existing_session(guild_id.get()) else {
        reject(ctx, component, "This control has expired. Use /nowplaying.").await;
        return Ok(());
    };
    let current_generation = session.details.lock().await.control_generation();
    if current_generation != generation {
        reject(ctx, component, BotError::StaleControl.user_message()).await;
        return Ok(());
    }

    let bot_id = ctx.cache.current_user().id;
    if let Err(error) = require_same_voice(&ctx.cache, guild_id, component.user.id, bot_id) {
        reject(ctx, component, error.user_message()).await;
        return Ok(());
    }

    // Acknowledge before remote work so Discord never times out the interaction.
    component
        .create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
        .await?;

    let lock = data.guild_lock(guild_id).await;
    let outcome = {
        let _guard = lock.lock().await;
        // Re-validate under the lock: a concurrent click may have advanced
        // the generation between the pre-check and lock acquisition.
        let current = session.details.lock().await.control_generation();
        if current != generation {
            Err(BotError::StaleControl)
        } else {
            apply(data, guild_id, &session, generation, op).await
        }
    };
    drop(lock);

    let builder = match outcome {
        Ok(builder) => builder,
        Err(BotError::StaleControl) => {
            // A concurrent interaction already refreshed this message.
            return Ok(());
        }
        Err(error) => EditInteractionResponse::new()
            .embed(embeds::error("Control failed", error.user_message()))
            .components(Vec::new()),
    };
    if let Err(error) = component.edit_response(&ctx.http, builder).await {
        tracing::warn!(error = ?error, "failed to update component response");
    }
    Ok(())
}

/// Render a queue page from a navigation button. Read-only; no voice gate.
async fn handle_queue_page(
    ctx: &Context,
    component: &ComponentInteraction,
    data: &BotData,
) -> Result<(), BotError> {
    let (target_guild, page) = match parse_queue_custom_id(&component.data.custom_id) {
        Ok(parsed) => parsed,
        Err(error) => {
            reject(ctx, component, error.user_message()).await;
            return Ok(());
        }
    };
    let Some(guild_id) = component.guild_id else {
        reject(
            ctx,
            component,
            "Queue pages are only available in a server.",
        )
        .await;
        return Ok(());
    };
    if guild_id.get() != target_guild {
        reject(ctx, component, "This queue page belongs to another server.").await;
        return Ok(());
    }

    component
        .create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
        .await?;

    let builder = match crate::commands::queue::collect_snapshot(data, guild_id).await {
        Ok(snapshot) => {
            let requested = usize::try_from(page).unwrap_or(usize::MAX);
            let (embed, rows) =
                crate::commands::queue::render_page_for(&snapshot, requested, guild_id.get());
            EditInteractionResponse::new().embed(embed).components(rows)
        }
        Err(error) => EditInteractionResponse::new()
            .embed(embeds::error("Queue unavailable", error.user_message()))
            .components(Vec::new()),
    };
    if let Err(error) = component.edit_response(&ctx.http, builder).await {
        tracing::warn!(error = ?error, "failed to update queue page");
    }
    Ok(())
}

/// Reject an interaction with a short private explanation.
async fn reject(ctx: &Context, component: &ComponentInteraction, message: &str) {
    let response = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content(message)
            .ephemeral(true),
    );
    if let Err(error) = component.create_response(&ctx.http, response).await {
        tracing::warn!(error = ?error, "failed to reject component interaction");
    }
}

/// Invalidate all previously issued controls and return the new generation.
async fn bump(session: &GuildSession) -> u64 {
    let mut details = session.details.lock().await;
    details.bump_control_generation();
    details.control_generation()
}

/// Apply one control while the guild lock is held by the caller.
async fn apply(
    data: &BotData,
    guild_id: GuildId,
    session: &Arc<GuildSession>,
    generation: u64,
    op: ControlOp,
) -> Result<EditInteractionResponse, BotError> {
    match op {
        ControlOp::PauseResume => {
            let player = require_player(data, guild_id)?;
            let current = player
                .get_player()
                .await
                .map_err(|error| BotError::Lavalink(error.to_string()))?;
            if current.track.is_none() {
                return Err(BotError::NoActivePlayer);
            }
            let changed = set_paused(data, guild_id, !current.paused).await?;
            let current_generation = if changed {
                bump(session).await
            } else {
                generation
            };
            let view = now_playing::collect_view(data, guild_id).await?;
            Ok(EditInteractionResponse::new()
                .embed(now_playing::build(&view))
                .components(control_rows(
                    guild_id.get(),
                    current_generation,
                    view.paused,
                )))
        }
        ControlOp::VolumeUp | ControlOp::VolumeDown => {
            let target = next_volume(
                {
                    let player = require_player(data, guild_id)?;
                    player
                        .get_player()
                        .await
                        .map_err(|error| BotError::Lavalink(error.to_string()))?
                        .volume
                },
                op,
            );
            set_volume(data, guild_id, target).await?;
            let view = now_playing::collect_view(data, guild_id).await?;
            Ok(EditInteractionResponse::new()
                .embed(now_playing::build(&view))
                .components(control_rows(guild_id.get(), generation, view.paused)))
        }
        ControlOp::Skip => {
            let player = require_player(data, guild_id)?;
            player
                .skip()
                .map_err(|error| BotError::Lavalink(error.to_string()))?;
            bump(session).await;
            Ok(EditInteractionResponse::new()
                .embed(embeds::success(
                    "Track skipped",
                    "Now playing the next queued track.",
                ))
                .components(Vec::new()))
        }
        ControlOp::Stop => {
            let player = require_player(data, guild_id)?;
            player
                .get_queue()
                .clear()
                .map_err(|error| BotError::Lavalink(error.to_string()))?;
            player
                .stop_now()
                .await
                .map_err(|error| BotError::Lavalink(error.to_string()))?;
            bump(session).await;
            Ok(EditInteractionResponse::new()
                .embed(embeds::success(
                    "Playback stopped",
                    "Queue cleared. Use /leave to disconnect.",
                ))
                .components(Vec::new()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{next_volume, MAX_VOLUME, MIN_VOLUME};
    use crate::ui::components::ControlOp;

    #[test]
    fn volume_steps_stay_inside_the_supported_range() {
        assert_eq!(next_volume(100, ControlOp::VolumeUp), 110);
        assert_eq!(next_volume(100, ControlOp::VolumeDown), 90);
        assert_eq!(next_volume(MAX_VOLUME, ControlOp::VolumeUp), MAX_VOLUME);
        assert_eq!(next_volume(MIN_VOLUME, ControlOp::VolumeDown), MIN_VOLUME);
        assert_eq!(next_volume(1000, ControlOp::VolumeUp), MAX_VOLUME);
        assert_eq!(next_volume(100, ControlOp::Skip), 100);
    }
}
