//! Property tests: the hard budget limit (FR-12) and every entry counted
//! (FR-21), on random trees with random flags (P1, plus `-l` and
//! `--focus`); and the same output for any thread count.
//!
//! Task-5 adds `md` and `json`.

mod support;

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use proptest::prelude::*;

use support::{Fixture, SCAN_TIME, gen_tree, opts, run_in, scan};

/// splitmix64, as in `gen_tree`.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

/// Names with chaff, markers, dotfiles, and series.
const NAMES: &[&str] = &[
    "src",
    "lib",
    "docs",
    "data",
    "node_modules",
    "target",
    "dist",
    "build",
    "vendor",
    "__pycache__",
    ".git",
    ".vscode",
    ".cache",
    ".env",
    ".DS_Store",
    ".gitkeep",
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "README.md",
    "main.rs",
    "app.min.js",
    "api.pb.go",
    "build.log",
    "notes.txt",
    "photo.jpg",
    "id_rsa",
];

/// Writes a random tree with `n` entries below `root`.
fn random_tree(root: &Path, seed: u64, n: usize) {
    let mut rng = Rng(seed);
    let mut dirs = vec![root.to_path_buf()];
    let mut all = Vec::new();
    for i in 0..n {
        let parent = dirs[rng.below(dirs.len())].clone();
        if parent.components().count() - root.components().count() > 8 {
            continue;
        }
        let name = match rng.below(10) {
            0..=3 => rng.pick(NAMES).to_string(),
            4 => format!("IMG_{:04}.png", rng.below(40)),
            5 => format!("part-{:05}.csv", rng.below(200)),
            6 => format!("{:016x}{:08x}", rng.next(), i),
            _ => format!("n{}", support::word(rng.below(400))),
        };
        let path = parent.join(&name);
        if fs::symlink_metadata(&path).is_ok() {
            continue;
        }
        match rng.below(12) {
            0..=2 => {
                fs::create_dir(&path).unwrap();
                dirs.push(path.clone());
            }
            3 if !all.is_empty() => {
                let target: &std::path::PathBuf = &all[rng.below(all.len())];
                symlink(target, &path).unwrap();
            }
            4 => symlink("nowhere", &path).unwrap(),
            5 if name == ".gitignore" || rng.below(4) == 0 => {
                let rules = ["*.log\n", "data/\n", "build\n", "!keep.log\n", "n*\n"];
                fs::write(parent.join(".gitignore"), rng.pick(&rules)).unwrap();
            }
            _ => {
                let len = match rng.below(20) {
                    0 => 0,
                    1 => 1 << 30,
                    k => (k as u64) * 37 + rng.next() % 5000,
                };
                let f = fs::File::create(&path).unwrap();
                f.set_len(len).unwrap();
            }
        }
        all.push(path);
    }
}

/// Non-directory entries below `root`, without following symlinks.
fn count_files(root: &Path) -> u64 {
    let mut n = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if fs::symlink_metadata(&p).unwrap().is_dir() {
                stack.push(p);
            } else {
                n += 1;
            }
        }
    }
    n
}

fn flag_sets() -> impl Strategy<Value = Vec<&'static str>> {
    let hidden = prop_oneof![Just(vec![]), Just(vec!["-A"]), Just(vec!["-a"])];
    let sort = prop_oneof![
        Just(vec![]),
        Just(vec!["--sort", "salience"]),
        Just(vec!["--sort", "name"]),
        Just(vec!["--sort", "size"]),
        Just(vec!["--sort", "age"]),
        Just(vec!["--sort", "type"]),
    ];
    let chaff = prop_oneof![
        Just(vec![]),
        Just(vec!["--chaff", "show"]),
        Just(vec!["--chaff", "dim"]),
        Just(vec!["--chaff", "fold"]),
        Just(vec!["--chaff", "hide"]),
        Just(vec!["--show=lock", "--hide=vendor"]),
    ];
    let depth = prop_oneof![Just(vec![]), Just(vec!["-L", "0"]), Just(vec!["-L", "2"])];
    let long = prop_oneof![Just(vec![]), Just(vec!["-l"])];
    (hidden, sort, chaff, depth, long).prop_map(|(a, b, c, d, e)| [a, b, c, d, e].concat())
}

