use crate::config::ConfigError;

/// Central error type shared by the library and binary.
#[derive(Debug, thiserror::Error)]
pub enum BotError {
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),
    #[error("environment error: {0}")]
    Environment(#[from] dotenvy::Error),
    #[error("logging initialization failed: {0}")]
    Logging(String),
    #[error("HTTP client error")]
    Http(#[from] reqwest::Error),
    #[error("Discord API error")]
    Discord(Box<serenity::Error>),
    #[error("voice connection failed: {0}")]
    Voice(String),
    #[error("track operation failed: {0}")]
    Track(String),
    #[error("invalid playback query: {0}")]
    InvalidQuery(String),
    #[error("author is not in a voice channel")]
    NotInVoice,
    #[error("the queue is empty")]
    QueueEmpty,
    #[error("yt-dlp failed: {0}")]
    YtDlp(String),
    #[error("{0}")]
    Message(String),
}

impl From<serenity::Error> for BotError {
    fn from(error: serenity::Error) -> Self {
        Self::Discord(Box::new(error))
    }
}
