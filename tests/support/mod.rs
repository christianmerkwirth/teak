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

    /// Writes a file of `len` bytes. Above 64 KiB the file is sparse, so
    /// large sizes cost no disk space.
    pub fn sized(&self, path: &str, len: u64) -> &Self {
        let p = self.at(path);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        let f = fs::File::create(&p).expect("create file");
        if len <= 64 * 1024 {
            use std::io::Write;
            (&f).write_all(&vec![b'x'; len as usize])
                .expect("write file");
        } else {
            f.set_len(len).expect("set file length");
        }
        self
    }

    /// Sets the mtime of every entry below the root (not the root itself)
    /// to `secs` seconds before `SCAN_TIME`. Symlinks are left out.
    pub fn mtime_all(&self, secs_before_scan: u64) -> &Self {
        fn walk(dir: &Path, t: std::time::SystemTime) {
            for entry in fs::read_dir(dir).expect("read dir") {
                let path = entry.expect("dir entry").path();
                let meta = fs::symlink_metadata(&path).expect("lstat");
                if meta.file_type().is_symlink() {
                    continue;
                }
                if meta.is_dir() {
                    walk(&path, t);
                }
                if meta.is_dir() || meta.is_file() {
                    fs::File::open(&path)
                        .and_then(|f| f.set_modified(t))
                        .expect("set mtime");
                }
            }
        }
        walk(
            self.path(),
            UNIX_EPOCH + Duration::from_secs(SCAN_TIME - secs_before_scan),
        );
        self
    }

    /// The absolute path of `path` in the fixture.
    pub fn abs(&self, path: &str) -> PathBuf {
        self.at(path)
    }
}

/// A name with no digits (`aa`, `ab`, …), so a run of them never forms a
/// series (FR-44).
pub fn word(i: usize) -> String {
    let a = (b'a' + (i / 26 % 26) as u8) as char;
    let b = (b'a' + (i % 26) as u8) as char;
    if i < 676 {
        format!("{a}{b}")
    } else {
        format!("{}{a}{b}", (b'a' + (i / 676 % 26) as u8) as char)
    }
}

/// The flags of the shared test conditions. Later flags override them.
pub const DEFAULT_FLAGS: &[&str] = &["--format", "plain", "--width", "120", "--color", "never"];

/// A teak command with a fixed scan time and a clean environment, in `dir`.
pub fn command(dir: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_teak"));
    cmd.current_dir(dir)
        .env("SOURCE_DATE_EPOCH", SCAN_TIME.to_string())
        .env_remove("TEAK_TEST_REPORT")
        .env_remove("TEAK_TEST_READ_DELAY_MS")
        .env_remove("TEAK_TEST_NO_CONTENT_CHECKS")
        .env_remove("NO_COLOR")
        .env_remove("CLICOLOR_FORCE")
        .env_remove("LS_COLORS")
        .env_remove("COLUMNS")
        .env_remove("LINES");
    cmd
}

/// Runs the teak binary in the fixture, with only the given flags.
pub fn teak(fixture: &Fixture, args: &[&str]) -> Output {
    command(fixture.path())
        .args(args)
        .output()
        .expect("run teak")
}

/// The result of one teak run, as text.
#[derive(Debug)]
pub struct Run {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn from_output(out: Output) -> Run {
        Run {
            code: out.status.code(),
            stdout: String::from_utf8(out.stdout).expect("stdout is UTF-8"),
            stderr: String::from_utf8(out.stderr).expect("stderr is UTF-8"),
        }
    }

    pub fn lines(&self) -> Vec<&str> {
        self.stdout.lines().collect()
    }

    pub fn stderr_lines(&self) -> Vec<&str> {
        self.stderr.lines().collect()
    }

    /// The first row whose name cell starts with `name`.
    pub fn row(&self, name: &str) -> Option<&str> {
        self.stdout.lines().find(|l| {
            let rest = strip_glyphs(l);
            rest.starts_with(name) && rest[name.len()..].chars().next().is_none_or(|c| c == ' ')
        })
    }

