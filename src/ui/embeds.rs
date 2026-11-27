use serenity::builder::CreateEmbed;
use serenity::model::colour::Colour;

use super::colors;

fn base(title: impl Into<String>, description: impl Into<String>, color: u32) -> CreateEmbed {
    CreateEmbed::new()
        .title(title)
        .description(description)
        .color(Colour::from(color))
}

/// Blue informational embed.
pub fn info(title: impl Into<String>, description: impl Into<String>) -> CreateEmbed {
    base(title, description, colors::INFO)
}

/// Green success embed.
pub fn success(title: impl Into<String>, description: impl Into<String>) -> CreateEmbed {
    base(title, description, colors::SUCCESS)
}

/// Red error embed. Callers should use a safe user-facing message.
pub fn error(title: impl Into<String>, description: impl Into<String>) -> CreateEmbed {
    base(title, description, colors::ERROR)
}
