use std::collections::HashMap;
use std::env;

use serenity::model::id::GuildId;
use thiserror::Error;

/// Runtime configuration for the wavemelo bot.
#[derive(Clone, PartialEq, Eq)]
pub struct AppConfig {
    /// Discord bot token used to authenticate the gateway and HTTP client.
    pub discord_token: String,
    /// Optional guild ID for instant slash command registration during development.
    pub discord_guild_id: Option<GuildId>,
    /// Lavalink server hostname or IP address.
    pub lavalink_host: String,
    /// Lavalink server port.
    pub lavalink_port: u16,
    /// Lavalink server authentication password.
    pub lavalink_password: String,
    /// Whether to use TLS/WSS for the Lavalink connection.
    pub lavalink_secure: bool,
}

impl std::fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppConfig")
            .field("discord_token", &"[redacted]")
            .field("discord_guild_id", &self.discord_guild_id)
            .field("lavalink_host", &self.lavalink_host)
            .field("lavalink_port", &self.lavalink_port)
            .field("lavalink_password", &"[redacted]")
            .field("lavalink_secure", &self.lavalink_secure)
            .finish()
    }
}

/// Configuration errors that never contain secret values.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    #[error("DISCORD_TOKEN is not set")]
    MissingDiscordToken,
    #[error("DISCORD_TOKEN is invalid")]
    InvalidDiscordToken,
    #[error("DISCORD_GUILD_ID must be a non-zero integer")]
    InvalidGuildId,
    #[error("LAVALINK_HOST is not set")]
    MissingLavalinkHost,
    #[error("LAVALINK_PORT must be between 1 and 65535")]
    InvalidLavalinkPort,
    #[error("LAVALINK_PASSWORD is not set")]
    MissingLavalinkPassword,
    #[error("LAVALINK_SECURE must be 'true' or 'false'")]
    InvalidLavalinkSecure,
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

        let guild_id_raw = values
            .get("DISCORD_GUILD_ID")
            .map(String::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty());
        let discord_guild_id = match guild_id_raw {
            Some(raw) => {
                let id = raw
                    .parse::<u64>()
                    .map_err(|_| ConfigError::InvalidGuildId)?;
                if id == 0 {
                    return Err(ConfigError::InvalidGuildId);
                }
                Some(GuildId::new(id))
            }
            None => None,
        };

        let lavalink_host = values
            .get("LAVALINK_HOST")
            .map(String::as_str)
            .unwrap_or("127.0.0.1")
            .trim()
            .to_owned();
        if lavalink_host.is_empty() {
            return Err(ConfigError::MissingLavalinkHost);
        }

        let lavalink_port = values
            .get("LAVALINK_PORT")
            .map(String::as_str)
            .unwrap_or("2333")
            .trim()
            .parse::<u16>()
            .map_err(|_| ConfigError::InvalidLavalinkPort)?;
        if lavalink_port == 0 {
            return Err(ConfigError::InvalidLavalinkPort);
        }

        let lavalink_password = values
            .get("LAVALINK_PASSWORD")
            .map(String::as_str)
            .unwrap_or("youshallnotpass")
            .trim()
            .to_owned();
        if lavalink_password.is_empty() {
            return Err(ConfigError::MissingLavalinkPassword);
        }

        let lavalink_secure = match values
            .get("LAVALINK_SECURE")
            .map(String::as_str)
            .unwrap_or("false")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "true" | "1" | "yes" => true,
            "false" | "0" | "no" => false,
            _ => return Err(ConfigError::InvalidLavalinkSecure),
        };

        let config = Self {
            discord_token,
            discord_guild_id,
            lavalink_host,
            lavalink_port,
            lavalink_password,
            lavalink_secure,
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
        if self.lavalink_host.trim().is_empty() {
            return Err(ConfigError::MissingLavalinkHost);
        }
        if self.lavalink_port == 0 {
            return Err(ConfigError::InvalidLavalinkPort);
        }
        if self.lavalink_password.trim().is_empty() {
            return Err(ConfigError::MissingLavalinkPassword);
        }
        Ok(())
    }

    /// Format the Lavalink server address as `host:port`.
    pub fn lavalink_address(&self) -> String {
        format!("{}:{}", self.lavalink_host, self.lavalink_port)
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, ConfigError};
    use serenity::model::id::GuildId;
    use std::collections::HashMap;

    fn variables(values: &[(&str, &str)]) -> HashMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn loads_token_with_defaults() {
        let config = AppConfig::from_map(&variables(&[("DISCORD_TOKEN", "test-token")]))
            .expect("valid configuration");

        assert_eq!(config.discord_token, "test-token");
        assert_eq!(config.lavalink_host, "127.0.0.1");
        assert_eq!(config.lavalink_port, 2333);
        assert_eq!(config.lavalink_password, "youshallnotpass");
        assert!(!config.lavalink_secure);
        assert_eq!(config.discord_guild_id, None);
    }

    #[test]
    fn loads_guild_id_for_fast_registration() {
        let config = AppConfig::from_map(&variables(&[
            ("DISCORD_TOKEN", "test-token"),
            ("DISCORD_GUILD_ID", "  123456789012345678  "),
        ]))
        .expect("valid configuration");

        assert_eq!(
            config.discord_guild_id,
            Some(GuildId::new(123_456_789_012_345_678))
        );
    }

    #[test]
    fn treats_blank_guild_id_as_absent() {
        let config = AppConfig::from_map(&variables(&[
            ("DISCORD_TOKEN", "test-token"),
            ("DISCORD_GUILD_ID", "   "),
        ]))
        .expect("valid configuration");

        assert_eq!(config.discord_guild_id, None);
    }

    #[test]
    fn rejects_invalid_guild_id() {
        for value in ["not-a-number", "0"] {
            assert_eq!(
                AppConfig::from_map(&variables(&[
                    ("DISCORD_TOKEN", "test-token"),
                    ("DISCORD_GUILD_ID", value),
                ]))
                .unwrap_err(),
                ConfigError::InvalidGuildId
            );
        }
    }

    #[test]
    fn loads_custom_lavalink_settings() {
        let config = AppConfig::from_map(&variables(&[
            ("DISCORD_TOKEN", "test-token"),
            ("LAVALINK_HOST", "  lavalink.example.com  "),
            ("LAVALINK_PORT", "  8080  "),
            ("LAVALINK_PASSWORD", "  mysecretpass  "),
            ("LAVALINK_SECURE", "  true  "),
        ]))
        .expect("valid configuration");

        assert_eq!(config.lavalink_host, "lavalink.example.com");
        assert_eq!(config.lavalink_port, 8080);
        assert_eq!(config.lavalink_password, "mysecretpass");
        assert!(config.lavalink_secure);
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
    fn rejects_invalid_lavalink_port() {
        for value in ["not-a-number", "0", "70000"] {
            assert_eq!(
                AppConfig::from_map(&variables(&[
                    ("DISCORD_TOKEN", "test-token"),
                    ("LAVALINK_PORT", value),
                ]))
                .unwrap_err(),
                ConfigError::InvalidLavalinkPort
            );
        }
    }

    #[test]
    fn rejects_empty_lavalink_password() {
        assert_eq!(
            AppConfig::from_map(&variables(&[
                ("DISCORD_TOKEN", "test-token"),
                ("LAVALINK_PASSWORD", "   "),
            ]))
            .unwrap_err(),
            ConfigError::MissingLavalinkPassword
        );
    }

    #[test]
    fn rejects_invalid_lavalink_secure() {
        assert_eq!(
            AppConfig::from_map(&variables(&[
                ("DISCORD_TOKEN", "test-token"),
                ("LAVALINK_SECURE", "maybe"),
            ]))
            .unwrap_err(),
            ConfigError::InvalidLavalinkSecure
        );
    }

    #[test]
    fn accepts_various_boolean_values() {
        for (value, expected) in [
            ("true", true),
            ("1", true),
            ("yes", true),
            ("false", false),
            ("0", false),
            ("no", false),
        ] {
            let config = AppConfig::from_map(&variables(&[
                ("DISCORD_TOKEN", "test-token"),
                ("LAVALINK_SECURE", value),
            ]))
            .expect("valid configuration");
            assert_eq!(
                config.lavalink_secure, expected,
                "failed for value: {value}"
            );
        }
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

    #[test]
    fn formats_lavalink_address() {
        let config = AppConfig::from_map(&variables(&[
            ("DISCORD_TOKEN", "test-token"),
            ("LAVALINK_HOST", "myhost"),
            ("LAVALINK_PORT", "9999"),
        ]))
        .expect("valid configuration");

        assert_eq!(config.lavalink_address(), "myhost:9999");
    }

    #[test]
    fn redacts_secrets_in_debug() {
        let config = AppConfig::from_map(&variables(&[
            ("DISCORD_TOKEN", "secret-token"),
            ("LAVALINK_PASSWORD", "secret-pass"),
        ]))
        .expect("valid configuration");

        let debug = format!("{:?}", config);
        assert!(!debug.contains("secret-token"));
        assert!(!debug.contains("secret-pass"));
        assert!(debug.contains("[redacted]"));
    }
}
