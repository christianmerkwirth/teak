//! Safety: read-only (FR-04), no network (FR-10), no execution of repo code
//! (FR-76), and no secret previews (FR-77).
//!
//! Task-5 adds `md` and `json` to these runs.

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use support::{Fixture, command, run};

/// The formats that exist in this phase.
const FORMATS: &[&str] = &["tty", "plain"];

/// Path → (kind, size, mode, mtime, content hash) for every entry.
fn snapshot(root: &Path) -> BTreeMap<String, (char, u64, u32, i64, u64)> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, (char, u64, u32, i64, u64)>) {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            let meta = fs::symlink_metadata(&path).unwrap();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let ft = meta.file_type();
            let (kind, hash) = if ft.is_symlink() {
                (
                    'l',
                    fnv(fs::read_link(&path).unwrap().as_os_str().as_encoded_bytes()),
                )
            } else if ft.is_dir() {
                ('d', 0)
            } else if ft.is_file() {
                ('f', fnv(&fs::read(&path).unwrap()))
            } else {
                ('s', 0)
            };
            out.insert(
                rel,
                (
                    kind,
                    meta.len(),
                    meta.mode(),
                    meta.mtime_nsec() + meta.mtime() * 1_000_000_000,
                    hash,
                ),
            );
            if ft.is_dir() {
                walk(&path, root, out);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn busy_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file("src/main.rs", "fn main() {}\n")
        .file("Cargo.toml", "[package]\n")
        .file("target/debug/app", "bin")
        .file(".git/HEAD", "ref: refs/heads/main\n")
        .file(".gitignore", "*.log\n")
        .file("build.log", "log\n")
        .file(".env", "API_KEY=sk-test-123\n")
        .file("deploy.sh", "#!/bin/sh\ntouch /tmp/never\n")
        .mode("deploy.sh", 0o755)
        .symlink("link", "src")
        .symlink("dead", "nowhere");
    for i in 1..=6 {
        fx.file(&format!("img/IMG_{i:04}.png"), "\u{89}PNG");
    }
    fx
}

// FR-04: Read-only

#[test]
fn fr04_scans_change_nothing() {
    let fx = busy_fixture();
    let before = snapshot(fx.path());
    for format in FORMATS {
        for extra in [
            &[][..],
            &["--ascii", "--follow"],
            &["-A", "--color", "always"],
        ] {
            let mut args = vec![
                "-a",
                "--detect",
                "full",
                "--preview",
                "full",
                "--format",
                format,
            ];
            args.extend_from_slice(extra);
            let out = run(&fx, &args);
            assert_eq!(out.code, Some(0), "{args:?}: {out:?}");
        }
    }
    assert_eq!(snapshot(fx.path()), before);
}

#[test]
fn fr04_read_only_tree_exits_0() {
    let fx = busy_fixture();
    fn lock(dir: &Path) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let meta = fs::symlink_metadata(&p).unwrap();
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                lock(&p);
            }
            let mode = meta.permissions().mode() & !0o222;
            fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
        }
    }
    fn unlock(dir: &Path) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let meta = fs::symlink_metadata(&p).unwrap();
            if meta.file_type().is_symlink() {
                continue;
            }
            let mode = meta.permissions().mode() | 0o200;
            fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
            if meta.is_dir() {
                unlock(&p);
            }
        }
    }
    lock(fx.path());
    let out = run(&fx, &["-a", "--detect", "full"]);
    unlock(fx.path());
    assert_eq!(out.code, Some(0), "{out:?}");
}

// FR-10: No network

