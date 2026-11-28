//! Pure audio-filter presets built from Lavalink's native filter model.
//!
//! Each preset returns a complete `Filters` value because Lavalink replaces
//! the entire filter set on every update.

use lavalink_rs::model::player::{Equalizer, Filters, Karaoke, Rotation, Timescale};

/// All preset names accepted by `/filter`.
pub const PRESETS: [&str; 6] = [
    "bassboost",
    "nightcore",
    "vaporwave",
    "8d",
    "karaoke",
    "clear",
];

fn equalizer(bands: &[(u8, f64)]) -> Option<Vec<Equalizer>> {
    Some(
        bands
            .iter()
            .map(|(band, gain)| Equalizer {
                band: *band,
                gain: *gain,
            })
            .collect(),
    )
}

fn timescale(speed: f64, pitch: f64) -> Option<Timescale> {
    Some(Timescale {
        speed: Some(speed),
        pitch: Some(pitch),
        rate: Some(1.0),
    })
}

/// Emphasize low-frequency bands.
pub fn bassboost() -> Filters {
    Filters {
        equalizer: equalizer(&[(0, 0.25), (1, 0.20), (2, 0.12), (3, 0.05)]),
        ..Filters::default()
    }
}

/// Faster tempo with a higher pitch.
pub fn nightcore() -> Filters {
    Filters {
        timescale: timescale(1.25, 1.25),
        ..Filters::default()
    }
}

/// Slowed, pitched-down playback with gentle stereo rotation.
pub fn vaporwave() -> Filters {
    Filters {
        timescale: timescale(0.82, 0.82),
        rotation: Some(Rotation {
            rotation_hz: Some(0.1),
        }),
        ..Filters::default()
    }
}

/// Stereo panning effect.
pub fn eight_d() -> Filters {
    Filters {
        rotation: Some(Rotation {
            rotation_hz: Some(0.2),
        }),
        ..Filters::default()
    }
}

/// Vocal reduction with the standard karaoke band.
pub fn karaoke() -> Filters {
    Filters {
        karaoke: Some(Karaoke {
            level: Some(1.0),
            mono_level: Some(1.0),
            filter_band: Some(220.0),
            filter_width: Some(100.0),
        }),
        ..Filters::default()
    }
}

/// Remove every active filter.
pub fn clear() -> Filters {
    Filters::default()
}

/// Resolve a preset name case-insensitively.
pub fn resolve(name: &str) -> Option<(&'static str, Filters)> {
    match name.trim().to_ascii_lowercase().as_str() {
        "bassboost" | "bass" => Some(("bassboost", bassboost())),
        "nightcore" => Some(("nightcore", nightcore())),
        "vaporwave" => Some(("vaporwave", vaporwave())),
        "8d" => Some(("8d", eight_d())),
        "karaoke" => Some(("karaoke", karaoke())),
        "clear" | "off" => Some(("clear", clear())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{clear, resolve, PRESETS};

    #[test]
    fn every_documented_preset_resolves() {
        for name in PRESETS {
            assert!(resolve(name).is_some(), "missing preset {name}");
        }
        assert!(resolve("unknown").is_none());
    }

    #[test]
    fn presets_are_case_insensitive() {
        assert_eq!(
            resolve(" NIGHTCORE ").map(|(name, _)| name),
            Some("nightcore")
        );
        assert_eq!(resolve("Off").map(|(name, _)| name), Some("clear"));
    }

    #[test]
    fn clear_has_no_active_filters() {
        let filters = clear();
        assert!(filters.equalizer.is_none());
        assert!(filters.timescale.is_none());
        assert!(filters.rotation.is_none());
        assert!(filters.karaoke.is_none());
    }

    #[test]
    fn bassboost_uses_valid_bands_and_gains() {
        let (_, filters) = resolve("bassboost").unwrap();
        let bands = filters.equalizer.unwrap();
        assert_eq!(bands.len(), 4);
        for band in bands {
            assert!(band.band <= 14);
            assert!((-0.25..=1.0).contains(&band.gain));
        }
    }
}
