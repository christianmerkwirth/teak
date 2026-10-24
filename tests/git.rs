//! Git status (FR-57): file and directory cells, the header, `--no-git`,
//! and the edge cases.

mod support;

use support::{Fixture, cells, git, git_commits, git_init_commit, git_set_upstream, run};

/// A repo on `main`, 2 commits ahead of `origin/main`, with `src/a.rs`
/// modified and `src/b.rs` untracked.
fn ahead_repo() -> Fixture {
    let fx = Fixture::new();
    fx.file("src/a.rs", "pub fn a() {}\n")
        .file("src/c.rs", "pub fn c() {}\n")
        .file("notes.txt", "notes\n");
    git_init_commit(&fx);
    git_set_upstream(&fx);
    git_commits(&fx, "notes.txt", 2);
    fx.file("src/a.rs", "pub fn a() { changed(); }\n")
        .file("src/b.rs", "pub fn b() {}\n");
    fx
}

#[test]
fn fr57_files_and_directories_show_their_status() {
    let fx = ahead_repo();
    let out = run(&fx, &[]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(cells(out.must_row("a.rs")).contains(&"M"), "{}", out.stdout);
    assert!(cells(out.must_row("b.rs")).contains(&"?"), "{}", out.stdout);
    let c = cells(out.must_row("c.rs"));
    assert!(!c.contains(&"M") && !c.contains(&"?"), "clean file: {c:?}");
    assert!(
        cells(out.must_row("src/")).contains(&"M2"),
        "{}",
        out.stdout
    );
    assert!(out.header().contains("git main ↑2"), "{}", out.header());
    assert!(out.header().contains("2 changed"), "{}", out.header());
}

#[test]
fn fr57_staged_new_and_deleted_files() {
    let fx = Fixture::new();
    fx.file("keep.txt", "keep\n").file("gone.txt", "gone\n");
    git_init_commit(&fx);
    fx.file("new.txt", "new\n");
    git(fx.path(), &["add", "new.txt"]);
    std::fs::remove_file(fx.abs("gone.txt")).unwrap();
    let out = run(&fx, &[]);
    assert!(
        cells(out.must_row("new.txt")).contains(&"A"),
        "{}",
        out.stdout
    );
    // A deleted file has no row; it counts in the header.
    assert!(out.header().contains("2 changed"), "{}", out.header());
}

#[test]
fn fr57_ignored_files_show_a_bang() {
    let fx = Fixture::new();
    fx.file(".gitignore", "*.log\n")
        .file("main.rs", "fn main() {}\n");
    git_init_commit(&fx);
    fx.file("build.log", "log\n");
    let out = run(&fx, &["--show", "ignored"]);
    assert!(
        cells(out.must_row("build.log")).contains(&"!"),
        "{}",
        out.stdout
    );
}

#[test]
fn fr57_no_git_turns_it_off() {
    let fx = ahead_repo();
    let out = run(&fx, &["--no-git"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(!out.header().contains("git"), "{}", out.header());
    for name in ["a.rs", "b.rs", "src/"] {
        let c = cells(out.must_row(name));
        assert!(
            !c.iter().any(|c| matches!(*c, "M" | "?" | "M2")),
            "{name}: {c:?}"
        );
    }
}

#[test]
fn fr57_outside_a_repo_there_is_no_git_column() {
    let fx = Fixture::new();
    fx.file("src/a.rs", "pub fn a() {}\n").file("b.txt", "b\n");
    let out = run(&fx, &[]);
    assert!(!out.header().contains("git"), "{}", out.header());
    for line in out.lines() {
        let c = cells(line);
        assert!(!c.iter().any(|c| matches!(*c, "M" | "?" | "A")), "{line}");
    }
}

#[test]
fn fr57_a_repo_with_no_commits() {
    let fx = Fixture::new();
    fx.file("a.txt", "a\n").file("src/b.rs", "fn b() {}\n");
    git(fx.path(), &["init", "-q"]);
    let out = run(&fx, &[]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(
        cells(out.must_row("a.txt")).contains(&"?"),
        "{}",
        out.stdout
    );
    assert!(cells(out.must_row("b.rs")).contains(&"?"), "{}", out.stdout);
    assert!(out.header().contains("git main"), "{}", out.header());
}

#[test]
fn fr57_a_subdirectory_of_a_repo() {
    let fx = ahead_repo();
    let out = support::run_in(&fx.abs("src"), &[]);
    assert!(cells(out.must_row("a.rs")).contains(&"M"), "{}", out.stdout);
    assert!(out.header().contains("git main ↑2"), "{}", out.header());
}

#[test]
fn fr57_ignored_chaff_is_not_untracked() {
    // `target/` is `build` chaff by the Rust rule pack, and gitignored.
    let fx = Fixture::new();
    fx.file("Cargo.toml", "[package]\n")
        .file(".gitignore", "/target\n")
        .file("src/main.rs", "fn main() {}\n");
    git_init_commit(&fx);
    for i in 0..5 {
        fx.file(&format!("target/debug/f{i}"), "bin");
    }
    for args in [&[][..], &["-a"], &["--no-gitignore"]] {
        let out = run(&fx, args);
        assert!(
            !out.header().contains("changed"),
            "{args:?}: {}",
            out.header()
        );
    }
}
