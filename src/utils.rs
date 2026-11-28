/// Format a duration in milliseconds as a human-readable string (e.g., "3:45" or "1:02:30").
pub fn format_duration_ms(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Truncate a string to a maximum character count, appending "..." if truncated.
///
/// Counting characters (not bytes) keeps Thai and other multi-byte text safe.
pub fn truncate(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_owned()
    } else {
        let truncated: String = text.chars().take(max_len.saturating_sub(3)).collect();
        format!("{truncated}...")
    }
}

#[cfg(test)]
mod tests {
    use super::{format_duration_ms, truncate};

    #[test]
    fn formats_short_durations() {
        assert_eq!(format_duration_ms(0), "0:00");
        assert_eq!(format_duration_ms(1000), "0:01");
        assert_eq!(format_duration_ms(60_000), "1:00");
        assert_eq!(format_duration_ms(225_000), "3:45");
    }

    #[test]
    fn formats_long_durations_with_hours() {
        assert_eq!(format_duration_ms(3_723_000), "1:02:03");
        assert_eq!(format_duration_ms(36_000_000), "10:00:00");
    }

    #[test]
    fn truncates_long_strings() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a very long title", 10), "this is...");
        assert_eq!(truncate("exact", 5), "exact");
    }

    #[test]
    fn handles_edge_cases() {
        assert_eq!(truncate("", 10), "");
        assert_eq!(truncate("abc", 3), "abc");
        assert_eq!(truncate("abcdef", 3), "...");
    }
}
