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
    #[error("Discord API error")]
    Discord(Box<serenity::Error>),
    #[error("invalid volume: {0}")]
    InvalidVolume(String),
    #[error("no active player in this guild")]
    NoActivePlayer,
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
    #[error("Lavalink error: {0}")]
    Lavalink(String),
    #[error("{0}")]
    Message(String),
}

impl From<serenity::Error> for BotError {
    fn from(error: serenity::Error) -> Self {
        Self::Discord(Box::new(error))
    }
}

impl BotError {
    /// Return a short, safe message suitable for Discord replies.
    ///
    /// Internal details, paths, and secrets are intentionally excluded.
    pub fn user_message(&self) -> &'static str {
        match self {
            Self::Config(_) | Self::Environment(_) | Self::Logging(_) => {
                "The bot is misconfigured. Please contact the server administrator."
            }
            Self::Discord(_) => "Could not reach Discord. Please try again later.",
            Self::Voice(_) => {
                "Could not join the voice channel. Please check the bot's permissions."
            }
            Self::Track(_) => "Playback failed. Please try again.",
            Self::InvalidVolume(_) => "Volume must be between 0 and 150.",
            Self::NoActivePlayer => "No active player here. Use /play first.",
            Self::InvalidQuery(_) => "The link or search query is invalid.",
            Self::NotInVoice => "You must join a voice channel before using this command.",
            Self::QueueEmpty => "The queue is empty.",
            Self::Lavalink(_) => "Could not load track data. Please try again later.",
            Self::Message(_) => "Something went wrong. Please try again later.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BotError;
    use crate::config::ConfigError;

    #[test]
    fn config_error_display_and_user_message() {
        let error = BotError::from(ConfigError::MissingDiscordToken);

        assert_eq!(
            error.to_string(),
            "configuration error: DISCORD_TOKEN is not set"
        );
        assert_eq!(
            error.user_message(),
            "The bot is misconfigured. Please contact the server administrator."
        );
    }

    #[test]
    fn displays_internal_detail_for_diagnostics() {
        let error = BotError::InvalidQuery("missing-query".to_owned());
        assert_eq!(error.to_string(), "invalid playback query: missing-query");

        let error = BotError::Voice("permission denied".to_owned());
        assert_eq!(
            error.to_string(),
            "voice connection failed: permission denied"
        );
    }

    #[test]
    fn maps_simple_variants_to_english_user_messages() {
        let cases = [
            (
                BotError::NotInVoice,
                "You must join a voice channel before using this command.",
            ),
            (BotError::QueueEmpty, "The queue is empty."),
            (
                BotError::InvalidQuery(String::new()),
                "The link or search query is invalid.",
            ),
            (
                BotError::Lavalink("failed".to_owned()),
                "Could not load track data. Please try again later.",
            ),
            (
                BotError::Logging("subscriber".to_owned()),
                "The bot is misconfigured. Please contact the server administrator.",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.user_message(), expected);
        }
    }

    #[test]
    fn user_message_does_not_leak_internal_details() {
        let marker = "internal-only-marker";
        let error = BotError::Lavalink(marker.to_owned());

        assert!(error.to_string().contains(marker));
        assert!(!error.user_message().contains(marker));
    }
}
