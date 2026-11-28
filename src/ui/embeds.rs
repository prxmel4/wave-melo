use std::time::Duration;

use serenity::all::CreateEmbed;

use crate::utils::{format_duration_ms, truncate};

const COLOR_SUCCESS: u32 = 0x002E_CC71; // Green
const COLOR_ERROR: u32 = 0x00E7_4C3C; // Red
const COLOR_INFO: u32 = 0x0034_98DB; // Blue
const COLOR_QUEUE: u32 = 0x009B_59B6; // Purple

/// Build a success embed with a green accent.
pub fn success(title: &str, description: impl Into<String>) -> CreateEmbed {
    CreateEmbed::new()
        .title(title)
        .description(description.into())
        .color(COLOR_SUCCESS)
}

/// Build an error embed with a red accent.
pub fn error(title: &str, description: impl Into<String>) -> CreateEmbed {
    CreateEmbed::new()
        .title(title)
        .description(description.into())
        .color(COLOR_ERROR)
}

/// Build an informational embed with a blue accent.
pub fn info(title: &str, description: impl Into<String>) -> CreateEmbed {
    CreateEmbed::new()
        .title(title)
        .description(description.into())
        .color(COLOR_INFO)
}

/// A single queue entry for display purposes.
pub struct QueueEntryView<'a> {
    pub title: &'a str,
    pub url: Option<&'a str>,
    pub requester: &'a str,
    pub duration: Option<Duration>,
}

/// Format one queue row. `position` is the rendered label or queue number.
fn queue_line(position: &str, entry: &QueueEntryView<'_>) -> String {
    let title_display = truncate(entry.title, 60);
    let duration_display = entry
        .duration
        .map(|d| format_duration_ms(d.as_millis() as u64))
        .unwrap_or_else(|| "🔴 LIVE".to_owned());
    let requester = entry.requester;

    if let Some(url) = entry.url {
        format!(
            "{position} [{title_display}]({url}) `[{duration_display}]` • Added by {requester}\n"
        )
    } else {
        format!("{position} {title_display} `[{duration_display}]` • Added by {requester}\n")
    }
}

/// Build a queue display embed.
///
/// `current` is the playing track (shown separately, without a queue number),
/// `entries` are the visible queued tracks numbered from 1, and `queued_count`
/// is the total number of tracks waiting in the queue.
pub fn queue(
    current: Option<&QueueEntryView<'_>>,
    entries: &[QueueEntryView<'_>],
    queued_count: usize,
) -> CreateEmbed {
    let mut description = String::new();

    if let Some(entry) = current {
        description.push_str(&queue_line("🎵 **Now playing:**", entry));
    }

    if current.is_none() && entries.is_empty() {
        description.push_str("*Queue is empty*\n");
    }

    for (index, entry) in entries.iter().enumerate() {
        let position = format!("**{}.**", index + 1);
        description.push_str(&queue_line(&position, entry));
    }

    if queued_count > entries.len() {
        let remaining = queued_count - entries.len();
        description.push_str(&format!("…and {remaining} more tracks\n"));
    }

    let footer_text = format!("{queued_count} tracks in queue");

    CreateEmbed::new()
        .title("🎶 Queue")
        .description(description)
        .color(COLOR_QUEUE)
        .footer(serenity::all::CreateEmbedFooter::new(footer_text))
}

/// Build a "now playing" embed with track metadata.
pub fn now_playing(
    title: &str,
    url: Option<&str>,
    requester: &str,
    duration: Option<Duration>,
    queue_position: Option<usize>,
) -> CreateEmbed {
    let title_display = truncate(title, 80);
    let duration_display = duration
        .map(|d| format_duration_ms(d.as_millis() as u64))
        .unwrap_or_else(|| "🔴 LIVE".to_owned());

    let description = if let Some(url) = url {
        format!("[{title_display}]({url})")
    } else {
        title_display
    };

    let mut embed = CreateEmbed::new()
        .title("🎵 Now Playing")
        .description(description)
        .color(COLOR_INFO)
        .field("Duration", format!("`{duration_display}`"), true)
        .field("Added by", requester, true);

    if let Some(pos) = queue_position {
        embed = embed.field("Queue Position", format!("#{pos}"), true);
    }

    embed
}
