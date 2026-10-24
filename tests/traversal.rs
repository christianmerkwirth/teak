//! Traversal and safety of the walk: FR-01 to FR-09.

mod support;

use std::os::unix::ffi::OsStrExt;
use std::time::Duration;

use support::{Fixture, cells, is_root, run, run_in, run_with_timeout, width};

// FR-01: Run on paths

#[test]
fn fr01_no_path_shows_the_current_directory() {
    let fx = Fixture::new();
    fx.file("proj/a.txt", "hello\n");
    let out = run_in(&fx.abs("proj"), &[]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(out.header().starts_with("proj/"), "{}", out.stdout);
    out.must_row("a.txt");
}

#[test]
fn fr01_several_paths_give_one_view_each() {
    let fx = Fixture::new();
    for i in 0..10 {
        fx.file(&format!("p1/f{i:02}.txt"), "1\n");
        fx.file(&format!("p2/g{i:02}.txt"), "2\n");
    }
    let out = run(&fx, &["p1", "p2", "-n", "6"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let lines = out.lines();
    let heads: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("p1/") || l.starts_with("p2/"))
        .map(|(i, _)| i)
        .collect();
    let feet: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("── "))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(heads.len(), 2, "{}", out.stdout);
    assert_eq!(feet.len(), 2, "{}", out.stdout);
    assert!(lines[heads[0]].starts_with("p1/"));
    assert!(lines[heads[1]].starts_with("p2/"));
    // Each view runs from its header to its footer, within the budget of 6.
    assert!(heads[0] < feet[0] && feet[0] < heads[1] && heads[1] < feet[1]);
    assert!(feet[0] - heads[0] < 6, "{}", out.stdout);
    assert!(feet[1] - heads[1] < 6, "{}", out.stdout);
}

#[test]
fn fr01_missing_path_is_one_error_and_the_rest_still_shows() {
    let fx = Fixture::new();
    fx.file("p1/a.txt", "a\n");
    let out = run(&fx, &["missing", "p1"]);
    assert_eq!(out.code, Some(1), "{out:?}");
    let errs = out.stderr_lines();
    assert_eq!(errs.len(), 1, "{}", out.stderr);
    assert!(errs[0].contains("missing"), "{}", out.stderr);
    assert!(out.header().starts_with("p1/"), "{}", out.stdout);
}

#[test]
fn fr01_a_file_path_shows_one_row_with_all_columns() {
    let fx = Fixture::new();
    fx.file("notes.txt", "some notes\n")
        .mtime("notes.txt", 7200);
    let out = run(&fx, &["notes.txt"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let lines = out.lines();
    assert_eq!(lines.len(), 3, "{}", out.stdout);
    assert!(lines[0].starts_with("notes.txt"), "{}", out.stdout);
    let row = cells(lines[1]);
    assert_eq!(row[0], "notes.txt", "{}", out.stdout);
    assert!(row.contains(&"11"), "size: {row:?}");
    assert!(row.contains(&"txt"), "type: {row:?}");
    assert!(row.contains(&"2h"), "age: {row:?}");
}

// FR-02: Symlinks

fn link_fixture() -> Fixture {
    let fx = Fixture::new();
    for i in 0..100 {
        fx.file(&format!("data/f{i:03}.txt"), "x\n");
    }
    fx.symlink("link", "data");
    fx
}

#[test]
fn fr02_symlinks_are_not_followed() {
    let fx = link_fixture();
    let out = run(&fx, &[]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(out.must_row("link").contains("⇢ data"), "{}", out.stdout);
    // 100 files in data/ plus the link itself.
    assert!(out.header().contains("· 101 files"), "{}", out.stdout);
    assert!(out.cells("data/").contains(&"100"), "{}", out.stdout);
}

#[test]
fn fr02_follow_shows_the_link_as_a_directory() {
    let fx = link_fixture();
    let out = run(&fx, &["--follow"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(out.cells("link/").contains(&"100"), "{}", out.stdout);
    assert!(out.header().contains("· 200 files"), "{}", out.stdout);
}

#[test]
fn fr02_follow_marks_a_loop_and_ends() {
    let fx = Fixture::new();
    fx.dir("a").symlink("a/b", "../a");
    let out =
        run_with_timeout(fx.path(), &["--follow"], Duration::from_secs(10)).expect("teak ends");
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(out.must_row("b").contains('↻'), "{}", out.stdout);
}

#[test]
fn fr02_broken_link_shows_its_target_and_a_mark() {
    let fx = Fixture::new();
    fx.file("ok.txt", "ok\n").symlink("dead", "nowhere");
    let out = run(&fx, &[]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(
        out.must_row("dead").contains("⇢ nowhere (broken)"),
        "{}",
        out.stdout
    );
}

// FR-03: Special files

#[test]
fn fr03_fifo_and_socket_are_typed_and_never_opened() {
    let fx = Fixture::new();
    fx.file("a.txt", "a\n");
    let fifo = std::ffi::CString::new(fx.abs("pipe").as_os_str().as_bytes()).unwrap();
    // SAFETY: a valid C string and a plain mode.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o644) }, 0);
    let _sock = std::os::unix::net::UnixListener::bind(fx.abs("sock")).unwrap();
    let out = run_with_timeout(
        fx.path(),
        &["--detect", "full", "--preview", "full"],
        Duration::from_secs(2),
    )
    .expect("teak ends within 2 s (an open FIFO would block)");
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(out.cells("pipe").contains(&"fifo"), "{}", out.stdout);
    assert!(out.cells("sock").contains(&"socket"), "{}", out.stdout);
}

#[test]
fn fr03_dev_null_is_a_device() {
    let fx = Fixture::new();
    let out = run(&fx, &["/dev/null", "--detect", "full"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(cells(out.lines()[1]).contains(&"device"), "{}", out.stdout);
}

#[cfg(feature = "test-hooks")]
#[test]
fn fr03_dev_null_is_not_opened() {
    let fx = Fixture::new();
    let (out, report) = support::run_report(
        fx.path(),
        &["/dev/null", "--detect", "full", "--preview", "full"],
    );
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(report.reads.is_empty(), "{:?}", report.reads);
}

// FR-05: No access

#[test]
fn fr05_unreadable_dir_is_marked_and_the_scan_goes_on() {
    if is_root() {
        eprintln!("skipped: root can read mode-000 directories");
        return;
    }
    let fx = Fixture::new();
    fx.file("locked/secret.txt", "s\n")
        .file("ok.txt", "ok\n")
        .mode("locked", 0o000);
    let out = run(&fx, &[]);
    fx.mode("locked", 0o755);
    assert_eq!(out.code, Some(1), "{out:?}");
    assert!(out.must_row("locked/").contains('⊘'), "{}", out.stdout);
    out.must_row("ok.txt");
    let errs = out.stderr_lines();
    assert_eq!(errs.len(), 1, "{}", out.stderr);
    assert!(errs[0].contains("locked"), "{}", out.stderr);
}

#[test]
fn fr05_unreadable_root_is_marked_in_the_header() {
    if is_root() {
        eprintln!("skipped: root can read mode-000 directories");
        return;
    }
    let fx = Fixture::new();
    fx.file("root/a.txt", "a\n").mode("root", 0o000);
    let out = run(&fx, &["root"]);
    fx.mode("root", 0o755);
    assert_eq!(out.code, Some(1), "{out:?}");
    assert!(out.header().starts_with("root/ ⊘"), "{}", out.stdout);
}

// FR-06: One file system

#[test]
fn fr06_one_file_system_stops_at_a_mount_point() {
    let probe = std::process::Command::new("unshare")
        .args(["-r", "-m", "true"])
        .output();
    if !probe.is_ok_and(|o| o.status.success()) {
        eprintln!("skipped: cannot mount a file system here (no user namespaces)");
        return;
    }
    let fx = Fixture::new();
    fx.file("top.txt", "t\n").dir("mnt");
    // An unprivileged user and mount namespace lets us mount a tmpfs.
    let script = r#"mount -t tmpfs none "$1/mnt" && head -c 5000 /dev/zero > "$1/mnt/big" && exec "$2" --format plain --width 120 --color never -x "$1""#;
    let out = std::process::Command::new("unshare")
        .args(["-r", "-m", "sh", "-c", script, "sh"])
        .arg(fx.path())
        .arg(env!("CARGO_BIN_EXE_teak"))
        .env("SOURCE_DATE_EPOCH", support::SCAN_TIME.to_string())
        .output();
    let out = support::Run::from_output(out.expect("run unshare"));
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(out.must_row("mnt/").contains('⊞'), "{}", out.stdout);
    assert!(out.header().contains("· 1 file"), "{}", out.stdout);
    assert!(out.row("big").is_none(), "{}", out.stdout);
}

// FR-07: Odd names

#[test]
fn fr07_non_utf8_name_is_escaped() {
    let fx = Fixture::new();
    let name = std::ffi::OsStr::from_bytes(&[0xFF, 0x61]);
    std::fs::write(fx.path().join(name), "x\n").unwrap();
    let out = run(&fx, &[]);
    assert_eq!(out.code, Some(0), "{out:?}");
    out.must_row("\\xFFa");
}

#[test]
fn fr07_long_name_is_cut_in_the_middle() {
    let fx = Fixture::new();
    let name = format!("{}.txt", "n".repeat(246));
    fx.file(&name, "x\n");
    let out = run(&fx, &["--width", "80"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let row = out
        .lines()
        .into_iter()
        .find(|l| l.contains("nnnn"))
        .expect("a row for the long name");
    let name_cell = cells(row)[0];
    assert!(name_cell.contains('…'), "{row}");
    assert!(name_cell.ends_with(".txt"), "{row}");
    for line in out.lines() {
        assert!(width(line) <= 80, "{} columns: {line}", width(line));
    }
}

#[test]
fn fr07_deep_chain_does_not_crash_and_joins() {
    let fx = Fixture::new();
    let deep = vec!["d"; 500].join("/");
    fx.file(&format!("{deep}/leaf.txt"), "leaf\n");
    let out = run(&fx, &[]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let joined = out
        .lines()
        .into_iter()
        .find(|l| l.contains("d/d/d/"))
        .expect("one joined row for the chain");
    assert!(joined.contains('…'), "{joined}");
    out.must_row("leaf.txt");
    for line in out.lines() {
        assert!(width(line) <= 120, "{line}");
    }
}

// FR-08: Pseudo file systems

#[cfg(target_os = "linux")]
#[test]
fn fr08_proc_is_scanned_without_content_reads() {
    let fx = Fixture::new();
    let out = run_with_timeout(
        fx.path(),
        &["/proc", "-n", "20", "--detect", "full", "--preview", "full"],
        Duration::from_secs(60),
    )
    .expect("teak ends");
    assert!(matches!(out.code, Some(0 | 1)), "{:?}", out.code);
    assert!(out.lines().len() <= 20, "{}", out.stdout);
    assert!(out.header().starts_with("/proc/"), "{}", out.stdout);
}

#[cfg(all(target_os = "linux", feature = "test-hooks"))]
#[test]
fn fr08_proc_types_come_from_stat_and_names() {
    let fx = Fixture::new();
    let (out, report) = support::run_report(
        fx.path(),
        &["/proc", "-n", "20", "--detect", "full", "--preview", "full"],
    );
    assert!(matches!(out.code, Some(0 | 1)), "{:?}", out.code);
    assert!(report.reads.is_empty(), "{:?}", report.reads);
}

// FR-09: Hard links

#[test]
fn fr09_hard_links_count_once() {
    let fx = Fixture::new();
    fx.sized("d/f1", 1 << 20);
    std::fs::hard_link(fx.abs("d/f1"), fx.abs("d/f2")).unwrap();
    let out = run(&fx, &["--bytes"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(out.cells("d/").contains(&"1048576"), "{}", out.stdout);
    out.must_row("f1");
    out.must_row("f2");
}
