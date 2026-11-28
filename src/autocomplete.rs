//! Slash-command autocomplete backed by bounded local state.

use crate::error::BotError;
use crate::state::BotData;

/// Maximum number of suggestions returned to Discord.
pub const MAX_SUGGESTIONS: usize = 5;
/// Discord's maximum length for an autocomplete choice name.
pub const MAX_CHOICE_CHARS: usize = 100;

const LOOP_MODES: [&str; 3] = ["off", "track", "queue"];

/// Trim a suggestion to Discord's character limit without splitting Unicode.
pub fn clamp_choice(value: &str) -> String {
    let mut trimmed: String = value.chars().take(MAX_CHOICE_CHARS).collect();
    if trimmed.is_empty() {
        trimmed.push(' ');
    }
    trimmed
}

/// Case-insensitive substring filter with a fixed suggestion cap.
pub fn filter_choices<'a>(values: &'a [String], partial: &str) -> Vec<&'a str> {
    let needle = partial.trim().to_ascii_lowercase();
    values
        .iter()
        .map(String::as_str)
        .filter(|value| needle.is_empty() || value.to_ascii_lowercase().contains(&needle))
        .take(MAX_SUGGESTIONS)
        .collect()
}

/// Static suggestions for `/loop`.
pub fn loop_choices(partial: &str) -> Vec<String> {
    LOOP_MODES
        .iter()
        .filter(|mode| {
            let needle = partial.trim().to_ascii_lowercase();
            needle.is_empty() || mode.to_ascii_lowercase().contains(&needle)
        })
        .take(MAX_SUGGESTIONS)
        .map(|mode| clamp_choice(mode))
        .collect()
}

/// Autocomplete callback for `/loop <mode>`.
pub async fn loop_mode(
    _ctx: poise::Context<'_, BotData, BotError>,
    partial: &str,
) -> serenity::builder::CreateAutocompleteResponse {
    let choices = loop_choices(partial)
        .into_iter()
        .map(serenity::builder::AutocompleteChoice::from)
        .collect();
    serenity::builder::CreateAutocompleteResponse::new().set_choices(choices)
}

/// Autocomplete callback for `/play <query>` using bounded local history.
pub async fn play_query(
    ctx: poise::Context<'_, BotData, BotError>,
    partial: &str,
) -> serenity::builder::CreateAutocompleteResponse {
    let recent: Vec<String> = match ctx.guild_id() {
        Some(guild_id) => {
            let session = ctx.data().guild_states.session(guild_id.get());
            let details = session.details.lock().await;
            details.recent_tracks.iter().rev().cloned().collect()
        }
        None => Vec::new(),
    };

    let choices = filter_choices(&recent, partial)
        .into_iter()
        .map(|value| serenity::builder::AutocompleteChoice::from(clamp_choice(value)))
        .collect();
    serenity::builder::CreateAutocompleteResponse::new().set_choices(choices)
}

#[cfg(test)]
mod tests {
    use super::{clamp_choice, filter_choices, loop_choices, MAX_CHOICE_CHARS, MAX_SUGGESTIONS};

    #[test]
    fn loop_choices_follow_the_partial_input() {
        assert_eq!(loop_choices(""), vec!["off", "track", "queue"]);
        assert_eq!(loop_choices("QU"), vec!["queue"]);
        assert!(loop_choices("unknown").is_empty());
    }

    #[test]
    fn clamp_choice_respects_unicode_boundaries() {
        let long = "日".repeat(MAX_CHOICE_CHARS + 10);
        let clamped = clamp_choice(&long);
        assert_eq!(clamped.chars().count(), MAX_CHOICE_CHARS);
        assert_eq!(clamp_choice(""), " ");
    }

    #[test]
    fn filter_choices_is_bounded_and_case_insensitive() {
        let values: Vec<String> = (0..10).map(|index| format!("Track {index}")).collect();
        assert_eq!(filter_choices(&values, "track").len(), MAX_SUGGESTIONS);
        assert_eq!(filter_choices(&values, "TRACK 9"), vec!["Track 9"]);
        assert_eq!(filter_choices(&values, "missing").len(), 0);
    }
}
