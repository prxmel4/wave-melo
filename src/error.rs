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
                "การตั้งค่าของบอทไม่ถูกต้อง กรุณาติดต่อผู้ดูแลระบบ"
            }
            Self::Discord(_) => "เกิดปัญหาการเชื่อมต่อกับ Discord กรุณาลองใหม่ในภายหลัง",
            Self::Voice(_) => "ไม่สามารถเข้าห้องเสียงได้ กรุณาตรวจสอบสิทธิ์ของบอท",
            Self::Track(_) => "เกิดปัญหาในการเล่นเพลง กรุณาลองใหม่",
            Self::InvalidQuery(_) => "รูปแบบลิงก์หรือคำค้นหาไม่ถูกต้อง",
            Self::NotInVoice => "คุณต้องอยู่ในห้องเสียงก่อนใช้คำสั่งนี้",
            Self::QueueEmpty => "คิวเพลงว่างอยู่",
            Self::Lavalink(_) => "ไม่สามารถดึงข้อมูลเพลงได้ กรุณาลองใหม่ในภายหลัง",
            Self::Message(_) => "เกิดข้อผิดพลาด กรุณาลองใหม่ในภายหลัง",
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
            "การตั้งค่าของบอทไม่ถูกต้อง กรุณาติดต่อผู้ดูแลระบบ"
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
    fn maps_simple_variants_to_thai_user_messages() {
        let cases = [
            (BotError::NotInVoice, "คุณต้องอยู่ในห้องเสียงก่อนใช้คำสั่งนี้"),
            (BotError::QueueEmpty, "คิวเพลงว่างอยู่"),
            (
                BotError::InvalidQuery(String::new()),
                "รูปแบบลิงก์หรือคำค้นหาไม่ถูกต้อง",
            ),
            (
                BotError::Lavalink("failed".to_owned()),
                "ไม่สามารถดึงข้อมูลเพลงได้ กรุณาลองใหม่ในภายหลัง",
            ),
            (
                BotError::Logging("subscriber".to_owned()),
                "การตั้งค่าของบอทไม่ถูกต้อง กรุณาติดต่อผู้ดูแลระบบ",
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
