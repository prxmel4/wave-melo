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
