//! The long view (FR-24): `ls -lah` columns with `-l`.

mod support;

use support::{Fixture, cells, run};

fn id(flag: &str) -> String {
    let out = std::process::Command::new("id")
        .arg(flag)
        .output()
        .expect("run id");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn long_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file("notes.txt", "notes\n")
        .mode("notes.txt", 0o644)
        .file("run.sh", "#!/bin/sh\n")
        .mode("run.sh", 0o755)
        .file("docs/new.md", "# New\n")
        .file("docs/old.md", "# Old\n")
        .symlink("link", "notes.txt");
    fx.mtime("notes.txt", 7200)
        .mtime("run.sh", 7200)
        .mtime("docs/new.md", 3600)
        .mtime("docs/old.md", 3 * 365 * 86_400);
    fx
}

#[test]
fn fr24_long_view_shows_mode_owner_and_group() {
    let fx = long_fixture();
    let out = run(&fx, &["-l"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let (user, group) = (id("-un"), id("-gn"));
    let c = cells(out.must_row("notes.txt"));
    assert!(c.contains(&"-rw-r--r--"), "{c:?}");
    assert!(c.contains(&user.as_str()), "{user}: {c:?}");
    assert!(c.contains(&group.as_str()), "{group}: {c:?}");
    assert!(
        cells(out.must_row("run.sh")).contains(&"-rwxr-xr-x"),
        "{}",
        out.stdout
    );
    assert!(
        cells(out.must_row("link")).contains(&"lrwxrwxrwx"),
        "{}",
        out.stdout
    );
    let docs = cells(out.must_row("docs/"));
    assert!(docs.iter().any(|c| c.starts_with("drwx")), "{docs:?}");
}

#[test]
fn fr24_long_view_shows_the_oldest_age_of_a_directory() {
    let fx = long_fixture();
    let out = run(&fx, &["-l", "-L", "1"]);
    assert!(
        cells(out.must_row("docs/")).contains(&"1h–3y"),
        "{}",
        out.stdout
    );
    let short = run(&fx, &["-L", "1"]);
    assert!(
        cells(short.must_row("docs/")).contains(&"1h"),
        "{}",
        short.stdout
    );
}

#[test]
fn fr24_default_view_has_no_long_columns() {
    let fx = long_fixture();
    let out = run(&fx, &[]);
    let user = id("-un");
    let c = cells(out.must_row("notes.txt"));
    assert!(!c.contains(&"-rw-r--r--"), "{c:?}");
    assert!(!c.contains(&user.as_str()), "{c:?}");
    // Unusual permissions keep the short form.
    assert!(
        cells(out.must_row("run.sh")).contains(&"rwx"),
        "{}",
        out.stdout
    );
}

#[test]
fn fr24_owner_and_group_drop_first_when_narrow() {
    let fx = long_fixture();
    let user = id("-un");
    let out = run(&fx, &["-l", "--width", "50"]);
    for line in out.lines() {
        assert!(support::width(line) <= 50, "{line}");
    }
    let c = cells(out.must_row("notes.txt"));
    assert!(!c.contains(&user.as_str()), "{c:?}");
    assert!(c.contains(&"-rw-r--r--"), "{c:?}");
}
