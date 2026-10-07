//! Records the build target for the run report and `--version` details.
fn main() {
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned());
    println!("cargo:rustc-env=DIMA_TARGET={target}");
    println!("cargo:rerun-if-changed=build.rs");
}