/// The directories below `root` (with `root`), in path order.
fn dirs_of(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![root.to_path_buf()];
    let mut i = 0;
    while i < out.len() {
        let mut subs: Vec<_> = fs::read_dir(&out[i])
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| fs::symlink_metadata(p).unwrap().is_dir())
            .collect();
        subs.sort();
        out.extend(subs);
        i += 1;
    }
    out
}

fn sizes() -> impl Strategy<Value = usize> {
    prop_oneof![6 => 0usize..200, 3 => 200usize..2000, 1 => 2000usize..20_000]
}

proptest! {
    // PROPTEST_CASES raises the count for a longer run.
    #![proptest_config(ProptestConfig {
        cases: std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(40),
        ..ProptestConfig::default()
    })]

    #[test]
    fn fr12_and_fr21_hold_for_random_trees(
        seed in any::<u64>(),
        n in sizes(),
        budget in 3usize..=300,
        flags in flag_sets(),
        focus in any::<Option<prop::sample::Index>>(),
    ) {
        let fx = Fixture::new();
        random_tree(fx.path(), seed, n);
        let n_text = budget.to_string();
        let mut args = vec!["-n", n_text.as_str()];
        args.extend(&flags);
        // `--focus` on a random directory; the library resolves it against
        // the current directory, so the path is absolute.
        let focus_text = focus.map(|i| {
            let dirs = dirs_of(fx.path());
            dirs[i.index(dirs.len())].to_string_lossy().into_owned()
        });
        if let Some(f) = &focus_text {
            args.extend(["--focus", f.as_str()]);
        }
        let s = scan(fx.path(), &args);

        // FR-12: the view and every text layout fit the budget.
        prop_assert!(s.view.rows.len() <= budget, "{} rows for {budget}", s.view.rows.len());
        let o = opts(&args);
        for (width, ascii, color) in [(120, false, false), (80, false, true), (40, true, false)] {
            let env = teak::Env::fixed(SCAN_TIME as i64);
            let mut set = teak::settings(&o, &env);
            set.width = width;
            if ascii {
                set.glyphs = &teak::layout::ASCII;
            }
            let lines = teak::layout::fit(&s, &teak::layout_opts(&o, &set));
            prop_assert!(lines.len() <= budget);
            let mut out = Vec::new();
            let style = teak::style::Style::new(color, None);
            teak::render::write(&lines, &s.tree, &style, &mut out).unwrap();
            let text = String::from_utf8(out).unwrap();
            prop_assert!(text.lines().count() <= budget);
            for line in support::strip_ansi(&text).lines() {
                prop_assert!(support::width(line) <= width, "{} > {width}: {line}", support::width(line));
            }
        }

        // FR-21: visible + summed + hidden = an independent walk.
        let shown: u64 = s.view.rows.iter().map(|r| r.files(&s.tree, &s.aggs)).sum();
        prop_assert_eq!(shown + s.view.hidden_files(), count_files(fx.path()));
    }
}

#[test]
fn random_trees_reach_the_large_sizes() {
    let fx = Fixture::new();
    random_tree(fx.path(), 1, 20_000);
    let n = count_files(fx.path());
    assert!(n > 10_000, "{n} files");
}

#[test]
fn same_output_for_any_thread_count() {
    let fx = Fixture::new();
    gen_tree::generate(&gen_tree::Opts {
        files: 3000,
        seed: 5,
        out: fx.path().join("t"),
        ..gen_tree::Opts::default()
    })
    .unwrap();
    let one = run_in(fx.path(), &["t", "-j", "1"]);
    let eight = run_in(fx.path(), &["t", "-j", "8"]);
    assert_eq!(one.code, Some(0));
    assert_eq!(one.stdout, eight.stdout);
    let again = run_in(fx.path(), &["t", "-j", "8"]);
    assert_eq!(eight.stdout, again.stdout);
}
