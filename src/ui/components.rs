//! Strict custom-ID grammar and buttons for Now Playing controls.

use serenity::all::{ButtonStyle, CreateActionRow, CreateButton};

use crate::error::BotError;

/// Operations exposed by Now Playing buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlOp {
    PauseResume,
    Skip,
    Stop,
    VolumeUp,
    VolumeDown,
}

impl ControlOp {
    fn code(self) -> &'static str {
        match self {
            Self::PauseResume => "pp",
            Self::Skip => "skip",
            Self::Stop => "stop",
            Self::VolumeUp => "vup",
            Self::VolumeDown => "vdn",
        }
    }

    fn from_code(code: &str) -> Option<Self> {
        match code {
            "pp" => Some(Self::PauseResume),
            "skip" => Some(Self::Skip),
            "stop" => Some(Self::Stop),
            "vup" => Some(Self::VolumeUp),
            "vdn" => Some(Self::VolumeDown),
            _ => None,
        }
    }
}

/// Build a custom ID under Discord's 100-character limit.
pub fn control_custom_id(guild_id: u64, generation: u64, op: ControlOp) -> String {
    format!("wavemelo:ctrl:{guild_id}:{generation}:{}", op.code())
}

fn parse_canonical_u64(value: &str) -> Result<u64, BotError> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(BotError::InvalidComponent(
            "invalid numeric field".to_owned(),
        ));
    }
    value
        .parse::<u64>()
        .map_err(|_| BotError::InvalidComponent("invalid numeric field".to_owned()))
}

/// Parse a control custom ID strictly.
pub fn parse_control_custom_id(raw: &str) -> Result<(u64, u64, ControlOp), BotError> {
    if raw.len() > 100 {
        return Err(BotError::InvalidComponent("custom ID too long".to_owned()));
    }
    let parts = raw.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        ["wavemelo", "ctrl", guild_id, generation, op] => {
            let guild_id = parse_canonical_u64(guild_id)?;
            let generation = parse_canonical_u64(generation)?;
            let op = ControlOp::from_code(op)
                .ok_or_else(|| BotError::InvalidComponent("unknown control".to_owned()))?;
            Ok((guild_id, generation, op))
        }
        _ => Err(BotError::InvalidComponent("malformed custom ID".to_owned())),
    }
}

/// Build the control action row for a Now Playing embed.
pub fn control_rows(guild_id: u64, generation: u64, paused: bool) -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![
        CreateButton::new(control_custom_id(
            guild_id,
            generation,
            ControlOp::PauseResume,
        ))
        .label(if paused { "Resume" } else { "Pause" })
        .style(ButtonStyle::Secondary),
        CreateButton::new(control_custom_id(guild_id, generation, ControlOp::Skip))
            .label("Skip")
            .style(ButtonStyle::Primary),
        CreateButton::new(control_custom_id(guild_id, generation, ControlOp::Stop))
            .label("Stop")
            .style(ButtonStyle::Danger),
        CreateButton::new(control_custom_id(guild_id, generation, ControlOp::VolumeUp))
            .label("Vol +")
            .style(ButtonStyle::Secondary),
        CreateButton::new(control_custom_id(
            guild_id,
            generation,
            ControlOp::VolumeDown,
        ))
        .label("Vol -")
        .style(ButtonStyle::Secondary),
    ])]
}

#[cfg(test)]
mod tests {
    use super::{control_custom_id, parse_control_custom_id, ControlOp};
    use crate::error::BotError;

    #[test]
    fn custom_ids_round_trip() {
        for op in [
            ControlOp::PauseResume,
            ControlOp::Skip,
            ControlOp::Stop,
            ControlOp::VolumeUp,
            ControlOp::VolumeDown,
        ] {
            let id = control_custom_id(123, 7, op);
            assert!(id.len() <= 100);
            assert_eq!(parse_control_custom_id(&id).unwrap(), (123, 7, op));
        }
    }

    #[test]
    fn rejects_malformed_and_unsafe_ids() {
        let valid = control_custom_id(1, 2, ControlOp::Skip);
        assert!(matches!(
            parse_control_custom_id(&valid.replace("wavemelo", "wavexelo")),
            Err(BotError::InvalidComponent(_))
        ));
        assert!(matches!(
            parse_control_custom_id("wavemelo:ctrl:01:2:skip"),
            Err(BotError::InvalidComponent(_))
        ));
        assert!(matches!(
            parse_control_custom_id("wavemelo:ctrl:1:-2:skip"),
            Err(BotError::InvalidComponent(_))
        ));
        assert!(matches!(
            parse_control_custom_id("wavemelo:ctrl:1:2:unknown"),
            Err(BotError::InvalidComponent(_))
        ));
        assert!(matches!(
            parse_control_custom_id("wavemelo:ctrl:1:2"),
            Err(BotError::InvalidComponent(_))
        ));
        let long = format!("wavemelo:ctrl:1:2:skip:{}", "x".repeat(100));
        assert!(matches!(
            parse_control_custom_id(&long),
            Err(BotError::InvalidComponent(_))
        ));
    }
}
