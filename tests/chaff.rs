//! Chaff: classes, rule order, the chaff line, and its controls (FR-20,
//! FR-30 to FR-37).

mod support;

use teak::chaff::{Action, ChaffClass};
use teak::tree::ROOT;

use support::{Fixture, cells, chaff_items, depth, node, run, scan, strip_glyphs};

/// The chaff line of the root, as text.
fn chaff_line(out: &support::Run) -> Option<String> {
    out.lines()
        .into_iter()
        .find(|l| depth(l) == 1 && strip_glyphs(l).starts_with("▪ chaff"))
        .map(String::from)
}

// FR-30: Chaff classes

fn class_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file("node_modules/left-pad/index.js", "module.exports = 1;\n")
        .file("Cargo.toml", "[package]\nname = \"x\"\n")
        .file("target/debug/x", "bin")
        .file("__pycache__/m.cpython-312.pyc", "pyc")
        .file(".git/HEAD", "ref: refs/heads/main\n")
        .file(".DS_Store", "ds")
        .file("Cargo.lock", "# lock\n")
        .file("api.pb.go", "package api\n")
        .file("app.min.js", "var a=1;\n")
        .file(".editorconfig", "root = true\n")
        .file(".gitkeep", "")
        .file("LICENSE", support::mit_license(2026, "Jane Doe"))
        .file("src/main.rs", "fn main() {}\n");
    fx
}

#[test]
fn fr30_every_class_has_its_entry_and_action() {
    let fx = class_fixture();
    let s = scan(fx.path(), &[]);
    let want = [
        ("node_modules", ChaffClass::Vendor),
        ("target", ChaffClass::Build),
        ("__pycache__", ChaffClass::Cache),
        (".git", ChaffClass::Vcs),
        (".DS_Store", ChaffClass::Editor),
        ("Cargo.lock", ChaffClass::Lock),
        ("api.pb.go", ChaffClass::Generated),
        ("app.min.js", ChaffClass::Minified),
        (".editorconfig", ChaffClass::Boilerplate),
        ("LICENSE", ChaffClass::Boilerplate),
        (".gitkeep", ChaffClass::Empty),
    ];
    for (rel, class) in want {
        let tag = s.tags[node(&s, rel) as usize];
        assert_eq!(tag.class, Some(class), "{rel}");
    }
    assert_eq!(s.tags[node(&s, "src/main.rs") as usize].class, None);
    let items = chaff_items(&s, ROOT).expect("a root chaff line");
    for rel in [
        "node_modules",
        "target",
        "__pycache__",
        "Cargo.lock",
        "api.pb.go",
        "app.min.js",
        ".editorconfig",
        "LICENSE",
        ".gitkeep",
    ] {
        assert!(items.contains(&node(&s, rel)), "{rel} in the chaff line");
    }
    for rel in [".git", ".DS_Store"] {
        let id = node(&s, rel);
        assert!(!items.contains(&id), "{rel} not in the chaff line");
        assert_eq!(s.tags[id as usize].action, Action::Hide, "{rel}");
    }
    let out = run(&fx, &[]);
    assert!(out.row(".git/").is_none(), "{}", out.stdout);
    let footer = out.footer();
    assert!(footer.contains("vcs 1"), "{footer}");
    assert!(footer.contains("editor 1"), "{footer}");
}

// FR-31: Rule order

#[test]
fn fr31_gitattributes_marks_generated() {
    let fx = Fixture::new();
    fx.file(".gitattributes", "docs/api.md linguist-generated\n")
        .file("docs/api.md", "# API\n")
        .file("docs/guide.md", "# Guide\n");
    let s = scan(fx.path(), &[]);
    let api = node(&s, "docs/api.md");
    assert_eq!(s.tags[api as usize].class, Some(ChaffClass::Generated));
    assert_eq!(s.tags[node(&s, "docs/guide.md") as usize].class, None);
}

#[test]
fn fr31_the_user_wins() {
    let fx = Fixture::new();
    fx.file(".gitattributes", "docs/api.md linguist-generated\n")
        .file("docs/api.md", "# API\n")
        .file("docs/guide.md", "# Guide\n");
    let out = run(&fx, &["--show=generated"]);
    out.must_row("api.md");
    let out = run(&fx, &[]);
    assert!(out.row("api.md").is_none(), "{}", out.stdout);
}

#[test]
fn fr31_negation_keeps_vendor_normal() {
    let fx = Fixture::new();
    fx.file(".gitattributes", "vendor/** -linguist-vendored\n")
        .file("vendor/lib/a.c", "int a;\n")
        .file("main.c", "int main;\n");
    let s = scan(fx.path(), &[]);
    let vendor = node(&s, "vendor");
    assert_eq!(s.tags[vendor as usize].class, None);
    let out = run(&fx, &[]);
    out.must_row("vendor/lib/");
}

