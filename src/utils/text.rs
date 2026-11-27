use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;

const ELLIPSIS: &str = "\u{2026}";

/// Truncate text to at most `max_graphemes` extended grapheme clusters.
///
/// When truncation is needed, the result ends with an ellipsis and the total
/// grapheme count remains at most `max_graphemes`. This avoids splitting UTF-8
/// sequences, Thai grapheme clusters, and emoji sequences.
pub fn truncate_graphemes(text: &str, max_graphemes: usize) -> Cow<'_, str> {
    if max_graphemes == 0 {
        return Cow::Borrowed("");
    }

    if text.graphemes(true).nth(max_graphemes).is_none() {
        return Cow::Borrowed(text);
    }

    if max_graphemes == 1 {
        return Cow::Borrowed(ELLIPSIS);
    }

    let mut truncated: String = text.graphemes(true).take(max_graphemes - 1).collect();
    truncated.push_str(ELLIPSIS);
    Cow::Owned(truncated)
}

/// Escape Discord markdown control characters so user-provided text renders literally.
pub fn escape_markdown(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());

    for ch in text.chars() {
        if matches!(ch, '\\' | '*' | '_' | '~' | '`' | '|' | '>' | '@') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }

    escaped
}

#[cfg(test)]
mod tests {
    use super::{escape_markdown, truncate_graphemes};
    use std::borrow::Cow;
    use unicode_segmentation::UnicodeSegmentation;

    #[test]
    fn returns_input_unchanged_when_it_fits() {
        assert!(matches!(
            truncate_graphemes("hello", 5),
            Cow::Borrowed("hello")
        ));
        assert!(matches!(
            truncate_graphemes("hello", 10),
            Cow::Borrowed("hello")
        ));
    }

    #[test]
    fn zero_limit_returns_empty() {
        assert_eq!(truncate_graphemes("hello", 0), "");
    }

    #[test]
    fn single_limit_returns_only_ellipsis() {
        assert_eq!(truncate_graphemes("hello", 1), "\u{2026}");
    }

    #[test]
    fn truncates_ascii_with_ellipsis_inside_limit() {
        let truncated = truncate_graphemes("abcdef", 4);
        assert_eq!(truncated, "abc\u{2026}");
        assert!(truncated.graphemes(true).count() <= 4);
    }

    #[test]
    fn truncates_thai_without_splitting_combining_marks() {
        let truncated = truncate_graphemes("สวัสดี", 3);
        assert_eq!(truncated, "สวั\u{2026}");
        assert!(truncated.graphemes(true).count() <= 3);
    }

    #[test]
    fn truncates_on_grapheme_boundaries_not_byte_boundaries() {
        let truncated = truncate_graphemes("กขคง", 2);
        assert_eq!(truncated, "ก\u{2026}");
        assert_eq!(truncated.chars().count(), 2);
    }

    #[test]
    fn keeps_emoji_sequences_intact() {
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
        let input = format!("{family}hello");
        let truncated = truncate_graphemes(&input, 2);
        assert_eq!(truncated, format!("{family}\u{2026}"));
    }

    #[test]
    fn escapes_markdown_control_characters() {
        assert_eq!(
            escape_markdown("a*b_c~d`e|f>g@h\\i"),
            "a\\*b\\_c\\~d\\`e\\|f\\>g\\@h\\\\i"
        );
    }

    #[test]
    fn leaves_plain_thai_text_unchanged() {
        assert_eq!(escape_markdown("เพลงไทย"), "เพลงไทย");
    }

    #[test]
    fn escapes_empty_text() {
        assert_eq!(escape_markdown(""), "");
    }
}
