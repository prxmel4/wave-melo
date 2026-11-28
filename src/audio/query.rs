use crate::error::BotError;

/// A classified playback query ready for Lavalink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    /// A direct URL passed as-is to Lavalink.
    Url(String),
    /// A normalized `ytsearch:` query.
    Search(String),
}

impl Query {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Url(value) | Self::Search(value) => value,
        }
    }
}

fn has_url_scheme(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn looks_like_bare_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "youtube.com/",
        "www.youtube.com/",
        "m.youtube.com/",
        "youtu.be/",
        "music.youtube.com/",
        "open.spotify.com/",
        "soundcloud.com/",
        "deezer.com/",
        "apple.co/",
        "music.apple.com/",
        "bandcamp.com/",
        "twitch.tv/",
        "vimeo.com/",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

/// Normalize a free-text query into a Lavalink search string.
pub fn normalize_search(query: &str) -> String {
    let trimmed = query.trim();
    let lower = trimmed.to_ascii_lowercase();
    if ["ytsearch:", "ytmsearch:", "scsearch:"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    {
        trimmed.to_owned()
    } else {
        format!("ytsearch:{trimmed}")
    }
}

/// Classify user input as a URL or a normalized YouTube search.
pub fn classify(input: &str) -> Result<Query, BotError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(BotError::InvalidQuery("empty query".to_owned()));
    }

    if has_url_scheme(trimmed) {
        Ok(Query::Url(trimmed.to_owned()))
    } else if looks_like_bare_url(trimmed) {
        Ok(Query::Url(format!("https://{trimmed}")))
    } else {
        Ok(Query::Search(normalize_search(trimmed)))
    }
}

#[cfg(test)]
mod tests {
    use super::{classify, normalize_search, Query};
    use crate::error::BotError;

    #[test]
    fn keeps_http_and_https_urls() {
        assert_eq!(
            classify("https://youtu.be/abc123").unwrap(),
            Query::Url("https://youtu.be/abc123".to_owned())
        );
        assert_eq!(
            classify("http://example.com/audio.mp3").unwrap(),
            Query::Url("http://example.com/audio.mp3".to_owned())
        );
    }

    #[test]
    fn adds_scheme_to_bare_youtube_urls() {
        assert_eq!(
            classify("youtube.com/watch?v=abc").unwrap(),
            Query::Url("https://youtube.com/watch?v=abc".to_owned())
        );
        assert_eq!(
            classify("open.spotify.com/track/abc").unwrap(),
            Query::Url("https://open.spotify.com/track/abc".to_owned())
        );
    }

    #[test]
    fn normalizes_plain_text_to_ytsearch() {
        assert_eq!(
            classify("泰国 เพลง").unwrap(),
            Query::Search("ytsearch:泰国 เพลง".to_owned())
        );
    }

    #[test]
    fn preserves_existing_search_prefixes() {
        for prefix in ["ytsearch:", "ytmsearch:", "scsearch:", "YTSearch:"] {
            let query = format!("{prefix}test");
            assert_eq!(normalize_search(&query), query);
            assert_eq!(classify(&query).unwrap(), Query::Search(query));
        }
    }

    #[test]
    fn rejects_empty_input() {
        assert!(matches!(classify(""), Err(BotError::InvalidQuery(_))));
        assert!(matches!(
            classify("   \t\n"),
            Err(BotError::InvalidQuery(_))
        ));
    }

    #[test]
    fn query_as_str_returns_inner_value() {
        let url = classify("https://example.com").unwrap();
        assert_eq!(url.as_str(), "https://example.com");

        let search = classify("hello").unwrap();
        assert_eq!(search.as_str(), "ytsearch:hello");
    }
}