// FR-32: Project-kind rules in context

#[test]
fn fr32_pack_rules_need_their_marker() {
    let fx = Fixture::new();
    fx.file("app/Cargo.toml", "[package]\n")
        .file("app/target/out.bin", "b")
        .file("photos/target/p.jpg", "j")
        .file("web/package.json", "{}\n")
        .file("web/dist/app.js", "a");
    let s = scan(fx.path(), &[]);
    let class = |rel: &str| s.tags[node(&s, rel) as usize].class;
    assert_eq!(class("app/target"), Some(ChaffClass::Build));
    assert_eq!(class("photos/target"), None);
    assert_eq!(class("web/dist"), Some(ChaffClass::Build));
}

// FR-33: One chaff line

fn big_chaff_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file("Cargo.toml", "[package]\n")
        .sized("node_modules/pkg/blob.bin", 180 << 20)
        .sized("target/release/app", 1229 << 20)
        .file("Cargo.lock", "# lock\n")
        .file("LICENSE", support::mit_license(2026, "Jane Doe"))
        .file("src/main.rs", "fn main() {}\n");
    for e in ["e1", "e2", "e3", "e4"] {
        fx.file(e, "");
    }
    fx
}

#[test]
fn fr33_one_chaff_line_large_first_then_names_then_more() {
    let fx = big_chaff_fixture();
    let out = run(&fx, &[]);
    let chaff: Vec<_> = out
        .lines()
        .into_iter()
        .filter(|l| strip_glyphs(l).starts_with("▪ chaff"))
        .collect();
    assert_eq!(chaff.len(), 1, "{}", out.stdout);
    let line = chaff[0];
    let t = line.find("target/ 1.2G").expect(line);
    let n = line.find("node_modules/ 180M").expect(line);
    assert!(t < n, "{line}");
    assert!(line.contains("Cargo.lock"), "{line}");
    assert!(line.contains("LICENSE (MIT)"), "{line}");
    assert!(line.ends_with("+4"), "{line}");
}

#[test]
fn fr33_items_that_do_not_fit_go_into_more() {
    let fx = Fixture::new();
    fx.file("main.rs", "fn main() {}\n");
    for i in 0..30 {
        fx.file(&format!("lib{i:02}.min.js"), "var a=1;\n");
    }
    let out = run(&fx, &["--width", "60"]);
    let line = chaff_line(&out).expect("a chaff line");
    assert!(support::width(&line) <= 60, "{line}");
    let shown = line.matches(".min.js").count();
    let more: usize = line
        .rsplit('+')
        .next()
        .and_then(|n| n.parse().ok())
        .expect("ends with +N");
    assert_eq!(shown + more, 30, "{line}");
}

// FR-34: Large chaff first

#[test]
fn fr34_large_chaff_line_is_the_first_child() {
    let fx = Fixture::new();
    fx.file("Cargo.toml", "[package]\n")
        .sized("target/release/app", 1229 << 20)
        .sized("src/big.rs", 14 << 20);
    let out = run(&fx, &[]);
    let lines = out.lines();
    assert!(
        strip_glyphs(lines[1]).starts_with("▪ chaff"),
        "{}",
        out.stdout
    );
}

#[test]
fn fr34_small_chaff_line_is_the_last_child() {
    let fx = Fixture::new();
    fx.file("Cargo.toml", "[package]\n")
        .sized("target/release/app", 10 << 20)
        .sized("src/big.rs", 14 << 20);
    let out = run(&fx, &[]);
    let lines = out.lines();
    let last_child = lines[lines.len() - 2];
    assert!(
        strip_glyphs(last_child).starts_with("▪ chaff"),
        "{}",
        out.stdout
    );
}

// FR-35: -A and -a

fn hidden_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file(".env", "API_KEY=x\n")
        .file(".git/HEAD", "ref: refs/heads/main\n")
        .sized(".git/objects/pack/p.pack", 10 << 20)
        .file(".vscode/settings.json", "{}\n")
        .file(".gitignore", "build.log\n")
        .file("build.log", "log\n")
        .file("main.rs", "fn main() {}\n");
    fx
}

#[test]
fn fr35_hidden_entries_are_only_counted() {
    let fx = hidden_fixture();
    let out = run(&fx, &[]);
    for name in [".env", ".git/", ".vscode/"] {
        assert!(out.row(name).is_none(), "{name}:\n{}", out.stdout);
    }
    let line = chaff_line(&out).expect("build.log folds");
    for name in [".env", ".git/", ".vscode/"] {
        assert!(!line.contains(name), "{name}: {line}");
    }
    assert!(line.contains("build.log"), "{line}");
    assert!(out.footer().contains("hidden 4"), "{}", out.footer());
}

