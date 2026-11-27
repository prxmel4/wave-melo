use crate::error::BotError;

/// A classified playback query ready for yt-dlp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    /// A direct URL, normalized to include a scheme.
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
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

/// Normalize a free-text query into a yt-dlp search string.
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
