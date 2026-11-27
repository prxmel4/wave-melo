use std::time::Duration;

use crate::state::QueuedTrack;

/// Metadata needed to display and rebuild an audio track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackMetadata {
    pub title: String,
    pub url: String,
    pub duration: Option<Duration>,
    pub requester: String,
}

impl TrackMetadata {
    pub fn new(title: String, url: String, duration: Option<Duration>, requester: String) -> Self {
        Self {
            title,
            url,
            duration,
            requester,
        }
    }
}

impl From<TrackMetadata> for QueuedTrack {
    fn from(metadata: TrackMetadata) -> Self {
        Self {
            title: metadata.title,
            url: metadata.url,
            requester: metadata.requester,
            duration: metadata.duration,
        }
    }
}
