use std::collections::HashMap;
use std::env;
use std::path::Path;

use thiserror::Error;

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

/// Configuration errors that never contain secret values.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    #[error("DISCORD_TOKEN is not set")]
    MissingDiscordToken,
    #[error("DISCORD_TOKEN is invalid")]
    InvalidDiscordToken,
    #[error("YOUTUBE_COOKIES_PATH is set but does not point to a file")]
    InvalidCookiesPath,
    #[error("YTDLP_BINARY must not be empty")]
    InvalidYtdlpBinary,
}

impl AppConfig {
    /// Load configuration from the process environment, including an optional `.env` file.
    pub fn load() -> Result<Self, ConfigError> {
        if let Err(error) = dotenvy::dotenv() {
            tracing::debug!(error = %error, "no .env file loaded");
        }
        Self::from_vars(env::vars())
    }

    /// Build configuration from an explicit variable set. Useful for tests.
    pub fn from_vars<I, K, V>(vars: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let values: HashMap<String, String> = vars
            .into_iter()
            .map(|(key, value)| (key.as_ref().to_owned(), value.as_ref().to_owned()))
            .collect();
        Self::from_map(&values)
    }

    /// Build and validate configuration from a map of environment-style keys.
    pub fn from_map(values: &HashMap<String, String>) -> Result<Self, ConfigError> {
        let discord_token = values
            .get("DISCORD_TOKEN")
            .map(String::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned();
        if discord_token.is_empty() {
            return Err(ConfigError::MissingDiscordToken);
        }
        if discord_token.chars().any(char::is_whitespace) {
            return Err(ConfigError::InvalidDiscordToken);
        }

        let cookies_path = values
            .get("YOUTUBE_COOKIES_PATH")
            .map(String::as_str)
            .map(str::trim)
            .filter(|path| !path.is_empty());
        let youtube_cookies_path = match cookies_path {
            Some(path) if Path::new(path).is_file() => Some(path.to_owned()),
            Some(_) => return Err(ConfigError::InvalidCookiesPath),
            None => None,
        };

        let ytdlp_binary = values
            .get("YTDLP_BINARY")
            .map(String::as_str)
            .unwrap_or("yt-dlp")
            .trim()
            .to_owned();
        if ytdlp_binary.is_empty() {
            return Err(ConfigError::InvalidYtdlpBinary);
        }

        let config = Self {
            discord_token,
            youtube_cookies_path,
            ytdlp_binary,
        };
        config.validate()?;
        Ok(config)
    }

    /// Re-validate an existing configuration without exposing stored secrets.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.discord_token.trim().is_empty()
            || self.discord_token.chars().any(char::is_whitespace)
        {
            return Err(ConfigError::MissingDiscordToken);
        }
        if let Some(path) = self.youtube_cookies_path.as_deref() {
            if path.trim().is_empty() || !Path::new(path).is_file() {
                return Err(ConfigError::InvalidCookiesPath);
            }
        }
        if self.ytdlp_binary.trim().is_empty() {
            return Err(ConfigError::InvalidYtdlpBinary);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, ConfigError};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn variables(values: &[(&str, &str)]) -> HashMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    fn temporary_cookie_file() -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "wavemelo-cookies-{}-{unique}.txt",
            std::process::id()
        ));
        fs::write(&path, "# Netscape HTTP Cookie File\n").expect("write test cookie file");
        path
    }

    #[test]
    fn loads_token_with_defaults() {
        let config = AppConfig::from_map(&variables(&[("DISCORD_TOKEN", "test-token")]))
            .expect("valid configuration");

        assert_eq!(config.discord_token, "test-token");
        assert_eq!(config.ytdlp_binary, "yt-dlp");
        assert_eq!(config.youtube_cookies_path, None);
    }

    #[test]
    fn loads_custom_ytdlp_binary() {
        let config = AppConfig::from_map(&variables(&[
            ("DISCORD_TOKEN", "test-token"),
            ("YTDLP_BINARY", "  /usr/local/bin/yt-dlp  "),
        ]))
        .expect("valid configuration");

        assert_eq!(config.ytdlp_binary, "/usr/local/bin/yt-dlp");
    }

    #[test]
    fn loads_existing_cookies_file() {
        let file = temporary_cookie_file();
        let path = file.to_string_lossy();
        let config = AppConfig::from_map(&variables(&[
            ("DISCORD_TOKEN", "test-token"),
            ("YOUTUBE_COOKIES_PATH", &path),
        ]))
        .expect("valid configuration");

        assert_eq!(config.youtube_cookies_path.as_deref(), Some(path.as_ref()));
        fs::remove_file(file).expect("remove test cookie file");
    }

    #[test]
    fn rejects_missing_or_empty_token() {
        assert_eq!(
            AppConfig::from_map(&HashMap::new()).unwrap_err(),
            ConfigError::MissingDiscordToken
        );
        assert_eq!(
            AppConfig::from_map(&variables(&[("DISCORD_TOKEN", "   ")])).unwrap_err(),
            ConfigError::MissingDiscordToken
        );
    }

    #[test]
    fn rejects_token_with_whitespace() {
        assert_eq!(
            AppConfig::from_map(&variables(&[("DISCORD_TOKEN", "token with space")])).unwrap_err(),
            ConfigError::InvalidDiscordToken
        );
    }

    #[test]
    fn rejects_missing_cookies_file() {
        assert_eq!(
            AppConfig::from_map(&variables(&[
                ("DISCORD_TOKEN", "test-token"),
                ("YOUTUBE_COOKIES_PATH", "does-not-exist.txt"),
            ]))
            .unwrap_err(),
            ConfigError::InvalidCookiesPath
        );
    }

    #[test]
    fn treats_blank_cookies_path_as_absent() {
        let config = AppConfig::from_map(&variables(&[
            ("DISCORD_TOKEN", "test-token"),
            ("YOUTUBE_COOKIES_PATH", "   "),
        ]))
        .expect("valid configuration");

        assert_eq!(config.youtube_cookies_path, None);
    }

    #[test]
    fn rejects_blank_ytdlp_binary() {
        assert_eq!(
            AppConfig::from_map(&variables(&[
                ("DISCORD_TOKEN", "test-token"),
                ("YTDLP_BINARY", "   "),
            ]))
            .unwrap_err(),
            ConfigError::InvalidYtdlpBinary
        );
    }

    #[test]
    fn validates_existing_configuration() {
        let mut config = AppConfig::from_map(&variables(&[("DISCORD_TOKEN", "test-token")]))
            .expect("valid configuration");
        assert!(config.validate().is_ok());

        config.discord_token.clear();
        assert_eq!(
            config.validate().unwrap_err(),
            ConfigError::MissingDiscordToken
        );
    }
}