    /// Like [`Run::row`], but panics with the output when there is no row.
    pub fn must_row(&self, name: &str) -> &str {
        self.row(name)
            .unwrap_or_else(|| panic!("no row for {name:?} in:\n{}", self.stdout))
    }

    /// The cells of the row for `name` (see [`cells`]).
    pub fn cells(&self, name: &str) -> Vec<&str> {
        cells(self.must_row(name))
    }

    pub fn header(&self) -> &str {
        self.stdout.lines().next().expect("a header line")
    }

    pub fn footer(&self) -> &str {
        self.stdout.lines().last().expect("a footer line")
    }
}

/// Runs teak in the fixture with the shared test flags, then `args`.
pub fn run(fixture: &Fixture, args: &[&str]) -> Run {
    run_in(fixture.path(), args)
}

/// Runs teak in `dir` with the shared test flags, then `args`.
pub fn run_in(dir: &Path, args: &[&str]) -> Run {
    Run::from_output(
        command(dir)
            .args(DEFAULT_FLAGS)
            .args(args)
            .output()
            .expect("run teak"),
    )
}

/// The tree glyph columns at the start of a row, Unicode or ASCII.
const GLYPHS: &[&str] = &["├── ", "└── ", "│   ", "|-- ", "`-- ", "|   ", "    "];

/// The row text after the tree glyphs.
pub fn strip_glyphs(line: &str) -> &str {
    let mut rest = line;
    'outer: loop {
        for g in GLYPHS {
            if let Some(r) = rest.strip_prefix(g) {
                rest = r;
                continue 'outer;
            }
        }
        return rest;
    }
}

/// The depth of a row: the number of glyph columns before its name.
pub fn depth(line: &str) -> usize {
    let rest = strip_glyphs(line);
    (line.chars().count() - rest.chars().count()) / 4
}

/// The cells of a row: the text after the glyphs, split on runs of two or
/// more spaces. Empty cells do not show up.
pub fn cells(line: &str) -> Vec<&str> {
    strip_glyphs(line)
        .split("  ")
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .collect()
}

/// The display width of a line.
pub fn width(line: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(line)
}

/// Strips ANSI SGR sequences.
pub fn strip_ansi(text: &str) -> String {
    let re = regex::Regex::new("\x1b\\[[0-9;]*m").unwrap();
    re.replace_all(text, "").into_owned()
}

/// Runs teak in a pseudo-terminal with `rows` × `cols`. Returns stdout and
/// stderr together, with `\r\n` turned into `\n`.
pub fn run_pty(dir: &Path, args: &[&str], rows: u16, cols: u16, env: &[(&str, &str)]) -> String {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    use std::io::Read;

    let pair = native_pty_system()
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");
    let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_teak"));
    cmd.args(args);
    cmd.cwd(dir);
    cmd.env("SOURCE_DATE_EPOCH", SCAN_TIME.to_string());
    for var in [
        "NO_COLOR",
        "CLICOLOR_FORCE",
        "LS_COLORS",
        "COLUMNS",
        "LINES",
    ] {
        cmd.env_remove(var);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = pair.slave.spawn_command(cmd).expect("spawn in pty");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("pty reader");
    let mut bytes = Vec::new();
    // The read ends with EIO when the child closes the pty.
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => bytes.extend_from_slice(&buf[..n]),
        }
    }
    child.wait().expect("wait for teak");
    String::from_utf8(bytes)
        .expect("pty output is UTF-8")
        .replace("\r\n", "\n")
}

