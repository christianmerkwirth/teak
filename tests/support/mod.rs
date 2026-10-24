//! Shared helpers for the integration tests.
//!
//! Each test binary pulls this in with `mod support;`. Not every binary uses
//! every helper, so dead-code warnings are off here.
#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, UNIX_EPOCH};

/// The seeded fixture generator from `examples/gen_tree.rs`. Tests call it in
/// process, so they need no nested `cargo run`.
#[path = "../../examples/gen_tree.rs"]
pub mod gen_tree;

/// The scan time that every test run gets through `SOURCE_DATE_EPOCH`.
pub const SCAN_TIME: u64 = 1_750_000_000;

/// A tree in a temp directory. The directory goes away when the value drops.
pub struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    pub fn new() -> Fixture {
        Fixture {
            dir: tempfile::tempdir().expect("create temp dir"),
        }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    fn at(&self, path: &str) -> PathBuf {
        self.dir.path().join(path)
    }

    /// Writes a file and creates its parent directories.
    pub fn file(&self, path: &str, bytes: impl AsRef<[u8]>) -> &Self {
        let p = self.at(path);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(&p, bytes).expect("write file");
        self
    }

    pub fn dir(&self, path: &str) -> &Self {
        fs::create_dir_all(self.at(path)).expect("create dir");
        self
    }

    pub fn symlink(&self, path: &str, target: &str) -> &Self {
        let p = self.at(path);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        symlink(target, &p).expect("create symlink");
        self
    }

    pub fn mode(&self, path: &str, mode: u32) -> &Self {
        fs::set_permissions(self.at(path), fs::Permissions::from_mode(mode)).expect("set mode");
        self
    }

    /// Sets the mtime to `secs` seconds before `SCAN_TIME`. Works on files and
    /// directories; it follows symlinks.
    pub fn mtime(&self, path: &str, secs_before_scan: u64) -> &Self {
        let t = UNIX_EPOCH + Duration::from_secs(SCAN_TIME - secs_before_scan);
        fs::File::open(self.at(path))
            .and_then(|f| f.set_modified(t))
            .expect("set mtime");
        self
    }
}

/// Runs the teak binary with a fixed scan time.
///
/// Task-2 adds the default flags `--format plain --width 120 --color never`.
pub fn teak(fixture: &Fixture, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_teak"))
        .args(args)
        .current_dir(fixture.path())
        .env("SOURCE_DATE_EPOCH", SCAN_TIME.to_string())
        .env_remove("TEAK_TEST_REPORT")
        .env_remove("TEAK_TEST_READ_DELAY_MS")
        .output()
        .expect("run teak")
}

/// The parsed `TEAK_TEST_REPORT` file.
#[cfg(feature = "test-hooks")]
#[derive(Debug, Default)]
pub struct Report {
    /// Content bytes read, per path.
    pub reads: std::collections::BTreeMap<String, u64>,
    /// Elapsed time per stage, in ms.
    pub stages_ms: std::collections::BTreeMap<String, f64>,
}

#[cfg(feature = "test-hooks")]
pub fn read_report(path: &Path) -> Report {
    let text = fs::read_to_string(path).expect("read test report");
    let json: serde_json::Value = serde_json::from_str(&text).expect("parse test report");
    let mut report = Report::default();
    for (k, v) in json["reads"].as_object().expect("reads object") {
        report
            .reads
            .insert(k.clone(), v.as_u64().expect("read count"));
    }
    for (k, v) in json["stages_ms"].as_object().expect("stages_ms object") {
        report
            .stages_ms
            .insert(k.clone(), v.as_f64().expect("stage ms"));
    }
    report
}
