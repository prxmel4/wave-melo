use std::time::Duration;

use serenity::builder::CreateEmbed;
use serenity::model::colour::Colour;

use super::colors;
use crate::utils::text::escape_markdown;
use crate::utils::time::format_duration;

fn base(title: impl Into<String>, description: impl Into<String>, color: u32) -> CreateEmbed {
    CreateEmbed::new()
        .title(title)
        .description(description)
        .color(Colour::from(color))
}

/// Blue informational embed.
pub fn info(title: impl Into<String>, description: impl Into<String>) -> CreateEmbed {
    base(title, description, colors::INFO)
}

/// Green success embed.
pub fn success(title: impl Into<String>, description: impl Into<String>) -> CreateEmbed {
    base(title, description, colors::SUCCESS)
}

/// Red error embed. Callers should use a safe user-facing message.
pub fn error(title: impl Into<String>, description: impl Into<String>) -> CreateEmbed {
    base(title, description, colors::ERROR)
}

/// Minimal view of a queue row used by the embed builder.
#[derive(Debug, Clone, Copy)]
pub struct QueueEntryView<'a> {
    pub title: &'a str,
    pub url: Option<&'a str>,
    pub requester: &'a str,
    pub duration: Option<Duration>,
}

/// Format one numbered queue row with escaped user text.
pub fn format_queue_entry(index: usize, entry: &QueueEntryView<'_>) -> String {
    let title = escape_markdown(entry.title);
    let requester = escape_markdown(entry.requester);

    match (entry.url, entry.duration) {
        (Some(url), Some(duration)) => format!(
            "{index}. [{title}](<{url}>) — {} — {requester}",
            format_duration(duration)
        ),
        (Some(url), None) => format!("{index}. [{title}](<{url}>) — {requester}"),
        (None, Some(duration)) => format!(
            "{index}. {title} — {} — {requester}",
            format_duration(duration)
        ),
        (None, None) => format!("{index}. {title} — {requester}"),
    }
}

/// Build the now-playing embed with progress and requester fields.
pub fn now_playing(
    title: &str,
    url: Option<&str>,
    requester: &str,
    duration: Option<Duration>,
    position: Option<Duration>,
) -> CreateEmbed {
    let safe_title = escape_markdown(title);
    let description = match url {
        Some(url) => format!("[{safe_title}](<{url}>)"),
        None => safe_title,
    };

    let progress = match (position, duration) {
        (Some(position), Some(duration)) => format!(
            "{} / {}",
            format_duration(position),
            format_duration(duration)
        ),
        (_, Some(duration)) => format!("? / {}", format_duration(duration)),
        (Some(position), None) => format_duration(position),
        (None, None) => "ไม่ทราบ".to_owned(),
    };

    base("กำลังเล่น", description, colors::NOW_PLAYING)
        .field("ตำแหน่ง", progress, false)
        .field("ขอโดย", escape_markdown(requester), true)
}

/// Build a paginated queue embed.
pub fn queue(entries: &[QueueEntryView<'_>], page: usize, total_pages: usize) -> CreateEmbed {
    let description = if entries.is_empty() {
        "คิวเพลงว่างอยู่".to_owned()
    } else {
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| format_queue_entry(index + 1, entry))
            .collect::<Vec<_>>()
            .join("\n")
    };

    base(
        format!("คิวเพลง (หน้า {}/{})", page.max(1), total_pages.max(1)),
        description,
        colors::QUEUE,
    )
}
