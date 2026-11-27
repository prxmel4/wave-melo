use std::time::Instant;

use songbird::input::{Compose, Input, YoutubeDl};

use crate::audio::cache::{CacheKey, ResolvedMetadata};
use crate::audio::{Query, TrackMetadata};
use crate::error::BotError;
use crate::state::BotData;
use crate::utils::text::truncate_graphemes;

const TITLE_LIMIT: usize = 256;
const URL_LIMIT: usize = 512;
const REQUESTED_FORMAT: &str = "bestaudio[acodec=opus]/bestaudio/best";

/// Subprocess arguments shared by metadata probing and prepared playback sources.
fn yt_dlp_args(cookies_path: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "--no-playlist".to_owned(),
        "--no-warnings".to_owned(),
        "-f".to_owned(),
        REQUESTED_FORMAT.to_owned(),
    ];
    if let Some(path) = cookies_path {
        args.push("--cookies".to_owned());
        args.push(path.to_owned());
    }
    args
}

fn bounded(text: String, limit: usize) -> String {
    truncate_graphemes(&text, limit).into_owned()
}

fn youtube_dl(data: &BotData, query: &Query) -> YoutubeDl<'static> {
    let source = YoutubeDl::new_ytdl_like(
        data.ytdlp_binary,
        data.http_client.clone(),
        query.as_str().to_owned(),
    );
    let source = source.user_args(yt_dlp_args(data.config.youtube_cookies_path.as_deref()));
    source
}

/// Resolve stable metadata through the bounded cache.
///
/// Stream URLs are deliberately not cached: Songbird re-resolves them when a
/// lazy source is made playable, so playback always receives a fresh URL.
pub async fn resolve_metadata(data: &BotData, query: &Query) -> Result<ResolvedMetadata, BotError> {
    let key = CacheKey::new(
        query.as_str(),
        data.config.youtube_cookies_path.as_deref().unwrap_or(""),
    );

    data.metadata_cache
        .get_or_resolve(key, || async {
            let _permit = data.extract_semaphore.acquire().await;
            tracing::debug!(extractor_invocations = 1, "resolving track metadata");

            let mut probe = youtube_dl(data, query);
            let metadata = probe
                .aux_metadata()
                .await
                .map_err(|_| BotError::YtDlp("yt-dlp metadata extraction failed".to_owned()))?;

            let title = metadata.title.unwrap_or_else(|| query.as_str().to_owned());
            let url = metadata
                .source_url
                .unwrap_or_else(|| query.as_str().to_owned());

            Ok(ResolvedMetadata::new(
                bounded(title, TITLE_LIMIT),
                bounded(url, URL_LIMIT),
                metadata.duration,
                Instant::now(),
            ))
        })
        .await
}

/// Build a lazy input without spawning yt-dlp at construction time.
pub fn build_input(data: &BotData, query: &Query) -> Input {
    youtube_dl(data, query).into()
}

/// Resolve metadata and prepare a lazy input for playback.
pub async fn build_source(
    data: &BotData,
    query: &Query,
    requester: &str,
) -> Result<(Input, TrackMetadata), BotError> {
    let resolved = resolve_metadata(data, query).await?;
    let input = build_input(data, query);
    let metadata = TrackMetadata::new(
        resolved.title,
        resolved.url,
        resolved.duration,
        requester.to_owned(),
    );
    Ok((input, metadata))
}
