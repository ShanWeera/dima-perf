//! Platform integration: the window icon (D80). The Windows executable icon and version
//! information are embedded by `build.rs`; the macOS and Linux package icons come from the
//! packaging metadata in `Cargo.toml`.

/// The application icon (generated from `assets/icon/dima.svg`).
const ICON_PNG: &[u8] = include_bytes!("../../../../assets/icon/dima-256.png");

/// The window icon, or `None` if it cannot be decoded (the platform default is then used).
#[must_use]
pub fn icon() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(ICON_PNG).ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_icon_decodes() {
        let i = super::icon().expect("bundled icon decodes");
        assert_eq!((i.width, i.height), (256, 256));
        assert_eq!(i.rgba.len(), 256 * 256 * 4);
    }
}
