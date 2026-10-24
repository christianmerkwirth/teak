//! Line budget and selection: FR-11, FR-13 to FR-17, FR-19, FR-70.

mod support;

use support::{Fixture, cells, depth, is_root, run, run_pty, strip_glyphs, word};

/// 500 files in 40 directories (some nested).
fn wide_fixture() -> Fixture {
    let fx = Fixture::new();
    for i in 0..500 {
        let dir = i % 40;
        let path = if dir % 4 == 0 {
            format!("d{dir:02}/sub/f{i:03}.txt")
        } else {
            format!("d{dir:02}/f{i:03}.txt")
        };
        fx.sized(&path, 100 + i as u64);
    }
    fx
}

// FR-11: Budget flag

#[test]
fn fr11_default_budget_is_100_lines() {
    let fx = wide_fixture();
    let out = run(&fx, &[]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(out.lines().len() <= 100, "{} lines", out.lines().len());
    assert!(out.footer().contains("of 100 lines"), "{}", out.footer());
}

#[test]
fn fr11_n_sets_the_budget() {
    let fx = wide_fixture();
    let out = run(&fx, &["-n", "30"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(out.lines().len() <= 30, "{}", out.stdout);
    assert!(out.footer().contains("of 30 lines"), "{}", out.footer());
}

#[test]
fn fr11_auto_fits_the_terminal_height() {
    let fx = wide_fixture();
    let text = run_pty(fx.path(), &["-n", "auto", "--color", "never"], 40, 120, &[]);
    let lines: Vec<_> = text.lines().collect();
    assert!(lines.len() <= 38, "{} lines:\n{text}", lines.len());
    assert!(text.contains("of 38 lines"), "{text}");
}

#[test]
fn fr11_auto_in_a_pipe_is_100() {
    let fx = wide_fixture();
    let out = run(&fx, &["-n", "auto"]);
    assert!(out.footer().contains("of 100 lines"), "{}", out.footer());
}

#[test]
fn fr11_bad_budgets_are_usage_errors() {
    let fx = Fixture::new();
    for bad in ["abc", "0", "2"] {
        let out = run(&fx, &["-n", bad]);
        assert_eq!(out.code, Some(2), "-n {bad}: {out:?}");
        assert!(
            out.stderr.contains("smallest budget is 3"),
            "-n {bad}: {}",
            out.stderr
        );
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn fr11_small_tree_uses_fewer_lines_than_the_budget() {
    let fx = Fixture::new();
    for i in 0..5 {
        fx.file(&format!("{}.txt", word(i)), "x\n");
    }
    let out = run(&fx, &["-n", "100"]);
    assert_eq!(out.lines().len(), 7, "{}", out.stdout);
    for i in 0..5 {
        out.must_row(&format!("{}.txt", word(i)));
    }
}

// FR-13: Selection by salience

#[test]
fn fr13_salience_opens_the_deep_recent_bulk() {
    let fx = Fixture::new();
    fx.sized("a/old.txt", 1000);
    for i in 0..50 {
        fx.sized(&format!("z/deep/deeper/f{}.dat", word(i)), 200);
    }
    fx.mtime_all(1800);
    fx.mtime("a/old.txt", 3 * 365 * 86_400);
    let out = run(&fx, &["-n", "12"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(out.lines().len() <= 12, "{}", out.stdout);
    let deep_rows = out
        .lines()
        .into_iter()
        .filter(|l| strip_glyphs(l).starts_with('f') && depth(l) == 2)
        .count();
    assert!(deep_rows >= 3, "files from z/deep/deeper/:\n{}", out.stdout);
    let a = out.must_row("a/");
    assert!(cells(a).contains(&"1"), "a/ is one folded row: {a}");
    assert!(out.row("old.txt").is_none(), "{}", out.stdout);
}

#[test]
fn fr13_ties_break_by_name_and_runs_agree() {
    let fx = Fixture::new();
    for d in 0..10 {
        for f in 0..5 {
            fx.sized(&format!("s{d}/{}.txt", word(f)), 500);
        }
    }
    fx.mtime_all(3600);
    // 10 dirs fill 10 of the 14 row lines; 4 lines are left to open one.
    let one = run(&fx, &["-n", "16"]);
    let two = run(&fx, &["-n", "16"]);
    assert_eq!(one.stdout, two.stdout);
    // The opened directory is the first by name.
    let open: Vec<_> = one.lines().into_iter().filter(|l| depth(l) == 2).collect();
    assert!(!open.is_empty(), "{}", one.stdout);
    let first_open = one.lines().into_iter().position(|l| depth(l) == 2).unwrap();
    assert!(
        strip_glyphs(one.lines()[first_open - 1]).starts_with("s0/"),
        "{}",
        one.stdout
    );
}

// FR-14: Overflow row

#[test]
fn fr14_overflow_row_counts_the_rest() {
    let fx = Fixture::new();
    for i in 0..50 {
        fx.sized(&format!("logs/l{}.txt", word(i)), 1024);
    }
    // Header, logs/, 8 files, overflow, footer.
    let out = run(&fx, &["-n", "12"]);
    let shown = out
        .lines()
        .into_iter()
        .filter(|l| strip_glyphs(l).starts_with('l') && depth(l) == 2)
        .count();
    assert_eq!(shown, 8, "{}", out.stdout);
    let over = out.cells("… 42 files");
    assert!(over.contains(&"42K"), "{over:?}");
    assert!(over.iter().any(|c| c.contains("txt")), "{over:?}");
}

#[test]
fn fr14_overflow_row_names_dirs_and_files() {
    let fx = Fixture::new();
    for d in 0..3 {
        fx.sized(&format!("big/sub{d}/old.bin"), 10);
    }
    for i in 0..45 {
        fx.sized(&format!("big/f{}.txt", word(i)), 5000);
    }
    fx.mtime_all(60);
    for d in 0..3 {
        fx.mtime(&format!("big/sub{d}/old.bin"), 5 * 365 * 86_400);
        fx.mtime(&format!("big/sub{d}"), 5 * 365 * 86_400);
    }
    // Header, big/, 5 files, overflow, footer.
    let out = run(&fx, &["-n", "9"]);
    let over = out.must_row("… 3 dirs, 40 files");
    let sizes = 3 * 10 + 40 * 5000;
    assert!(
        cells(over).contains(&format!("{}K", sizes / 1024).as_str()),
        "{over} ({sizes} bytes)\n{}",
        out.stdout
    );
}

// FR-15: Joined directory chains

#[test]
fn fr15_single_child_chain_is_one_row() {
    let fx = Fixture::new();
    fx.file("src/main/java/com/acme/App.java", "class App {}\n");
    let out = run(&fx, &[]);
    let lines = out.lines();
    let i = lines
        .iter()
        .position(|l| strip_glyphs(l).starts_with("src/main/java/com/acme/ "))
        .unwrap_or_else(|| panic!("{}", out.stdout));
    assert!(strip_glyphs(lines[i + 1]).starts_with("App.java"));
    assert_eq!(depth(lines[i + 1]), depth(lines[i]) + 1);
}

#[test]
fn fr15_chain_stops_at_a_fork() {
    let fx = Fixture::new();
    fx.file("a/b/c/x.txt", "x\n").file("a/b/d/y.txt", "y\n");
    let out = run(&fx, &[]);
    let ab = out.must_row("a/b/");
    let c = out.must_row("c/");
    let d = out.must_row("d/");
    assert_eq!(depth(c), depth(ab) + 1, "{}", out.stdout);
    assert_eq!(depth(d), depth(ab) + 1, "{}", out.stdout);
}

// FR-16: Depth limit

fn five_deep() -> Fixture {
    let fx = Fixture::new();
    fx.file("r1.txt", "r\n")
        .file("l1/a.txt", "a\n")
        .file("l1/l2/b.txt", "b\n")
        .file("l1/l2/l3/c.txt", "c\n")
        .file("l1/l2/l3/l4/d.txt", "d\n")
        .file("l1/l2/l3/l4/l5/e.txt", "e\n");
    fx
}

#[test]
fn fr16_depth_limit_keeps_totals() {
    let fx = five_deep();
    let out = run(&fx, &["-L", "2"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let lines = out.lines();
    for l in &lines[1..lines.len() - 1] {
        assert!(depth(l) <= 2, "too deep: {l}\n{}", out.stdout);
    }
    assert!(out.header().contains("· 6 files"), "{}", out.header());
}

#[test]
fn fr16_depth_zero_is_header_summary_footer() {
    let fx = five_deep();
    let out = run(&fx, &["-L", "0"]);
    assert_eq!(out.lines().len(), 3, "{}", out.stdout);
    assert!(cells(out.lines()[1]).contains(&"6"), "{}", out.stdout);
}

// FR-17: Display order

fn order_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.sized("b.txt", 10_000)
        .sized("a.txt", 1000)
        .sized("c/inner.txt", 10);
    fx
}

fn names_at_depth_1(out: &support::Run) -> Vec<String> {
    out.lines()
        .into_iter()
        .filter(|l| depth(l) == 1)
        .map(|l| cells(l)[0].to_string())
        .collect()
}

#[test]
fn fr17_default_order_is_dirs_first_then_name() {
    let fx = order_fixture();
    let out = run(&fx, &[]);
    assert_eq!(
        names_at_depth_1(&out),
        ["c/", "a.txt", "b.txt"],
        "{}",
        out.stdout
    );
}

#[test]
fn fr17_sort_changes_order_not_selection() {
    let fx = order_fixture();
    let default = run(&fx, &[]);
    let by_size = run(&fx, &["--sort", "size"]);
    let names = names_at_depth_1(&by_size);
    let b = names.iter().position(|n| n == "b.txt").unwrap();
    let a = names.iter().position(|n| n == "a.txt").unwrap();
    assert!(b < a, "{}", by_size.stdout);
    let rows = |out: &support::Run| {
        let mut v: Vec<String> = out
            .lines()
            .into_iter()
            .map(|l| strip_glyphs(l).to_string())
            .collect();
        v.sort();
        v
    };
    assert_eq!(rows(&default), rows(&by_size));
}

#[test]
fn fr17_bad_sort_key_lists_the_valid_keys() {
    let fx = order_fixture();
    let out = run(&fx, &["--sort", "foo"]);
    assert_eq!(out.code, Some(2), "{out:?}");
    for key in ["salience", "name", "size", "age", "type"] {
        assert!(out.stderr.contains(key), "{key}: {}", out.stderr);
    }
}

// FR-19: Header

#[test]
fn fr19_header_shows_root_size_and_count() {
    let fx = Fixture::new();
    for i in 0..3 {
        fx.sized(&format!("fx/f{i}.bin"), 1024);
    }
    let out = run(&fx, &["fx"]);
    assert!(
        out.header().starts_with("fx/  3.0K · 3 files"),
        "{}",
        out.header()
    );
}

#[test]
fn fr19_p3_header_shows_kind_and_git() {
    let fx = Fixture::new();
    fx.file("Cargo.toml", "[package]\nname = \"x\"\n")
        .file("src/main.rs", "fn main() {}\n");
    support::git_init_commit(&fx);
    support::git_set_upstream(&fx);
    support::git_commits(&fx, "src/main.rs", 2);
    let out = run(&fx, &[]);
    assert!(out.header().contains("rust crate"), "{}", out.header());
    assert!(out.header().contains("git main ↑2"), "{}", out.header());
    // Clean: no changed count, and no `↓0`.
    assert!(!out.header().contains("changed"), "{}", out.header());
    assert!(!out.header().contains('↓'), "{}", out.header());
}

// FR-70: Exit codes

#[test]
fn fr70_readable_tree_exits_0() {
    let fx = Fixture::new();
    fx.file("a.txt", "a\n");
    assert_eq!(run(&fx, &[]).code, Some(0));
}

#[test]
fn fr70_unreadable_dir_exits_1() {
    if is_root() {
        eprintln!("skipped: root can read mode-000 directories");
        return;
    }
    let fx = Fixture::new();
    fx.file("a.txt", "a\n").dir("locked").mode("locked", 0o000);
    let code = run(&fx, &[]).code;
    fx.mode("locked", 0o755);
    assert_eq!(code, Some(1));
}

#[test]
fn fr70_unknown_flag_exits_2() {
    let fx = Fixture::new();
    assert_eq!(run(&fx, &["--bogus"]).code, Some(2));
}
