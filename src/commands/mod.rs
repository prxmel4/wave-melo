pub mod ping;
pub mod play;
pub mod playback;
pub mod queue;
pub mod voice;

pub use ping::ping;

pub fn commands() -> Vec<poise::Command<crate::state::BotData, crate::error::BotError>> {
    vec![
        ping(),
        play::play(),
        playback::skip(),
        playback::stop(),
        playback::volume(),
        playback::pause(),
        playback::resume(),
        queue::queue(),
        voice::join(),
        voice::leave(),
    ]
}
