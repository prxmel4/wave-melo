pub mod ping;
pub mod play;
pub mod playback;
pub mod voice;

pub use ping::ping;

pub fn commands() -> Vec<poise::Command<crate::state::BotData, crate::error::BotError>> {
    vec![
        ping(),
        play::play(),
        playback::skip(),
        playback::stop(),
        voice::join(),
        voice::leave(),
    ]
}
