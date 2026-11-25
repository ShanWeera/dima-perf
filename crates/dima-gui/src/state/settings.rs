//! Settings remembered between sessions (D73): the last-used analysis options that do not
//! depend on the file, the conserved-region parameters, the worker threads, the file-dialog
//! choice and the export settings.
//!
//! Stored as `{"version": N, "settings": {…}}`. A file written by another version, or one that
//! cannot be read, is replaced by the defaults and the user is told once (safe reset); nothing
//! half-read is ever used. The one exception is a figure size outside its range (such as one
//! saved before the minimum was raised): it is moved into the range, the user is told, and the
//! corrected settings are saved at once, so the notice appears once. The alphabet, k and query
//! name are not remembered: they belong to one file and reset with every new file (G12, G13).

use serde::{Deserialize, Serialize};

use dima_core::config::{DEFAULT_FILL, DEFAULT_SUPPORT_THRESHOLD, Estimator, ValidationMode};

/// Version of the stored settings; bump on any incompatible change.
pub const SETTINGS_VERSION: u32 = 1;

/// Widths of exported figures, in pixels: at least 720, so the title, a legend of up to three
/// rows and the axes fit around a plot at least 100 px tall.
pub const FIGURE_WIDTH: std::ops::RangeInclusive<u32> = 720..=20_000;
/// Heights of exported figures, in pixels: at least 320 (see [`FIGURE_WIDTH`]).
pub const FIGURE_HEIGHT: std::ops::RangeInclusive<u32> = 320..=20_000;

/// Which file dialog to use (D100).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogMode {
    /// The operating system's dialog, falling back to the built-in browser.
    #[default]
    Native,
    /// Always the built-in browser.
    BuiltIn,
}

/// Everything remembered.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Support threshold T.
    pub support_threshold: u64,
    /// Entropy estimator.
    pub estimator: Estimator,
    /// Validation mode.
    pub validation: ValidationMode,
    /// Minimum sample coverage to flag, if any.
    pub min_coverage: Option<f64>,
    /// Fill value for empty header fields.
    pub fill: String,
    /// HCS threshold as typed (validated on use).
    pub hcs_threshold: String,
    /// Build conserved regions from eligible positions only.
    pub hcs_eligible_only: bool,
    /// Worker threads; `None` = all CPUs but one (D101).
    pub threads: Option<usize>,
    /// File dialogs.
    pub dialogs: DialogMode,
    /// Width and height of exported figures, in pixels.
    pub figure_size: (u32, u32),
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            support_threshold: DEFAULT_SUPPORT_THRESHOLD,
            estimator: Estimator::default(),
            validation: ValidationMode::default(),
            min_coverage: None,
            fill: DEFAULT_FILL.to_owned(),
            hcs_threshold: dima_core::config::DEFAULT_HCS_THRESHOLD.to_string(),
            hcs_eligible_only: false,
            threads: None,
            dialogs: DialogMode::default(),
            figure_size: (1800, 640),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    settings: serde_json::Value,
}

/// What reading stored settings found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loaded {
    /// The settings as stored.
    Ok,
    /// The stored figure size was outside its range and was moved into it; the notice says so.
    /// The corrected settings should be saved, so the user is told once.
    Adjusted(String),
    /// The stored settings were replaced by the defaults; the notice says why.
    Reset(String),
}

impl Settings {
    /// The settings in their stored form.
    #[must_use]
    pub fn to_json(&self) -> String {
        let stored = Stored {
            version: SETTINGS_VERSION,
            settings: serde_json::to_value(self).unwrap_or(serde_json::Value::Null),
        };
        serde_json::to_string_pretty(&stored).unwrap_or_default()
    }

    /// Reads stored settings. Returns the defaults with a notice when the text was written by
    /// another version, cannot be read or holds an invalid value; a figure size outside its
    /// range is moved into it instead, with a notice.
    #[must_use]
    pub fn from_json(text: &str) -> (Self, Loaded) {
        let reset = |why: &str| {
            (
                Self::default(),
                Loaded::Reset(format!("Settings were reset to the defaults ({why}).")),
            )
        };
        let Ok(stored) = serde_json::from_str::<Stored>(text) else {
            return reset("the settings file could not be read");
        };
        if stored.version != SETTINGS_VERSION {
            return reset("they were saved by a different version of DiMA");
        }
        let mut s = match serde_json::from_value::<Self>(stored.settings) {
            Ok(s) if s.is_valid() => s,
            _ => return reset("the saved values were not valid"),
        };
        let saved = s.figure_size;
        s.figure_size = (
            saved.0.clamp(*FIGURE_WIDTH.start(), *FIGURE_WIDTH.end()),
            saved.1.clamp(*FIGURE_HEIGHT.start(), *FIGURE_HEIGHT.end()),
        );
        if s.figure_size == saved {
            return (s, Loaded::Ok);
        }
        let notice = figure_size_notice(saved, s.figure_size);
        (s, Loaded::Adjusted(notice))
    }

