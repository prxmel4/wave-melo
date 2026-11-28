pub mod autoplay;
pub mod filter;
pub mod history;
pub mod mode247;
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
        playback::seek(),
        playback::now_playing(),
        playback::loop_mode(),
        queue::queue(),
        queue::queueremove(),
        queue::queuejump(),
        queue::queuemove(),
        queue::queueclear(),
        queue::shuffleq(),
        history::history(),
        filter::filter(),
        autoplay::autoplay(),
        mode247::mode_247(),
        voice::join(),
        voice::leave(),
    ]
}
