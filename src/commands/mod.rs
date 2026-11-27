pub mod ping;

pub use ping::ping;

pub fn commands() -> Vec<poise::Command<crate::state::BotData, crate::error::BotError>> {
    vec![ping()]
}
