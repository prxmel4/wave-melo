use crate::error::BotError;
use crate::state::BotData;

/// Build Poise framework options for the bot.
pub fn framework_options() -> poise::FrameworkOptions<BotData, BotError> {
    poise::FrameworkOptions {
        commands: Vec::new(),
        ..Default::default()
    }
}
