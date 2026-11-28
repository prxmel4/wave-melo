use std::time::Duration;

use lavalink_rs::model::track::{TrackData, TrackLoadData, TrackLoadType};
use lavalink_rs::prelude::LavalinkClient;

use crate::audio::Query;
use crate::error::BotError;

/// Metadata about a loaded playlist or album.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistMeta {
    pub name: String,
    pub selected_track: Option<u32>,
}

/// The result of a track load operation.
#[derive(Debug, Clone)]
pub enum LoadedTracks {
    /// A single track was loaded.
    Single(TrackData),
    /// A search result produced one or more tracks; the first is selected.
    SearchResult(TrackData),
    /// A playlist or album was loaded.
    Playlist {
        meta: PlaylistMeta,
        tracks: Vec<TrackData>,
    },
    /// No results were found.
    Empty,
}

/// Resolve a query through Lavalink and return the loaded tracks.
///
/// `guild_id` selects the node responsible for the guild (node affinity).
pub async fn load_tracks(
    client: &LavalinkClient,
    guild_id: u64,
    query: &Query,
) -> Result<LoadedTracks, BotError> {
    let result = client
        .load_tracks(guild_id, query.as_str())
        .await
        .map_err(|e| BotError::Lavalink(e.to_string()))?;

    match result.load_type {
        TrackLoadType::Track => match result.data {
            Some(TrackLoadData::Track(track)) => Ok(LoadedTracks::Single(track)),
            _ => Err(BotError::Lavalink("unexpected track load data".to_owned())),
        },
        TrackLoadType::Search => match result.data {
            Some(TrackLoadData::Search(mut tracks)) => {
                if tracks.is_empty() {
                    return Ok(LoadedTracks::Empty);
                }
                let track = tracks.swap_remove(0);
                Ok(LoadedTracks::SearchResult(track))
            }
            _ => Err(BotError::Lavalink("unexpected search load data".to_owned())),
        },
        TrackLoadType::Playlist => match result.data {
            Some(TrackLoadData::Playlist(playlist)) => {
                let meta = PlaylistMeta {
                    name: playlist.info.name,
                    selected_track: playlist.info.selected_track,
                };
                Ok(LoadedTracks::Playlist {
                    meta,
                    tracks: playlist.tracks,
                })
            }
            _ => Err(BotError::Lavalink(
                "unexpected playlist load data".to_owned(),
            )),
        },
        TrackLoadType::Empty => Ok(LoadedTracks::Empty),
        TrackLoadType::Error => match result.data {
            Some(TrackLoadData::Error(err)) => {
                Err(BotError::Lavalink(format!("load failed: {}", err.message)))
            }
            _ => Err(BotError::Lavalink("unknown load error".to_owned())),
        },
    }
}

/// Format a track's duration as a `Duration`, returning `None` for live streams.
pub fn track_duration(track: &TrackData) -> Option<Duration> {
    if track.info.is_stream || track.info.length == 0 {
        None
    } else {
        Some(Duration::from_millis(track.info.length))
    }
}

/// Return the best display URL for a track, falling back to the identifier.
pub fn track_url(track: &TrackData) -> String {
    track
        .info
        .uri
        .clone()
        .unwrap_or_else(|| track.info.identifier.clone())
}
