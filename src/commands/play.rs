use poise::CreateReply;

use crate::audio::events::attach_lifecycle_events;
use crate::audio::query::classify;
use crate::audio::queue::{enqueue, is_idle};
use crate::audio::source::build_source;
use crate::audio::voice::{author_voice_channel, channel_is_cached};
use crate::error::BotError;
use crate::state::{BotData, CurrentTrack, QueuedTrack};
use crate::ui::embeds;

/// Play a URL or search a YouTube query; queue it if something is playing.
#[poise::command(slash_command, guild_only)]
pub async fn play(
    ctx: poise::Context<'_, BotData, BotError>,
    #[description = "YouTube URL or search query"] query: String,
) -> Result<(), BotError> {
    let (guild_id, channel_id) = author_voice_channel(ctx)?;
    if !channel_is_cached(ctx.cache(), guild_id, channel_id) {
        return Err(BotError::Voice("voice channel is not available".to_owned()));
    }

    let query = classify(&query)?;
    let data = ctx.data();

    if data.voice.get(guild_id).is_none() {
        data.voice
            .join(guild_id, channel_id)
            .await
            .map_err(|error| BotError::Voice(error.to_string()))?;
    }

    let requester = ctx.author().name.clone();
    let (input, metadata) = build_source(
        data.http_client.clone(),
        data.ytdlp_binary,
        &query,
        &requester,
        data.config.youtube_cookies_path.as_deref(),
    )
    .await?;
    let queued = QueuedTrack::from(metadata);

    if !is_idle(data, guild_id).await {
        let title = queued.title.clone();
        enqueue(data, guild_id, queued).await;
        ctx.send(CreateReply::default().embed(embeds::success(
            "เพิ่มเพลงเข้าคิวแล้ว",
            format!("**{title}** ถูกเพิ่มเข้าคิว"),
        )))
        .await?;
        return Ok(());
    }

    let embed = embeds::now_playing(
        &queued.title,
        Some(&queued.url),
        &queued.requester,
        queued.duration,
        None,
    );
    let call = data
        .voice
        .get(guild_id)
        .ok_or_else(|| BotError::Voice("no active voice call".to_owned()))?;
    let handle = {
        let mut guard = call.lock().await;
        guard.play_input(input)
    };
    attach_lifecycle_events(&handle, guild_id, data)?;

    let current = CurrentTrack {
        handle,
        metadata: queued,
    };
    data.current.lock().await.insert(guild_id, current);

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}
