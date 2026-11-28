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

    #[test]
    fn nightcore_speeds_up_and_raises_pitch() {
        let (_, filters) = resolve("nightcore").unwrap();
        let timescale = filters.timescale.unwrap();
        assert_eq!(timescale.speed, Some(1.25));
        assert_eq!(timescale.pitch, Some(1.25));
        assert_eq!(timescale.rate, Some(1.0));
        assert!(filters.equalizer.is_none());
        assert!(filters.rotation.is_none());
    }

    #[test]
    fn vaporwave_slows_down_with_gentle_rotation() {
        let (_, filters) = resolve("vaporwave").unwrap();
        let timescale = filters.timescale.unwrap();
        assert_eq!(timescale.speed, Some(0.82));
        assert_eq!(timescale.pitch, Some(0.82));
        assert_eq!(timescale.rate, Some(1.0));
        assert_eq!(filters.rotation.unwrap().rotation_hz, Some(0.1));
        assert!(filters.karaoke.is_none());
    }

    #[test]
    fn eight_d_rotates_without_touching_timescale() {
        let (_, filters) = resolve("8d").unwrap();
        assert_eq!(filters.rotation.unwrap().rotation_hz, Some(0.2));
        assert!(filters.timescale.is_none());
        assert!(filters.equalizer.is_none());
    }

    #[test]
    fn karaoke_targets_the_standard_vocal_band() {
        let (_, filters) = resolve("karaoke").unwrap();
        let karaoke = filters.karaoke.unwrap();
        assert_eq!(karaoke.level, Some(1.0));
        assert_eq!(karaoke.mono_level, Some(1.0));
        assert_eq!(karaoke.filter_band, Some(220.0));
        assert_eq!(karaoke.filter_width, Some(100.0));
        assert!(filters.timescale.is_none());
    }

    #[test]
    fn every_preset_replaces_the_full_filter_set() {
        // Lavalink replaces the entire filter set on update, so each preset
        // must be a complete value and `clear` must serialize to `{}`.
        for name in PRESETS {
            let (_, filters) = resolve(name).unwrap();
            let value = serde_json::to_value(&filters).unwrap();
            assert!(value.is_object(), "preset {name} is not an object");
        }
        assert_eq!(
            serde_json::to_value(clear()).unwrap(),
            serde_json::json!({})
        );
    }
}
