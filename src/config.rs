/// Runtime configuration for the wavemelo bot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    /// Discord bot token used to authenticate the gateway and HTTP client.
    pub discord_token: String,
    /// Optional path to a YouTube cookies file used by yt-dlp.
    pub youtube_cookies_path: Option<String>,
    /// Optional override for the yt-dlp executable name or path.
    pub ytdlp_binary: String,
}
