//! The type cache (FR-54): no stale types, no reads on a hit, a corrupt or
//! foreign file is ignored, and `--no-cache` does no cache I/O.

mod support;

use std::path::Path;

use support::{Fixture, corpus, run_cached};

/// The type cell of a file row: name, size, type, age.
fn type_cell(out: &support::Run, name: &str) -> String {
    out.cells(name)[2].to_string()
}

fn types_bin(cache: &Path) -> std::path::PathBuf {
    cache.join("teak").join("types.bin")
}

/// Visible files, a license for the content checks, and a folded directory
/// for the samples.
fn cached_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file("x", corpus("script"))
        .file("report.pdf", corpus("report.pdf"))
        .file("LICENSE", support::mit_license(2026, "Jane Doe"))
        .file("main.rs", "fn main() {}\n");
    for i in 0..30 {
        fx.file(
            &format!("pics/{}.png", support::word(i)),
            corpus("logo.png"),
        );
    }
    fx
}

#[test]
fn fr54_a_changed_file_gets_a_new_type() {
    let fx = cached_fixture();
    let cache = Fixture::new();
    let out = run_cached(fx.path(), cache.path(), &["-L", "1"]);
    assert_eq!(type_cell(&out, "x"), "python", "{}", out.stdout);
    assert!(types_bin(cache.path()).is_file(), "run 1 writes the cache");
    fx.file("x", corpus("logo.png")).mtime("x", 3600);
    let out = run_cached(fx.path(), cache.path(), &["-L", "1"]);
    assert_eq!(type_cell(&out, "x"), "png", "{}", out.stdout);
}

#[cfg(feature = "test-hooks")]
#[test]
fn fr54_a_cache_hit_reads_no_content() {
    let fx = cached_fixture();
    let cache = Fixture::new();
    let env = [("XDG_CACHE_HOME", cache.path().to_str().unwrap())];
    let (first, report) = support::run_report_env(fx.path(), &["-L", "1"], &env);
    assert!(!report.reads.is_empty());
    let (second, report) = support::run_report_env(fx.path(), &["-L", "1"], &env);
    assert!(report.reads.is_empty(), "{:?}", report.reads);
    assert_eq!(first.stdout, second.stdout);
    assert!(second.stdout.contains("LICENSE (MIT)"), "{}", second.stdout);
}

#[test]
fn fr54_a_corrupt_cache_is_ignored_and_made_again() {
    let fx = cached_fixture();
    let cache = Fixture::new();
    cache.file(
        "teak/types.bin",
        b"TEAKC\x01\x00garbage that is not a cache",
    );
    let out = run_cached(fx.path(), cache.path(), &["-L", "1"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(out.stderr.is_empty(), "{}", out.stderr);
    assert_eq!(type_cell(&out, "x"), "python", "{}", out.stdout);
    let loaded = teak::cache::Cache::load(
        &cache.path().join("teak"),
        teak::detect::magika::Model::version(),
    );
    assert!(!loaded.is_empty(), "teak made a new cache");
}

#[test]
fn fr54_another_model_version_drops_the_entries() {
    let fx = cached_fixture();
    let cache = Fixture::new();
    run_cached(fx.path(), cache.path(), &["-L", "1"]);
    let dir = cache.path().join("teak");
    let ours = teak::cache::Cache::load(&dir, teak::detect::magika::Model::version());
    assert!(!ours.is_empty());
    let other = teak::cache::Cache::load(&dir, "some_other_model_v9");
    assert!(other.is_empty());
}

#[cfg(feature = "test-hooks")]
#[test]
fn fr54_another_model_version_means_new_reads() {
    let fx = cached_fixture();
    let cache = Fixture::new();
    run_cached(fx.path(), cache.path(), &["-L", "1"]);
    // Change the model name in the header: `TEAKC`, u16 version, u16 length.
    let path = types_bin(cache.path());
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[9] ^= 0x20;
    std::fs::write(&path, bytes).unwrap();
    let env = [("XDG_CACHE_HOME", cache.path().to_str().unwrap())];
    let (out, report) = support::run_report_env(fx.path(), &["-L", "1"], &env);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(!report.reads.is_empty());
}

#[cfg(feature = "test-hooks")]
#[test]
fn fr54_no_cache_does_no_cache_io() {
    let fx = cached_fixture();
    let cache = Fixture::new();
    run_cached(fx.path(), cache.path(), &["-L", "1"]);
    let before = std::fs::read(types_bin(cache.path())).unwrap();
    let env = [("XDG_CACHE_HOME", cache.path().to_str().unwrap())];
    let (out, report) = support::run_report_env(fx.path(), &["-L", "1", "--no-cache"], &env);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(!report.reads.is_empty(), "no cache hits");
    assert!(
        !report.stages_ms.contains_key("cache_load"),
        "{:?}",
        report.stages_ms
    );
    assert_eq!(std::fs::read(types_bin(cache.path())).unwrap(), before);
    // A cache dir that does not exist stays that way.
    let missing = cache.path().join("nope");
    let env = [("XDG_CACHE_HOME", missing.to_str().unwrap())];
    support::run_report_env(fx.path(), &["--no-cache"], &env);
    assert!(!missing.exists());
}

#[test]
fn fr54_the_cache_is_never_written_inside_the_scanned_tree() {
    let fx = cached_fixture();
    let out = run_cached(fx.path(), fx.path(), &["-L", "1"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(!fx.abs("teak").exists(), "a cache inside the tree");
}

#[test]
fn fr54_a_symlink_in_the_cache_path_still_counts_as_inside() {
    // As on macOS, where /var is a symlink to /private/var: the cache path
    // goes through a symlink, and the root is the real path.
    let fx = cached_fixture();
    let links = Fixture::new();
    let link = links.path().join("link");
    std::os::unix::fs::symlink(fx.path(), &link).unwrap();
    let out = run_cached(fx.path(), &link, &["-L", "1"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(!fx.abs("teak").exists(), "a cache inside the tree");
}

#[test]
fn fr54_detect_off_needs_no_cache() {
    let fx = cached_fixture();
    let cache = Fixture::new();
    run_cached(fx.path(), cache.path(), &["--detect", "off"]);
    assert!(!types_bin(cache.path()).exists());
}
