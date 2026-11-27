use reqwest::Client;
use songbird::input::{Compose, Input, YoutubeDl};

use crate::audio::{Query, TrackMetadata};
use crate::error::BotError;

/// Build a Songbird input and its display metadata from a classified query.
pub async fn build_source(
    client: Client,
    ytdlp_binary: &'static str,
    query: &Query,
    requester: &str,
) -> Result<(Input, TrackMetadata), BotError> {
    let source = YoutubeDl::new_ytdl_like(ytdlp_binary, client, query.as_str().to_owned());

    let mut probe = source.clone();
    let metadata = probe
        .aux_metadata()
        .await
        .map_err(|error| BotError::YtDlp(error.to_string()))?;

    let title = metadata.title.unwrap_or_else(|| query.as_str().to_owned());
    let url = metadata
        .source_url
        .unwrap_or_else(|| query.as_str().to_owned());

    Ok((
        source.into(),
        TrackMetadata::new(title, url, metadata.duration, requester.to_owned()),
    ))
}
