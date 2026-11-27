use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use serenity::model::id::GuildId;
use songbird::tracks::TrackHandle;
use tokio::sync::Mutex;

use crate::config::AppConfig;
use crate::error::BotError;

/// A lightweight queue entry that can be rebuilt into an audio source later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedTrack {
    pub title: String,
    pub url: String,
    pub requester: String,
    pub duration: Option<Duration>,
}

/// The currently playing track plus the handle needed to control it.
pub struct CurrentTrack {
    pub handle: TrackHandle,
    pub metadata: QueuedTrack,
}

/// Shared process-wide state passed to every Poise command.
#[derive(Clone)]
pub struct BotData {
    pub config: Arc<AppConfig>,
    pub http_client: reqwest::Client,
    pub voice: Arc<songbird::Songbird>,
    /// Leaked once at startup so Songbird sources can borrow a `'static` binary name.
    pub ytdlp_binary: &'static str,
    pub queues: Arc<Mutex<HashMap<GuildId, VecDeque<QueuedTrack>>>>,
    pub current: Arc<Mutex<HashMap<GuildId, CurrentTrack>>>,
}

impl BotData {
    /// Build shared state from validated configuration.
    pub fn new(config: AppConfig) -> Result<Self, BotError> {
        let ytdlp_binary = Box::leak(config.ytdlp_binary.clone().into_boxed_str());

        Ok(Self {
            config: Arc::new(config),
            http_client: reqwest::Client::builder().build()?,
            voice: songbird::Songbird::serenity(),
            ytdlp_binary,
            queues: Arc::new(Mutex::new(HashMap::new())),
            current: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}
