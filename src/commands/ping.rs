use std::time::Instant;

use poise::CreateReply;

use crate::error::BotError;
use crate::state::BotData;
use crate::ui::embeds;

/// Check bot health and latency.
#[poise::command(slash_command)]
pub async fn ping(ctx: poise::Context<'_, BotData, BotError>) -> Result<(), BotError> {
    ctx.defer_ephemeral().await?;

    let gateway_latency = ctx.ping().await;
    let http_started = Instant::now();
    ctx.http().get_current_user().await?;
    let http_latency = http_started.elapsed();

    let description = format!(
        "Gateway: {} ms · HTTP: {} ms",
        gateway_latency.as_millis(),
        http_latency.as_millis()
    );
    let handle = ctx
        .send(
            CreateReply::default()
                .embed(embeds::info("Pong!", format!("Checking...\n{description}"))),
        )
        .await?;

    handle
        .edit(
            ctx,
            CreateReply::default().embed(embeds::info("Pong!", description)),
        )
        .await?;
    Ok(())
}
