use std::time::Duration;

/// Format a duration as `MM:SS`, or `HH:MM:SS` when it spans an hour.
pub fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::format_duration;
    use std::time::Duration;

    #[test]
    fn formats_zero() {
        assert_eq!(format_duration(Duration::ZERO), "00:00");
    }

    #[test]
    fn formats_seconds_under_a_minute() {
        assert_eq!(format_duration(Duration::from_secs(1)), "00:01");
        assert_eq!(format_duration(Duration::from_secs(59)), "00:59");
    }

    #[test]
    fn formats_minutes_and_seconds() {
        assert_eq!(format_duration(Duration::from_secs(60)), "01:00");
        assert_eq!(format_duration(Duration::from_secs(61)), "01:01");
        assert_eq!(format_duration(Duration::from_secs(599)), "09:59");
    }

    #[test]
    fn switches_to_hours_at_one_hour() {
        assert_eq!(format_duration(Duration::from_secs(3599)), "59:59");
        assert_eq!(format_duration(Duration::from_secs(3600)), "01:00:00");
        assert_eq!(format_duration(Duration::from_secs(3601)), "01:00:01");
    }

    #[test]
    fn formats_hours_minutes_seconds() {
        assert_eq!(format_duration(Duration::from_secs(3661)), "01:01:01");
        assert_eq!(format_duration(Duration::from_secs(86399)), "23:59:59");
    }

    #[test]
    fn ignores_subsecond_precision() {
        assert_eq!(format_duration(Duration::from_millis(999)), "00:00");
        assert_eq!(format_duration(Duration::from_millis(1_001)), "00:01");
    }

    #[test]
    fn supports_long_durations() {
        assert_eq!(format_duration(Duration::from_secs(360_000)), "100:00:00");
    }
}
