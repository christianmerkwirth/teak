//! Sets `cfg(magika)` when the `magika` feature is on and the target has a
//! prebuilt ONNX Runtime. pyke ships none for Intel macOS
//! (`x86_64-apple-darwin`, docs/phase0-report.md Plan B), so there teak
//! builds without Magika, as with `--no-default-features`: `--version`
//! prints `magika: off`, and `--detect full` acts as `fast` (FR-55).
//! `cargo install teak` then works on every target with no extra flag.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(magika)");
    println!("cargo::rerun-if-changed=build.rs");
    let feature = std::env::var_os("CARGO_FEATURE_MAGIKA").is_some();
    let target = std::env::var("TARGET").unwrap_or_default();
    if feature && target != "x86_64-apple-darwin" {
        println!("cargo::rustc-cfg=magika");
    }
}