    /// Range checks of every value but the figure size, which is moved into its range instead
    /// (a hand-edited file could hold anything).
    fn is_valid(&self) -> bool {
        self.support_threshold >= 1
            && self.min_coverage.is_none_or(|c| (0.0..=1.0).contains(&c))
            && !self.fill.trim().is_empty()
            && self.threads.is_none_or(|t| t >= 1)
    }
}

/// The notice of a stored figure size moved from `saved` into its range, `now`.
fn figure_size_notice(saved: (u32, u32), now: (u32, u32)) -> String {
    let (min_w, min_h) = (*FIGURE_WIDTH.start(), *FIGURE_HEIGHT.start());
    let (w, h) = now;
    if now.0 >= saved.0 && now.1 >= saved.1 {
        format!(
            "Exported figures are now at least {min_w} \u{d7} {min_h} px so the title, legend \
             and axes fit; yours was raised to {w} \u{d7} {h} px"
        )
    } else {
        // Only a hand-edited file holds a size above the maximum.
        format!(
            "Exported figures are between {min_w} \u{d7} {min_h} and {} \u{d7} {} px; yours \
             was changed to {w} \u{d7} {h} px",
            FIGURE_WIDTH.end(),
            FIGURE_HEIGHT.end()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let s = Settings {
            support_threshold: 30,
            estimator: Estimator::ChaoWangJost,
            min_coverage: Some(0.9),
            hcs_threshold: "95.5".into(),
            threads: Some(3),
            dialogs: DialogMode::BuiltIn,
            ..Settings::default()
        };
        let (back, loaded) = Settings::from_json(&s.to_json());
        assert_eq!(back, s);
        assert_eq!(loaded, Loaded::Ok);
    }

    #[test]
    fn defaults_follow_the_library() {
        let s = Settings::default();
        assert_eq!(s.support_threshold, 100, "D6: T = 100 everywhere");
        assert_eq!(s.hcs_threshold, "100", "D44: HCS default 100");
        assert_eq!(s.fill, "Unknown");
        assert_eq!(s.estimator, Estimator::Dima);
        assert_eq!(s.validation, ValidationMode::Strict);
    }

    #[test]
    fn safe_reset_on_other_versions_and_bad_input() {
        for text in [
            "",
            "not json",
            r#"{"version": 99, "settings": {}}"#,
            r#"{"version": 1, "settings": {"support_threshold": 0}}"#,
            r#"{"version": 1, "settings": {"min_coverage": 2.0}}"#,
            r#"{"version": 1, "settings": {"unknown_key": 1}}"#,
            r#"{"version": 1, "settings": {"fill": "  "}}"#,
            r#"{"version": 1, "settings": {"figure_size": [-1, 500]}}"#,
            // An invalid value resets everything, even next to a size that would be clamped.
            r#"{"version": 1, "settings": {"support_threshold": 0, "figure_size": [100, 100]}}"#,
        ] {
            let (s, loaded) = Settings::from_json(text);
            assert_eq!(s, Settings::default(), "{text}");
            assert!(matches!(loaded, Loaded::Reset(_)), "{text}");
        }
        // Missing keys take their defaults (forward-compatible within a version).
        let (s, loaded) =
            Settings::from_json(r#"{"version": 1, "settings": {"support_threshold": 30}}"#);
        assert_eq!(s.support_threshold, 30);
        assert_eq!(s.fill, "Unknown");
        assert_eq!(loaded, Loaded::Ok);
    }

    #[test]
    fn figure_sizes_are_moved_into_their_range() {
        let read = |size: &str| {
            Settings::from_json(&format!(
                r#"{{"version": 1, "settings": {{"support_threshold": 30, "figure_size": {size}}}}}"#
            ))
        };
        let (s, loaded) = read("[1800, 150]");
        assert_eq!(s.figure_size, (1800, 320));
        assert_eq!(s.support_threshold, 30, "the other settings are kept");
        assert_eq!(
            loaded,
            Loaded::Adjusted(
                "Exported figures are now at least 720 \u{d7} 320 px so the title, legend and \
                 axes fit; yours was raised to 1800 \u{d7} 320 px"
                    .to_owned()
            )
        );
        let (s, loaded) = read("[100, 100]");
        assert_eq!(s.figure_size, (720, 320));
        assert!(
            matches!(loaded, Loaded::Adjusted(n) if n.ends_with("raised to 720 \u{d7} 320 px"))
        );
        let (s, loaded) = read("[30000, 30000]");
        assert_eq!(s.figure_size, (20_000, 20_000));
        assert!(
            matches!(&loaded, Loaded::Adjusted(n) if n.ends_with("changed to 20000 \u{d7} 20000 px")),
            "{loaded:?}"
        );
        let (s, loaded) = read("[720, 320]");
        assert_eq!((s.figure_size, loaded), ((720, 320), Loaded::Ok));
        // The defaults are inside the range.
        let d = Settings::default().figure_size;
        assert!(FIGURE_WIDTH.contains(&d.0) && FIGURE_HEIGHT.contains(&d.1));
    }
}
