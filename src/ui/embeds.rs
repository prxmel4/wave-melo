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

#[cfg(test)]
mod tests {
    use super::{error, format_queue_entry, info, now_playing, queue, success, QueueEntryView};
    use crate::ui::colors;
    use serde_json::Value;
    use serenity::builder::CreateEmbed;
    use std::time::Duration;

    fn as_value(embed: CreateEmbed) -> Value {
        serde_json::to_value(embed).expect("serialize embed")
    }

    #[test]
    fn info_success_error_set_title_description_and_color() {
        let cases = [
            (info("Info", "Details"), colors::INFO),
            (success("Done", "Saved"), colors::SUCCESS),
            (error("Oops", "Try again"), colors::ERROR),
        ];

        for (embed, color) in cases {
            let value = as_value(embed);
            assert_eq!(value["color"].as_u64(), Some(u64::from(color)));
            assert!(value["title"].is_string());
            assert!(value["description"].is_string());
        }

        assert_eq!(as_value(info("Info", "Details"))["title"], "Info");
        assert_eq!(
            as_value(error("Oops", "Try again"))["description"],
            "Try again"
        );
    }

    #[test]
    fn now_playing_formats_progress_and_escapes_text() {
        let embed = as_value(now_playing(
            "เพลง*unya",
            Some("https://youtu.be/example"),
            "@user",
            Some(Duration::from_secs(125)),
            Some(Duration::from_secs(5)),
        ));

        assert_eq!(embed["title"], "กำลังเล่น");
        assert_eq!(embed["color"], colors::NOW_PLAYING);
        let fields = embed["fields"].as_array().expect("fields array");
        assert_eq!(fields[0]["name"], "ตำแหน่ง");
        assert_eq!(fields[0]["value"], "00:05 / 02:05");
        assert_eq!(fields[1]["name"], "ขอโดย");
        assert_eq!(fields[1]["value"], "\\@user");
        assert!(embed["description"]
            .as_str()
            .expect("description")
            .contains("\\*"));
    }

    #[test]
    fn now_playing_handles_missing_duration() {
        let embed = as_value(now_playing(
            "Title",
            None,
            "user",
            None,
            Some(Duration::from_secs(9)),
        ));
        let fields = embed["fields"].as_array().expect("fields array");
        assert_eq!(fields[0]["value"], "00:09");
    }

    #[test]
    fn queue_embed_reports_empty_queue() {
        let embed = as_value(queue(&[], 1, 1));
        assert_eq!(embed["description"], "คิวเพลงว่างอยู่");
        assert_eq!(embed["color"], colors::QUEUE);
    }

    #[test]
    fn formats_queue_entry_with_url_and_duration() {
        let entry = QueueEntryView {
            title: "a*b",
            url: Some("https://example.com/audio"),
            requester: "user_name",
            duration: Some(Duration::from_secs(61)),
        };

        assert_eq!(
            format_queue_entry(2, &entry),
            "2. [a\\*b](<https://example.com/audio>) — 01:01 — user\\_name"
        );
    }

    #[test]
    fn formats_queue_entry_without_url_or_duration() {
        let entry = QueueEntryView {
            title: "เพลงไทย",
            url: None,
            requester: "คนขอ",
            duration: None,
        };

        assert_eq!(format_queue_entry(1, &entry), "1. เพลงไทย — คนขอ");
    }

    #[test]
    fn queue_embed_joins_rows_and_clamps_page() {
        let entries = [
            QueueEntryView {
                title: "First",
                url: None,
                requester: "A",
                duration: Some(Duration::from_secs(3)),
            },
            QueueEntryView {
                title: "Second",
                url: None,
                requester: "B",
                duration: None,
            },
        ];
        let embed = as_value(queue(&entries, 0, 0));

        assert_eq!(embed["title"], "คิวเพลง (หน้า 1/1)");
        assert_eq!(embed["description"], "1. First — 00:03 — A\n2. Second — B");
    }
}
