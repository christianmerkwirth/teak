//! `--stats` (FR-61): the header, a type-group table, and the footer.

mod support;

use support::{Fixture, cells, run, word};

fn mixed() -> Fixture {
    let fx = Fixture::new();
    for i in 0..30 {
        fx.file(&format!("src/{}.rs", word(i)), "fn f() {}\n");
    }
    for i in 0..5 {
        fx.sized(&format!("data/{}.csv", word(i)), 50_000);
    }
    fx.file("README.md", "# Title\n")
        .file("notes.txt", "notes\n");
    fx
}

#[test]
fn fr61_stats_has_header_table_and_footer() {
    let fx = mixed();
    let out = run(&fx, &["--stats"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let lines = out.lines();
    assert!(lines[0].contains("37 files"), "{}", out.stdout);
    assert!(out.footer().starts_with("── "), "{}", out.stdout);
    let table = &lines[1..lines.len() - 1];
    assert_eq!(
        cells(table[0]),
        ["group", "files", "size"],
        "{}",
        out.stdout
    );
    let rows: Vec<Vec<&str>> = table[1..].iter().map(|l| cells(l)).collect();
    let group = |g: &str| {
        rows.iter()
            .find(|r| r[0] == g)
            .unwrap_or_else(|| panic!("no {g} row:\n{}", out.stdout))
    };
    assert_eq!(group("code")[1], "30");
    assert_eq!(group("data")[1], "5");
    assert_eq!(group("text")[1], "2");
    // Sorted by size, high first.
    assert_eq!(rows[0][0], "data", "{}", out.stdout);
    for line in &lines {
        assert!(!line.contains("├──") && !line.contains("└──"), "{line}");
    }
}

#[test]
fn fr61_stats_keeps_the_budget() {
    let fx = mixed();
    let out = run(&fx, &["--stats", "-n", "4"]);
    assert!(out.lines().len() <= 4, "{}", out.stdout);
    assert!(out.footer().starts_with("── "), "{}", out.stdout);
}
