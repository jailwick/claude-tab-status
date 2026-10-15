//! Plugin configuration, read from the KDL block the plugin is loaded with.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::status::IDLE_GLYPH;

/// Glyph shown for a working tab when `steady` is on.
pub const DEFAULT_WORKING_GLYPH: &str = "◐";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Show `working_glyph` for the whole time Claude works instead of mirroring the `◐`/`◑`
    /// spinner of the pane's title, so the tab is renamed only when the state changes.
    pub steady: bool,
    pub working_glyph: String,
    pub idle_glyph: String,
    /// Fallback for panes whose title changes without anyone telling the plugin: re-read pane
    /// titles this often. `None` (default) is off.
    pub refresh_interval: Option<Duration>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            steady: false,
            working_glyph: DEFAULT_WORKING_GLYPH.to_owned(),
            idle_glyph: IDLE_GLYPH.to_owned(),
            refresh_interval: None,
        }
    }
}

impl Config {
    /// Builds the config from the plugin's key/value options. Missing or invalid values fall back
    /// to the defaults.
    pub fn from_options(options: &BTreeMap<String, String>) -> Self {
        let defaults = Self::default();
        Self {
            steady: options
                .get("steady")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(defaults.steady),
            working_glyph: glyph_option(options, "working_glyph").unwrap_or(defaults.working_glyph),
            idle_glyph: glyph_option(options, "idle_glyph").unwrap_or(defaults.idle_glyph),
            refresh_interval: options
                .get("refresh_interval")
                .and_then(|v| v.trim().parse::<f64>().ok())
                .and_then(|secs| Duration::try_from_secs_f64(secs).ok())
                .filter(|interval| !interval.is_zero()),
        }
    }
}

fn glyph_option(options: &BTreeMap<String, String>, key: &str) -> Option<String> {
    options
        .get(key)
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn defaults_when_empty() {
        assert_eq!(Config::from_options(&options(&[])), Config::default());
    }

    #[test]
    fn reads_all_options() {
        let config = Config::from_options(&options(&[
            ("steady", "true"),
            ("working_glyph", "●"),
            ("idle_glyph", " ○ "),
            ("refresh_interval", "1.5"),
        ]));
        assert!(config.steady);
        assert_eq!(config.working_glyph, "●");
        assert_eq!(config.idle_glyph, "○");
        assert_eq!(config.refresh_interval, Some(Duration::from_millis(1500)));
    }

    #[test]
    fn invalid_values_fall_back_to_defaults() {
        let config = Config::from_options(&options(&[("steady", "yes"), ("working_glyph", "  ")]));
        assert_eq!(config, Config::default());
    }

    #[test]
    fn refresh_interval_off_unless_positive() {
        for value in ["0", "-1", "soon", "NaN", "inf"] {
            let config = Config::from_options(&options(&[("refresh_interval", value)]));
            assert_eq!(config.refresh_interval, None, "{value}");
        }
    }
}
