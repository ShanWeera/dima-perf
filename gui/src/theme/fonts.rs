//! Bundled typography (IBM Plex).
//!
//! Shipping the fonts rather than relying on whatever egui defaults to gives
//! identical, predictable rendering on macOS, Windows and Linux — important for
//! a data-dense UI where column alignment and small-size legibility matter.
//!
//! IBM Plex Sans is designed for interfaces where data accuracy matters (clear
//! glyph differentiation, e.g. 1/l/I and 0/O), and IBM Plex Mono shares its
//! design DNA, so k-mer sequences sit naturally beside the rest of the UI.
//!
//! Licensed under the SIL Open Font License 1.1 — see `assets/fonts/OFL.txt`.

use std::sync::Arc;

/// UI text face.
///
/// Public because file exports rasterise their own labels with the same face
/// (see `charts::scene`), so on-screen and exported figures match. Embedding it
/// once here keeps a single copy in the binary.
pub const PLEX_SANS: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans-Regular.ttf");
/// Heavier face for headings and emphasis.
const PLEX_SANS_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans-SemiBold.ttf");
/// Sequence / numeric face.
const PLEX_MONO: &[u8] = include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf");

/// Name of the custom family carrying the semibold face.
///
/// Use via `egui::FontFamily::Name(fonts::SEMIBOLD.into())`.
pub const SEMIBOLD: &str = "PlexSansSemiBold";

/// Install the bundled fonts into `ctx`.
///
/// Call once during app construction, **before** any text is laid out.
///
/// egui's own default fonts are deliberately retained *after* ours in every
/// family's fallback chain. Plex covers Latin text but not the UI symbols the
/// app draws (sort arrows, check marks, the dismiss glyph, the macOS command
/// key). Keeping the defaults as a fallback means an uncovered code point
/// resolves to egui's font instead of rendering as a missing-glyph box.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}

/// Build the font set installed by [`install`].
///
/// Separated as a pure function so the family/fallback wiring can be unit
/// tested without constructing an egui context.
pub fn definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "PlexSans".to_owned(),
        Arc::new(egui::FontData::from_static(PLEX_SANS)),
    );
    fonts.font_data.insert(
        SEMIBOLD.to_owned(),
        Arc::new(egui::FontData::from_static(PLEX_SANS_SEMIBOLD)),
    );
    fonts.font_data.insert(
        "PlexMono".to_owned(),
        Arc::new(egui::FontData::from_static(PLEX_MONO)),
    );

    // Prepend our faces so they win for the code points they cover, while the
    // pre-existing default entries remain as the fallback tail.
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "PlexSans".to_owned());

    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "PlexMono".to_owned());

    // The semibold family reuses the proportional fallback chain so headings
    // degrade to a covered glyph rather than to nothing.
    let mut semibold_chain = vec![SEMIBOLD.to_owned()];
    semibold_chain.extend(
        fonts
            .families
            .get(&egui::FontFamily::Proportional)
            .cloned()
            .unwrap_or_default(),
    );
    fonts
        .families
        .insert(egui::FontFamily::Name(SEMIBOLD.into()), semibold_chain);

    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_faces_are_present_and_look_like_truetype() {
        // Guards against a truncated or missing asset being linked in: every
        // TrueType file starts with the 0x00010000 sfnt version tag.
        for (name, bytes) in [
            ("sans", PLEX_SANS),
            ("semibold", PLEX_SANS_SEMIBOLD),
            ("mono", PLEX_MONO),
        ] {
            assert!(bytes.len() > 50_000, "{name} font asset looks truncated");
            assert_eq!(
                &bytes[0..4],
                &[0x00, 0x01, 0x00, 0x00],
                "{name} font is not a TrueType sfnt"
            );
        }
    }

    #[test]
    fn every_face_is_registered() {
        let defs = definitions();
        for key in ["PlexSans", SEMIBOLD, "PlexMono"] {
            assert!(defs.font_data.contains_key(key), "{key} was not registered");
        }
    }

    #[test]
    fn bundled_faces_take_priority_in_their_families() {
        let defs = definitions();
        assert_eq!(
            defs.families[&egui::FontFamily::Proportional]
                .first()
                .map(String::as_str),
            Some("PlexSans"),
            "Plex Sans must win for proportional text"
        );
        assert_eq!(
            defs.families[&egui::FontFamily::Monospace]
                .first()
                .map(String::as_str),
            Some("PlexMono"),
            "Plex Mono must win for sequences"
        );
        assert_eq!(
            defs.families[&egui::FontFamily::Name(SEMIBOLD.into())]
                .first()
                .map(String::as_str),
            Some(SEMIBOLD),
        );
    }

    #[test]
    fn default_fonts_are_retained_as_fallback() {
        // The UI draws symbol glyphs (sort arrows, check marks, the command key)
        // that Plex does not cover. Losing egui's default fonts from the fallback
        // chain would render those as missing-glyph boxes, so assert the chains
        // keep more than just our own face.
        let defaults = egui::FontDefinitions::default();
        let defs = definitions();

        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let before = defaults.families[&family].len();
            let after = defs.families[&family].len();
            assert_eq!(
                after,
                before + 1,
                "family {family:?} should gain exactly one face and keep every fallback"
            );
            for fallback in &defaults.families[&family] {
                assert!(
                    defs.families[&family].contains(fallback),
                    "fallback {fallback} was dropped from {family:?}"
                );
            }
        }
    }

    #[test]
    fn semibold_family_falls_back_to_proportional_chain() {
        let defs = definitions();
        let semibold = &defs.families[&egui::FontFamily::Name(SEMIBOLD.into())];
        assert!(
            semibold.len() > 1,
            "semibold family must have a fallback tail"
        );
        assert!(semibold.contains(&"PlexSans".to_string()));
    }
}
