//! `--why PATH` (FR-60): the rule, the action, the salience terms, and the
//! decision.

mod support;

use support::{Fixture, run, word};

fn project() -> Fixture {
    let fx = Fixture::new();
    fx.file("src/main.rs", "fn main() {}\n")
        .file("package.json", "{}\n")
        .file(".gitignore", "*.log\n")
        .file("build.log", "log\n")
        .file(".DS_Store", "x");
    for i in 0..20 {
        fx.file(&format!("node_modules/m{i}/index.js"), "x\n");
    }
    for i in 0..60 {
        fx.sized(&format!("docs/{}.md", word(i)), 100);
    }
    fx
}

fn line<'a>(out: &'a support::Run, key: &str) -> &'a str {
    out.lines()
        .into_iter()
        .find(|l| l.starts_with(key))
        .unwrap_or_else(|| panic!("no {key} line in:\n{}", out.stdout))
}

const TERMS: &[&str] = &["size", "recency", "git", "rarity", "role", "anomaly"];

#[test]
fn fr60_why_a_folded_vendor_directory() {
    let fx = project();
    let out = run(&fx, &["--why", "node_modules"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert_eq!(line(&out, "path:"), "path: node_modules/");
    let class = line(&out, "class:");
    assert!(class.contains("vendor"), "{class}");
    assert!(class.contains("global"), "{class}");
    assert!(class.contains("node_modules/"), "{class}");
    assert_eq!(line(&out, "action:"), "action: fold");
    let salience = line(&out, "salience:");
    for t in TERMS {
        assert!(salience.contains(t), "{t}: {salience}");
    }
    assert!(salience.contains("= "), "{salience}");
    let decision = line(&out, "decision:");
    assert!(decision.contains("folded into"), "{decision}");
    assert!(!out.stdout.contains("├──"), "no tree:\n{}", out.stdout);
}

#[test]
fn fr60_why_a_visible_file() {
    let fx = project();
    let out = run(&fx, &["--why", "src/main.rs"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert_eq!(line(&out, "class:"), "class: none");
    assert_eq!(line(&out, "action:"), "action: show");
    assert_eq!(line(&out, "decision:"), "decision: visible");
    let salience = line(&out, "salience:");
    for t in TERMS {
        assert!(salience.contains(t), "{t}: {salience}");
    }
}

#[test]
fn fr60_why_names_the_gitignore_rule_and_overflow_and_hidden() {
    let fx = project();
    let out = run(&fx, &["--why", "build.log"]);
    let class = line(&out, "class:");
    assert!(class.contains("ignored"), "{class}");
    assert!(class.contains("*.log"), "{class}");
    assert!(class.contains(".gitignore"), "{class}");
    let out = run(&fx, &["--why", ".DS_Store"]);
    assert_eq!(
        line(&out, "decision:"),
        "decision: hidden, counted in footer"
    );
    let out = run(
        &fx,
        &["--why", &format!("docs/{}.md", word(59)), "-n", "12"],
    );
    let decision = line(&out, "decision:");
    assert!(
        decision == "decision: in overflow of docs/" || decision == "decision: folded into docs/",
        "{decision}"
    );
}

#[test]
fn fr60_why_inside_a_folded_directory() {
    let fx = project();
    let out = run(&fx, &["--why", "node_modules/m3/index.js"]);
    let class = line(&out, "class:");
    assert!(class.contains("vendor"), "{class}");
    assert!(class.contains("inside node_modules/"), "{class}");
    assert!(
        line(&out, "decision:").contains("folded into"),
        "{}",
        out.stdout
    );
}

#[test]
fn fr60_why_outside_the_root_is_a_usage_error() {
    let fx = project();
    for bad in ["/etc/passwd", "nope.txt"] {
        let out = run(&fx, &["--why", bad]);
        assert_eq!(out.code, Some(2), "--why {bad}: {out:?}");
        assert!(out.stderr.contains("--why"), "{}", out.stderr);
        assert!(out.stdout.is_empty(), "{}", out.stdout);
    }
}