#[test]
fn fr10_no_network_crates_in_the_normal_graph() {
    let out = std::process::Command::new(env!("CARGO"))
        .args(["tree", "--offline", "-e", "normal", "--prefix", "none"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run cargo tree");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let deps = String::from_utf8(out.stdout).unwrap();
    const BANNED: &[&str] = &[
        "reqwest",
        "ureq",
        "hyper",
        "curl",
        "isahc",
        "attohttpc",
        "surf",
        "sentry",
        "opentelemetry",
    ];
    for line in deps.lines() {
        let name = line.split_whitespace().next().unwrap_or("");
        assert!(
            !BANNED.iter().any(|b| name.starts_with(b)),
            "network crate in the normal graph: {line}"
        );
    }
}

#[test]
fn fr10_same_output_without_network() {
    let probe = std::process::Command::new("unshare")
        .args(["-r", "-n", "true"])
        .output();
    if !probe.is_ok_and(|o| o.status.success()) {
        eprintln!("skipped: no unprivileged network namespace here");
        return;
    }
    let fx = busy_fixture();
    let with = command(fx.path())
        .args(support::DEFAULT_FLAGS)
        .output()
        .unwrap();
    let without = std::process::Command::new("unshare")
        .args(["-r", "-n", env!("CARGO_BIN_EXE_teak")])
        .args(support::DEFAULT_FLAGS)
        .current_dir(fx.path())
        .env("SOURCE_DATE_EPOCH", support::SCAN_TIME.to_string())
        .output()
        .unwrap();
    assert_eq!(with.stdout, without.stdout);
}

// FR-76: No execution of repo code

struct Marks {
    _dir: tempfile::TempDir,
    paths: [std::path::PathBuf; 4],
}

/// A real repo whose config, hooks, and attributes would run commands: an
/// fsmonitor, a post-checkout hook, a clean/smudge filter, and a script in
/// the tree. `src/lib.rs` is modified and `.gitattributes` is untracked, so
/// git status has work to do.
fn hostile_repo() -> (Fixture, Marks) {
    let dir = tempfile::tempdir().unwrap();
    let paths = [1, 2, 3, 4].map(|i| dir.path().join(format!("mark{i}")));
    let fx = Fixture::new();
    let sh = |p: &Path| format!("#!/bin/sh\ntouch '{}'\n", p.display());
    fx.file("run.sh", sh(&paths[3]))
        .mode("run.sh", 0o755)
        .file("src/lib.rs", "pub fn f() {}\n");
    support::git_init_commit(&fx);
    let config = fx.abs(".git/config");
    let mut text = fs::read_to_string(&config).unwrap();
    text.push_str(&format!(
        "[core]\n\tfsmonitor = touch '{}'\n[filter \"x\"]\n\tclean = touch '{}'\n\tsmudge = touch '{}'\n\trequired = true\n",
        paths[0].display(),
        paths[2].display(),
        paths[2].display()
    ));
    fs::write(&config, text).unwrap();
    fx.file(".git/hooks/post-checkout", sh(&paths[1]))
        .mode(".git/hooks/post-checkout", 0o755)
        .file(".gitattributes", "* filter=x\n")
        .file("src/lib.rs", "pub fn f() { g(); }\n");
    (fx, Marks { _dir: dir, paths })
}

/// The git cells are right: `src/lib.rs` is `M`, `.gitattributes` is `?`.
fn assert_git_cells(out: &support::Run) {
    let lib = support::cells(out.must_row("lib.rs"));
    assert!(lib.contains(&"M"), "{}", out.stdout);
    let attrs = support::cells(out.must_row(".gitattributes"));
    assert!(attrs.contains(&"?"), "{}", out.stdout);
    let run_sh = support::cells(out.must_row("run.sh"));
    assert!(!run_sh.contains(&"M"), "{}", out.stdout);
}

#[test]
fn fr76_nothing_from_the_tree_runs() {
    let (fx, marks) = hostile_repo();
    for format in FORMATS {
        let out = run(
            &fx,
            &[
                "-a",
                "--detect",
                "full",
                "--preview",
                "full",
                "--format",
                format,
            ],
        );
        assert_eq!(out.code, Some(0), "{out:?}");
        assert_git_cells(&out);
    }
    for m in &marks.paths {
        assert!(!m.exists(), "{} exists", m.display());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn fr76_only_one_execve() {
    if std::process::Command::new("strace")
        .arg("-V")
        .output()
        .is_err()
    {
        eprintln!("skipped: strace is not installed");
        return;
    }
    let (fx, _marks) = hostile_repo();
    let trace = tempfile::NamedTempFile::new().unwrap();
    let out = std::process::Command::new("strace")
        .args(["-f", "-e", "trace=execve", "-o"])
        .arg(trace.path())
        .arg(env!("CARGO_BIN_EXE_teak"))
        .args(support::DEFAULT_FLAGS)
        .args(["-a", "--detect", "full", "--preview", "full"])
        .current_dir(fx.path())
        .output()
        .unwrap();
    if !out.status.success() && String::from_utf8_lossy(&out.stderr).contains("ptrace") {
        eprintln!("skipped: ptrace is not allowed here");
        return;
    }
    let text = fs::read_to_string(trace.path()).unwrap();
    let execs: Vec<_> = text.lines().filter(|l| l.contains("execve(")).collect();
    assert_eq!(execs.len(), 1, "{text}");
    assert!(execs[0].contains("teak"), "{text}");
}

// FR-77: No secret previews

fn secrets_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file(".env", "API_KEY=sk-test-123\n")
        .file("id_rsa", "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXk=\n-----END OPENSSH PRIVATE KEY-----\n")
        .file("server.pem", "-----BEGIN PRIVATE KEY-----\nMIIE\n-----END PRIVATE KEY-----\n")
        .file("credentials.json", "{\"token\": \"sk-test-123\"}\n")
        .file(".netrc", "machine x login u password sk-test-123\n")
        .file("main.rs", "fn main() {}\n");
    fx
}

#[test]
fn fr77_secret_rows_show_type_and_size_only() {
    let fx = secrets_fixture();
    for format in FORMATS {
        let out = run(&fx, &["-a", "--preview", "full", "--format", format]);
        assert!(!out.stdout.contains("sk-test-123"), "{}", out.stdout);
        assert!(!out.stdout.contains("BEGIN"), "{}", out.stdout);
        for name in [".env", "id_rsa", "server.pem", "credentials.json", ".netrc"] {
            let row = out.must_row(name);
            assert!(row.contains("no preview (secret)"), "{row}");
            let c = support::cells(row);
            assert!(c.len() >= 4, "name, size, type, age: {row}");
        }
        // Real previews are on: the other file shows its first line.
        let main = out.must_row("main.rs");
        assert!(main.contains("fn main() {}"), "{main}");
    }
}

#[test]
fn fr77_a_private_key_by_content_is_a_secret() {
    let fx = Fixture::new();
    fx.file(
        "backup.txt",
        "-----BEGIN OPENSSH PRIVATE KEY-----\nb3Blbg==\n",
    )
    .file("notes.txt", "hello\n");
    let out = run(&fx, &["--preview", "full"]);
    assert!(
        out.must_row("backup.txt").contains("no preview (secret)"),
        "{}",
        out.stdout
    );
    assert!(
        !out.must_row("notes.txt").contains("secret"),
        "{}",
        out.stdout
    );
    assert!(!out.stdout.contains("BEGIN"), "{}", out.stdout);
}
