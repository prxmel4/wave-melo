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

impl BotError {
    /// Return a short, safe message suitable for Discord replies.
    ///
    /// Internal details, paths, and secrets are intentionally excluded.
    pub fn user_message(&self) -> &'static str {
        match self {
            Self::Config(_) | Self::Environment(_) | Self::Logging(_) => {
                "การตั้งค่าของบอทไม่ถูกต้อง กรุณาติดต่อผู้ดูแลระบบ"
            }
            Self::Http(_) => "เกิดปัญหาการเชื่อมต่อภายนอก กรุณาลองใหม่ในภายหลัง",
            Self::Discord(_) => "เกิดปัญหาการเชื่อมต่อกับ Discord กรุณาลองใหม่ในภายหลัง",
            Self::Voice(_) => "ไม่สามารถเข้าห้องเสียงได้ กรุณาตรวจสอบสิทธิ์ของบอท",
            Self::Track(_) => "เกิดปัญหาในการเล่นเพลง กรุณาลองใหม่",
            Self::InvalidQuery(_) => "รูปแบบลิงก์หรือคำค้นหาไม่ถูกต้อง",
            Self::NotInVoice => "คุณต้องอยู่ในห้องเสียงก่อนใช้คำสั่งนี้",
            Self::QueueEmpty => "คิวเพลงว่างอยู่",
            Self::YtDlp(_) => "ไม่สามารถดึงข้อมูลเพลงได้ กรุณาลองใหม่ในภายหลัง",
            Self::Message(_) => "เกิดข้อผิดพลาด กรุณาลองใหม่ในภายหลัง",
        }
    }
}