/// Runs teak and kills it after `limit`. Returns `None` on a timeout.
pub fn run_with_timeout(dir: &Path, args: &[&str], limit: Duration) -> Option<Run> {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::Instant;

    let mut child = command(dir)
        .args(DEFAULT_FLAGS)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn teak");
    // Drain both pipes, so a full pipe never blocks teak.
    let drain = |mut r: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            r.read_to_end(&mut buf).ok();
            buf
        })
    };
    let out = drain(Box::new(child.stdout.take().unwrap()));
    let err = drain(Box::new(child.stderr.take().unwrap()));
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("poll teak") {
            return Some(Run {
                code: status.code(),
                stdout: String::from_utf8(out.join().unwrap()).expect("stdout is UTF-8"),
                stderr: String::from_utf8(err.join().unwrap()).expect("stderr is UTF-8"),
            });
        }
        if start.elapsed() > limit {
            child.kill().ok();
            child.wait().ok();
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The detection corpus: small files with wrong and missing extensions.
/// `tests/corpus/EXPECTED.tsv` lists the label that each file must get.
pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// The bytes of a corpus file.
pub fn corpus(name: &str) -> Vec<u8> {
    fs::read(corpus_dir().join(name)).unwrap_or_else(|e| panic!("corpus file {name}: {e}"))
}

/// The built-in MIT template, filled in with a year and a holder.
pub fn mit_license(year: u32, holder: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/boilerplate/MIT.txt");
    fs::read_to_string(path)
        .expect("MIT template")
        .replace("[year] [fullname]", &format!("{year} {holder}"))
}

/// True when the tests run as root (mode-000 checks do not work then).
pub fn is_root() -> bool {
    // SAFETY: getuid has no preconditions.
    unsafe { libc::getuid() == 0 }
}

/// Runs teak with `TEAK_TEST_REPORT` set and returns the run and the report.
#[cfg(feature = "test-hooks")]
pub fn run_report(dir: &Path, args: &[&str]) -> (Run, Report) {
    run_report_env(dir, args, &[])
}

/// Like [`run_report`], with more environment variables.
#[cfg(feature = "test-hooks")]
pub fn run_report_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> (Run, Report) {
    let report_dir = tempfile::tempdir().expect("report dir");
    let report = report_dir.path().join("report.json");
    let out = command(dir)
        .args(DEFAULT_FLAGS)
        .args(args)
        .env("TEAK_TEST_REPORT", &report)
        .envs(env.iter().copied())
        .output()
        .expect("run teak");
    (Run::from_output(out), read_report(&report))
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

/// Parses `args` (without the program name) into teak options.
pub fn opts(args: &[&str]) -> teak::cli::Opts {
    let argv = std::iter::once("teak").chain(args.iter().copied());
    match teak::cli::parse_from(argv).expect("valid flags") {
        teak::cli::Parsed::Run(opts) => opts,
        teak::cli::Parsed::Version => panic!("--version is not a scan"),
    }
}

/// Builds the view of `dir` through the library, as `teak args` would.
pub fn scan(dir: &Path, args: &[&str]) -> teak::Scan {
    let opts = opts(args);
    let budget = match opts.budget {
        teak::cli::Budget::Lines(n) => n,
        teak::cli::Budget::Auto => 100,
    };
    let mut cx = teak::Context::new(&opts);
    teak::build(dir, &opts, &mut cx, budget, SCAN_TIME as i64).expect("scan")
}

/// The node at `rel` (a `/`-joined path below the root).
pub fn node(scan: &teak::Scan, rel: &str) -> teak::tree::NodeId {
    scan.tree
        .find(rel)
        .unwrap_or_else(|| panic!("no node {rel}"))
}

/// The nodes in the chaff row of the directory `dir`, if it has one.
pub fn chaff_items(scan: &teak::Scan, dir: teak::tree::NodeId) -> Option<Vec<teak::tree::NodeId>> {
    scan.view.rows.iter().find_map(|r| match &r.kind {
        teak::view::RowKind::Chaff { parent, items, .. } if *parent == dir => {
            Some(items.iter().map(|i| i.node).collect())
        }
        _ => None,
    })
}
