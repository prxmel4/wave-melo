//! Pure seek parsing, validation, and progress-bar rendering.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekParseError {
    Empty,
    TooManyParts,
    EmptyComponent,
    NonAsciiDigit,
    InvalidComponentLength,
    MinutesOrSecondsOutOfRange,
    Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekValidationError {
    LiveOrUnknownLength,
    NotSeekable,
    BeyondTrackLength { target_ms: u64, length_ms: u64 },
}

/// Parse `90`, `01:30`, or `00:01:30` into a duration.
///
/// The parser is deliberately strict: no signs, whitespace, Unicode digits,
/// empty components, or out-of-range minute/second fields are accepted.
pub fn parse_seek(input: &str) -> Result<Duration, SeekParseError> {
    if input.is_empty() || input.trim() != input {
        return Err(SeekParseError::Empty);
    }

    let parts = input.split(':').collect::<Vec<_>>();
    let total_seconds = match parts.as_slice() {
        [seconds] => parse_number(seconds, 18)?,
        [minutes, seconds] => {
            parse_time_component(minutes, 2)? * 60 + parse_time_component(seconds, 2)?
        }
        [hours, minutes, seconds] => {
            parse_number(hours, 6)? * 3600
                + parse_time_component(minutes, 2)? * 60
                + parse_time_component(seconds, 2)?
        }
        _ => return Err(SeekParseError::TooManyParts),
    };

    let millis = total_seconds
        .checked_mul(1000)
        .ok_or(SeekParseError::Overflow)?;
    Ok(Duration::from_millis(millis))
}

fn parse_number(component: &str, max_digits: usize) -> Result<u64, SeekParseError> {
    if component.is_empty() {
        return Err(SeekParseError::EmptyComponent);
    }
    if component.len() > max_digits {
        return Err(SeekParseError::InvalidComponentLength);
    }
    if !component.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(SeekParseError::NonAsciiDigit);
    }
    component
        .parse::<u64>()
        .map_err(|_| SeekParseError::Overflow)
}

fn parse_time_component(component: &str, max_digits: usize) -> Result<u64, SeekParseError> {
    let value = parse_number(component, max_digits)?;
    if value >= 60 {
        return Err(SeekParseError::MinutesOrSecondsOutOfRange);
    }
    Ok(value)
}

/// Validate a parsed seek target against the authoritative track metadata.
pub fn validate_seek_target(
    target: Duration,
    is_seekable: bool,
    is_stream: bool,
    length_ms: u64,
) -> Result<u64, SeekValidationError> {
    if is_stream || length_ms == 0 {
        return Err(SeekValidationError::LiveOrUnknownLength);
    }
    if !is_seekable {
        return Err(SeekValidationError::NotSeekable);
    }
    let target_ms = u64::try_from(target.as_millis()).unwrap_or(u64::MAX);
    if target_ms >= length_ms {
        return Err(SeekValidationError::BeyondTrackLength {
            target_ms,
            length_ms,
        });
    }
    Ok(target_ms)
}

/// Render a progress indicator without dividing by zero for live or unknown lengths.
pub fn progress_bar(position_ms: u64, length_ms: Option<u64>, width: usize) -> String {
    let Some(length) = length_ms.filter(|length| *length > 0) else {
        return "🔴 LIVE".to_owned();
    };
    let width = width.clamp(1, 32);
    let position = position_ms.min(length);
    let filled = (position as u128 * width as u128 / length as u128) as usize;
    format!(
        "{}{}{}",
        "━".repeat(filled),
        "🔘",
        "─".repeat(width - filled)
    )
}

#[cfg(test)]
mod tests {
    use super::{
        parse_seek, progress_bar, validate_seek_target, SeekParseError, SeekValidationError,
    };
    use std::time::Duration;

    #[test]
    fn parses_supported_formats() {
        assert_eq!(parse_seek("90").unwrap(), Duration::from_millis(90_000));
        assert_eq!(parse_seek("0").unwrap(), Duration::ZERO);
        assert_eq!(parse_seek("01:30").unwrap(), Duration::from_millis(90_000));
        assert_eq!(
            parse_seek("00:01:30").unwrap(),
            Duration::from_millis(90_000)
        );
        assert_eq!(
            parse_seek("1:02:03").unwrap(),
            Duration::from_millis(3_723_000)
        );
    }

    #[test]
    fn rejects_malformed_input() {
        for (input, expected) in [
            ("", SeekParseError::Empty),
            (" 90", SeekParseError::Empty),
            ("90 ", SeekParseError::Empty),
            ("1:2:3:4", SeekParseError::TooManyParts),
            ("01:", SeekParseError::EmptyComponent),
            (":30", SeekParseError::EmptyComponent),
            ("1::2", SeekParseError::EmptyComponent),
            ("-5", SeekParseError::NonAsciiDigit),
            ("+90", SeekParseError::NonAsciiDigit),
            ("١٢", SeekParseError::NonAsciiDigit),
            ("0:60", SeekParseError::MinutesOrSecondsOutOfRange),
            ("1:2:99", SeekParseError::MinutesOrSecondsOutOfRange),
            ("999999999999999999", SeekParseError::Overflow),
        ] {
            assert_eq!(parse_seek(input), Err(expected), "input {input:?}");
        }
    }

    #[test]
    fn validates_targets_against_track_metadata() {
        assert_eq!(
            validate_seek_target(Duration::from_secs(1), true, false, 60_000).unwrap(),
            1000
        );
        assert_eq!(
            validate_seek_target(Duration::from_secs(1), true, true, 60_000),
            Err(SeekValidationError::LiveOrUnknownLength)
        );
        assert_eq!(
            validate_seek_target(Duration::from_secs(1), true, false, 0),
            Err(SeekValidationError::LiveOrUnknownLength)
        );
        assert_eq!(
            validate_seek_target(Duration::from_secs(1), false, false, 60_000),
            Err(SeekValidationError::NotSeekable)
        );
        assert_eq!(
            validate_seek_target(Duration::from_secs(90), true, false, 60_000),
            Err(SeekValidationError::BeyondTrackLength {
                target_ms: 90_000,
                length_ms: 60_000,
            })
        );
    }

    #[test]
    fn renders_progress_safely() {
        assert_eq!(progress_bar(1000, Some(4000), 4), "━🔘───");
        assert_eq!(progress_bar(0, None, 8), "🔴 LIVE");
        assert_eq!(progress_bar(0, Some(0), 8), "🔴 LIVE");
        assert_eq!(progress_bar(9000, Some(4000), 4), "━━━━🔘");
        assert_eq!(progress_bar(0, Some(1000), 0).chars().count(), 2);
    }
}
