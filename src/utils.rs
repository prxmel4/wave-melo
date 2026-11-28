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
/// Counting characters (not bytes) keeps Unicode and other multi-byte text safe.
pub fn truncate(text: &str, max_len: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_len {
        return text.to_owned();
    }
    if max_len <= 3 {
        return text.chars().take(max_len).collect();
    }

    let truncated: String = text.chars().take(max_len - 3).collect();
    format!("{truncated}...")
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
        assert_eq!(truncate("abcdef", 3), "abc");
    }

    #[test]
    fn truncation_never_exceeds_the_requested_length() {
        for limit in 0..=3 {
            let result = truncate("abcdef", limit);
            assert!(result.chars().count() <= limit, "limit {limit}");
        }
        assert_eq!(truncate("abcdef", 0), "");
        assert_eq!(truncate("abcdef", 1), "a");
        assert_eq!(truncate("abcdef", 2), "ab");
    }

    #[test]
    fn truncation_preserves_unicode_boundaries() {
        let text = "🎵 café 日本語";
        for limit in 1..=text.chars().count() {
            let result = truncate(text, limit);
            assert!(result.chars().count() <= limit, "limit {limit}");
            let visible = result.strip_suffix("...").unwrap_or(&result);
            assert!(text.starts_with(visible), "limit {limit}");
        }
        assert_eq!(truncate("日本語の曲", 4), "日...");
    }

    /// Verify the English-only source policy without external tooling.
    #[test]
    fn source_contains_no_thai_characters() {
        fn collect(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            let entries = std::fs::read_dir(dir).expect("src directory is readable");
            for entry in entries {
                let path = entry.expect("directory entry").path();
                if path.is_dir() {
                    collect(&path, files);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    files.push(path);
                }
            }
        }

        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect(&src, &mut files);
        assert!(!files.is_empty(), "source files were found");

        for path in files {
            let contents = std::fs::read_to_string(&path).expect("source file is UTF-8");
            let thai = contents
                .chars()
                .filter(|c| ('\u{0E00}'..='\u{0E7F}').contains(c))
                .count();
            assert_eq!(thai, 0, "Thai characters remain in {}", path.display());
        }
    }
}
