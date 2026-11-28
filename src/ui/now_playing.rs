//! Now Playing embed rendering. Pure view data in, Serenity embed out.

use serenity::all::CreateEmbed;
use serenity::model::id::GuildId;

use crate::error::BotError;
use crate::playback::seek::progress_bar;
use crate::state::BotData;
use crate::utils::{format_duration_ms, truncate};

/// Owned presentation data for the Now Playing embed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlayingView {
    pub title: String,
    pub url: Option<String>,
    pub author: String,
    pub requester: String,
    pub position_ms: u64,
    pub length_ms: Option<u64>,
    pub volume: u16,
    pub loop_mode: String,
    pub active_filter: String,
    pub paused: bool,
    pub connected: bool,
    /// Generation stamped into interactive controls for stale-button protection.
    pub generation: u64,
}

fn safe_http_url(url: Option<&str>) -> Option<&str> {
    url.filter(|value| {
        value.chars().count() <= 512
            && (value.starts_with("https://") || value.starts_with("http://"))
    })
}

/// Collect a consistent snapshot of remote player state for presentation.
///
/// All remote reads happen here so slash commands and interactive controls
/// render the same view from the same source of truth.
pub async fn collect_view(data: &BotData, guild_id: GuildId) -> Result<NowPlayingView, BotError> {
    let player_ctx = crate::playback::ops::require_player(data, guild_id)?;
    let player = player_ctx
        .get_player()
        .await
        .map_err(|error| BotError::Lavalink(error.to_string()))?;
    let track = player.track.ok_or(BotError::NoActivePlayer)?;

    let requester = track
        .user_data
        .as_ref()
        .and_then(|data| data.get("requester"))
        .and_then(|value| value.as_str())
        .unwrap_or("Unknown user")
        .to_owned();
    let length_ms = (!track.info.is_stream && track.info.length > 0).then_some(track.info.length);
    let session = data.guild_states.session(guild_id.get());
    let loop_mode = session.loop_control.mode();
    let (active_filter, generation) = {
        let details = session.details.lock().await;
        (
            details
                .active_filter
                .clone()
                .unwrap_or_else(|| "None".to_owned()),
            details.control_generation(),
        )
    };

    Ok(NowPlayingView {
        title: track.info.title.clone(),
        url: Some(crate::audio::loader::track_url(&track)),
        author: track.info.author.clone(),
        requester,
        position_ms: player.state.position,
        length_ms,
        volume: player.volume,
        loop_mode: loop_mode.as_str().to_owned(),
        active_filter,
        paused: player.paused,
        connected: player.state.connected,
        generation,
    })
}

/// Build the embed. Live streams and unknown durations never trigger division.
pub fn build(view: &NowPlayingView) -> CreateEmbed {
    let title = truncate(&view.title, 80);
    let description = match safe_http_url(view.url.as_deref()) {
        Some(url) => format!("[{title}]({url})"),
        None => title,
    };
    let duration_label = view
        .length_ms
        .map(format_duration_ms)
        .unwrap_or_else(|| "LIVE".to_owned());
    let position_label = format!(
        "{} / {}",
        format_duration_ms(view.position_ms),
        duration_label
    );
    let status = if !view.connected {
        "Disconnected"
    } else if view.length_ms.is_none() {
        "Playing (live)"
    } else if view.paused {
        "Paused"
    } else {
        "Playing"
    };

    CreateEmbed::new()
        .title("🎵 Now Playing")
        .description(description)
        .color(0x0034_98DB)
        .field("Artist", truncate(&view.author, 100), true)
        .field("Requested by", truncate(&view.requester, 100), true)
        .field("Status", status, true)
        .field("Position", position_label, true)
        .field("Volume", format!("{} / 1000", view.volume), true)
        .field("Loop", truncate(&view.loop_mode, 32), true)
        .field("Filter", truncate(&view.active_filter, 32), true)
        .field(
            "Progress",
            progress_bar(view.position_ms, view.length_ms, 16),
            false,
        )
}

#[cfg(test)]
mod tests {
    use super::{build, safe_http_url, NowPlayingView};
    use crate::playback::seek::progress_bar;

    fn view() -> NowPlayingView {
        NowPlayingView {
            title: "A Track".to_owned(),
            url: Some("https://example.com/track".to_owned()),
            author: "An Artist".to_owned(),
            requester: "listener".to_owned(),
            position_ms: 1000,
            length_ms: Some(4000),
            volume: 100,
            loop_mode: "Off".to_owned(),
            active_filter: "None".to_owned(),
            paused: false,
            connected: true,
            generation: 0,
        }
    }

    #[test]
    fn view_defaults_are_consistent() {
        let view = view();
        assert_eq!(view.loop_mode, "Off");
        assert_eq!(view.active_filter, "None");
        assert_eq!(progress_bar(view.position_ms, view.length_ms, 4), "━🔘───");
    }

    #[test]
    fn rejects_non_http_or_oversized_urls() {
        assert!(safe_http_url(Some("javascript:alert(1)")).is_none());
        assert!(safe_http_url(Some(&"x".repeat(513))).is_none());
        assert_eq!(safe_http_url(Some("https://ok")), Some("https://ok"));
    }

    #[test]
    fn builds_embed_for_live_tracks() {
        let mut view = view();
        view.length_ms = None;
        view.url = None;
        let embed = build(&view);
        let value = serde_json::to_value(&embed).expect("embed is serializable");
        assert!(value.get("title").is_some());
        assert_eq!(value.get("color"), Some(&serde_json::json!(0x0034_98DB)));
    }
}