#[test]
fn fr35_almost_all_folds_hidden_entries() {
    let fx = hidden_fixture();
    let out = run(&fx, &["-A"]);
    let line = chaff_line(&out).expect("a chaff line");
    for name in [".env", ".git/ 10M", ".vscode/"] {
        assert!(line.contains(name), "{name}: {line}");
    }
}

#[test]
fn fr35_all_shows_everything_as_rows() {
    let fx = hidden_fixture();
    let out = run(&fx, &["-a"]);
    for name in [".env", ".git/", ".vscode/", "build.log"] {
        out.must_row(name);
    }
    assert!(chaff_line(&out).is_none(), "{}", out.stdout);
}

// FR-36: Chaff controls

fn controls_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file("node_modules/a/index.js", "x\n")
        .file("Cargo.lock", "# lock\n")
        .file("app.min.js", "var a=1;\n")
        .file("main.rs", "fn main() {}\n");
    fx
}

#[test]
fn fr36_chaff_show_makes_normal_rows() {
    let fx = controls_fixture();
    let out = run(&fx, &["--chaff", "show"]);
    assert!(chaff_line(&out).is_none(), "{}", out.stdout);
    for name in ["node_modules/a/", "Cargo.lock", "app.min.js"] {
        out.must_row(name);
    }
}

#[test]
fn fr36_show_one_class() {
    let fx = controls_fixture();
    let out = run(&fx, &["--show=lock"]);
    out.must_row("Cargo.lock");
    let line = chaff_line(&out).expect("the rest stays folded");
    assert!(line.contains("node_modules/"), "{line}");
    assert!(line.contains("app.min.js"), "{line}");
}

#[test]
fn fr36_hide_one_class() {
    let fx = controls_fixture();
    let out = run(&fx, &["--hide=vendor"]);
    let line = chaff_line(&out).expect("a chaff line");
    assert!(!line.contains("node_modules"), "{line}");
    assert!(out.footer().contains("vendor 1"), "{}", out.footer());
}

#[test]
fn fr36_bad_class_lists_the_valid_classes() {
    let fx = controls_fixture();
    let out = run(&fx, &["--show=foo"]);
    assert_eq!(out.code, Some(2), "{out:?}");
    for class in [
        "vendor",
        "build",
        "cache",
        "vcs",
        "editor",
        "lock",
        "generated",
        "minified",
        "boilerplate",
        "empty",
        "duplicate",
    ] {
        assert!(out.stderr.contains(class), "{class}: {}", out.stderr);
    }
}

#[test]
fn fr36_the_later_flag_wins() {
    let fx = controls_fixture();
    let out = run(&fx, &["--show=lock", "--hide=lock"]);
    assert!(out.row("Cargo.lock").is_none(), "{}", out.stdout);
    let line = chaff_line(&out).expect("a chaff line");
    assert!(!line.contains("Cargo.lock"), "{line}");
    let out = run(&fx, &["--hide=lock", "--show=lock"]);
    out.must_row("Cargo.lock");
}

// FR-37: Gitignore

fn ignore_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file(".gitignore", "data/\n")
        .sized("data/huge.bin", 40 << 30)
        .file("src/main.rs", "fn main() {}\n");
    fx
}

#[test]
fn fr37_gitignored_entries_fold_and_large_ones_come_first() {
    let fx = ignore_fixture();
    let out = run(&fx, &[]);
    let lines = out.lines();
    let first = strip_glyphs(lines[1]);
    assert!(first.starts_with("▪ chaff"), "{}", out.stdout);
    assert!(first.contains("data/ 40G"), "{first}");
}

#[test]
fn fr37_no_gitignore_shows_the_entry() {
    let fx = ignore_fixture();
    let out = run(&fx, &["--no-gitignore"]);
    assert!(
        cells(out.must_row("data/")).contains(&"40G"),
        "{}",
        out.stdout
    );
}

#[test]
fn fr37_gitignore_applies_outside_a_repo() {
    let fx = ignore_fixture();
    assert!(!fx.abs(".git").exists());
    let s = scan(fx.path(), &[]);
    assert_eq!(
        s.tags[node(&s, "data") as usize].class,
        Some(ChaffClass::Ignored)
    );
}

// FR-20: Footer

#[test]
fn fr20_footer_counts_lines_files_and_folded_classes() {
    let fx = Fixture::new();
    for i in 0..120 {
        fx.file(&format!("node_modules/p{}/f{i}.js", i % 7), "x\n");
    }
    for i in 0..30 {
        fx.file(&format!("src/f{i:02}.rs"), "fn f() {}\n");
    }
    let out = run(&fx, &[]);
    let footer = out.footer();
    for part in ["of 100 lines", "150 files", "vendor 120"] {
        assert!(footer.contains(part), "{part}: {footer}");
    }
}
