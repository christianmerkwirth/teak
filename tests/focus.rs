//! Focus (FR-18): the path opens, most of the budget goes inside it, and
//! the context around it stays.

mod support;

use support::{Fixture, depth, run, strip_glyphs, word};

/// `src/api/` with 200 files, more in `src/`, and 8 other top-level
/// directories with files of their own.
fn repo() -> Fixture {
    let fx = Fixture::new();
    for i in 0..200 {
        fx.sized(&format!("src/api/{}.rs", word(i)), 300);
    }
    fx.sized("src/main.rs", 2000);
    for i in 0..20 {
        fx.sized(&format!("src/util/{}.rs", word(i)), 5000);
    }
    for d in 0..8 {
        for i in 0..30 {
            fx.sized(&format!("top{d}/{}.txt", word(i)), 4000);
        }
    }
    fx.mtime_all(3600);
    fx
}

/// The rows below the row of `name` that are deeper than it.
fn rows_below<'a>(lines: &[&'a str], name: &str) -> Vec<&'a str> {
    let Some(i) = lines.iter().position(|l| strip_glyphs(l).starts_with(name)) else {
        return Vec::new();
    };
    let d = depth(lines[i]);
    lines[i + 1..]
        .iter()
        .take_while(|l| depth(l) > d)
        .copied()
        .collect()
}

#[test]
fn fr18_focus_opens_the_path_and_spends_the_budget_there() {
    let fx = repo();
    let out = run(&fx, &["--focus", "src/api", "-n", "30"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let lines = out.lines();
    assert!(lines.len() <= 30, "{}", out.stdout);
    let in_src = rows_below(&lines, "src/");
    assert!(!in_src.is_empty(), "src/ is open:\n{}", out.stdout);
    let in_api = rows_below(&lines, "api/");
    assert!(!in_api.is_empty(), "api/ is open:\n{}", out.stdout);
    let rows = lines.len() - 2;
    assert!(
        in_api.len() * 2 >= rows,
        "{} of {rows} rows in api/:\n{}",
        in_api.len(),
        out.stdout
    );
    // Each other top-level directory is a row, or counted in the root's
    // overflow row.
    let shown = (0..8)
        .filter(|d| out.row(&format!("top{d}/")).is_some())
        .count();
    let overflow: usize = lines
        .iter()
        .filter(|l| depth(l) == 1 && strip_glyphs(l).starts_with("… "))
        .filter_map(|l| {
            let rest = strip_glyphs(l).trim_start_matches("… ");
            rest.split(' ').next()?.parse::<usize>().ok()
        })
        .sum();
    assert!(
        shown + overflow >= 8,
        "{shown} + {overflow}:\n{}",
        out.stdout
    );
}

#[test]
fn fr18_focus_resolves_against_the_current_directory() {
    let fx = Fixture::new();
    for i in 0..40 {
        fx.sized(&format!("proj/a/deep/{}.txt", word(i)), 100);
        fx.sized(&format!("proj/b/{}.txt", word(i)), 100_000);
    }
    let out = run(&fx, &["proj", "--focus", "proj/a/deep", "-n", "12"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(
        !rows_below(&out.lines(), "a/deep/").is_empty(),
        "{}",
        out.stdout
    );
}

#[test]
fn fr18_focus_outside_the_root_is_a_usage_error() {
    let fx = repo();
    for bad in ["nope", "/etc", "../.."] {
        let out = run(&fx, &["--focus", bad]);
        assert_eq!(out.code, Some(2), "--focus {bad}: {out:?}");
        assert!(out.stderr.contains("--focus"), "{}", out.stderr);
        assert!(out.stdout.is_empty(), "{}", out.stdout);
    }
}
