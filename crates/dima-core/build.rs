//! The documentation address of this build (D113): a release build (`DIMA_RELEASE=1`, set by the
//! release workflow) links the documentation of its own version, `<homepage>vX.Y/`; any other
//! build links `<homepage>latest/`, the documentation of the development version.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DIMA_RELEASE");
    let homepage = std::env::var("CARGO_PKG_HOMEPAGE").unwrap_or_default();
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let path = if std::env::var("DIMA_RELEASE").is_ok_and(|v| v == "1") {
        let mut parts = version.split('.');
        format!(
            "v{}.{}",
            parts.next().unwrap_or("0"),
            parts.next().unwrap_or("0")
        )
    } else {
        "latest".to_owned()
    };
    println!("cargo:rustc-env=DIMA_DOCS_URL={homepage}{path}/");
}
