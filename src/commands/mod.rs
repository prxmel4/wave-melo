pub mod ping;
pub mod voice;

pub use ping::ping;

pub fn commands() -> Vec<poise::Command<crate::state::BotData, crate::error::BotError>> {
    vec![ping(), voice::join()]
}
