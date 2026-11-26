//! Bundled typography (IBM Plex).
//!
//! Bundled fonts render identically on macOS, Windows and Linux, which matters in a dense data
//! interface where column alignment and small sizes must be legible. IBM Plex Sans distinguishes
//! easily confused glyphs (1/l/I, 0/O); IBM Plex Mono aligns k-mer residues column by column.
//!
//! Licensed under the SIL Open Font License 1.1 (`assets/fonts/OFL.txt`).

use std::sync::Arc;

/// Interface text face; chart exports rasterise their labels with it too (`charts::scene`), so
/// exported figures match the screen.
pub const PLEX_SANS: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans-Regular.ttf");
/// Heavier face for headings and emphasis; the titles of exported PNG figures are drawn in it,
/// as their SVG twins ask for (`font-weight="600"`).
pub const PLEX_SANS_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans-SemiBold.ttf");
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
/// app draws (sort arrows, check marks, the warning sign, the macOS command
/// key, the triangles ▸ and ▼). Keeping the defaults as a fallback means an
/// uncovered code point resolves to egui's font instead of rendering as a
/// missing-glyph box. The triangles are only in egui's monospaced Hack, so Hack
/// ends the proportional chains too; being last, it draws only what no other
/// face has. A symbol that no face has (✕, U+2715) is not used.
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
    // pre-existing default entries remain as the fallback tail, Hack last.
    let proportional = fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default();
    proportional.insert(0, "PlexSans".to_owned());
    if !proportional.iter().any(|f| f == "Hack") {
        proportional.push("Hack".to_owned());
    }

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

        for (family, added) in [
            (egui::FontFamily::Proportional, 2),
            (egui::FontFamily::Monospace, 1),
        ] {
            let before = defaults.families[&family].len();
            let after = defs.families[&family].len();
            assert_eq!(
                after,
                before + added,
                "family {family:?} should gain {added} faces and keep every fallback"
            );
            for fallback in &defaults.families[&family] {
                assert!(
                    defs.families[&family].contains(fallback),
                    "fallback {fallback} was dropped from {family:?}"
                );
            }
        }
        assert_eq!(
            defs.families[&egui::FontFamily::Proportional]
                .last()
                .map(String::as_str),
            Some("Hack"),
            "Hack ends the proportional chain, so it draws only what no other face has"
        );
    }

    /// Every symbol the interface writes is drawn by a face of the family that writes it, so
    /// none shows as an empty box: the close glyph, the sort arrows, the class check mark, the
    /// warning sign, the theme switch, the command key, the menu-path and coverage triangles,
    /// the drag-hint bullet and the minus of the zoom-out button. The faces are read directly
    /// (egui's own check reports no glyph for those of the face that draws its replacement
    /// character, among them the check mark).
    #[test]
    fn every_symbol_of_the_interface_has_a_glyph() {
        use ab_glyph::Font as _;
        let defs = definitions();
        let has = |family: &egui::FontFamily, c: char| {
            defs.families[family].iter().any(|name| {
                let data = &defs.font_data[name];
                ab_glyph::FontRef::try_from_slice_and_index(&data.font, data.index)
                    .is_ok_and(|face| face.glyph_id(c).0 != 0)
            })
        };
        let symbols = [
            crate::panels::common::CLOSE,
            crate::state::sort::Direction::Ascending.arrow(),
            crate::state::sort::Direction::Descending.arrow(),
            "\u{2714}",
            "\u{26A0}",
            "\u{2600}",
            "\u{2318}",
            "\u{25B8}",
            "\u{25BC}",
            "\u{2022}",
            "\u{2212}",
        ];
        for family in [
            egui::FontFamily::Proportional,
            egui::FontFamily::Name(SEMIBOLD.into()),
        ] {
            for c in symbols.iter().flat_map(|s| s.trim().chars()) {
                assert!(has(&family, c), "{c:?} has no glyph in {family:?}");
            }
        }
        // The glyph that the close buttons used to show is in no face.
        assert!(!has(&egui::FontFamily::Proportional, '\u{2715}'));
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
