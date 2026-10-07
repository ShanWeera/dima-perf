//! Build script of the desktop app.
//!
//! * Records the target triple for the run report (`tool.target`, D103), as the CLI does.
//! * When building for Windows, embeds the application icon and the version information in the
//!   executable (D80, D82). This needs the Windows SDK resource compiler (`rc.exe`, or `windres`
//!   with the GNU toolchain) and never the network. A missing resource compiler only produces a
//!   warning: the program still builds, without an embedded icon.

fn main() {
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned());
    println!("cargo:rustc-env=DIMA_GUI_TARGET={target}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/icon/dima.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_resources();
    }
}

fn embed_windows_resources() {
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icon/dima.ico")
        .set("ProductName", "DiMA")
        .set(
            "FileDescription",
            "DiMA: sequence diversity dynamics analyser for viruses",
        )
        .set("ProductVersion", &version)
        .set("FileVersion", &version)
        .set("OriginalFilename", "dima-gui.exe")
        .set(
            "LegalCopyright",
            "Copyright (c) Shan Tharanga. MIT License.",
        );
    if let Err(e) = res.compile() {
        println!("cargo:warning=the Windows icon and version information were not embedded: {e}");
    }
}
