//! teak: fit a large directory tree into a fixed line budget.
//!
//! Phase 0 holds only the `io` gate. Task-2 adds the pipeline and `run()`.

pub mod io;

/// The `teak --version` line, without the newline.
pub fn version_line() -> String {
    let version = env!("CARGO_PKG_VERSION");
    #[cfg(feature = "magika")]
    return format!("teak {version} (magika: on, model {})", magika::MODEL_NAME);
    #[cfg(not(feature = "magika"))]
    return format!("teak {version} (magika: off)");
}
