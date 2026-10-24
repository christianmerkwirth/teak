//! Boilerplate templates: the built-in table (FR-40) and user templates
//! (FR-41).

mod support;

use std::fs;
use std::path::Path;

use teak::chaff::ChaffClass;
use teak::chaff::templates::{Templates, fnv1a64, normalize};

use support::{Fixture, mit_license, node, run, scan, strip_glyphs};

fn class(s: &teak::Scan, rel: &str) -> Option<ChaffClass> {
    s.tags[node(s, rel) as usize].class
}

/// The chaff line at `depth`, if there is one.
fn chaff_line(out: &support::Run, depth: usize) -> Option<String> {
    out.lines()
        .into_iter()
        .find(|l| support::depth(l) == depth && strip_glyphs(l).starts_with("▪ chaff"))
        .map(String::from)
}

fn license_fixture(text: &str) -> Fixture {
    let fx = Fixture::new();
    fx.file("LICENSE", text)
        .file("src/main.rs", "fn main() {}\n")
        .file("README.md", "# x\n");
    fx
}

// FR-40: Built-in boilerplate templates

#[test]
fn fr40_the_mit_license_gets_a_label() {
    let fx = license_fixture(&mit_license(2026, "Christian Merkwirth"));
    let s = scan(fx.path(), &[]);
    assert_eq!(class(&s, "LICENSE"), Some(ChaffClass::Boilerplate));
    let out = run(&fx, &[]);
    let line = chaff_line(&out, 1).expect("a chaff line");
    assert!(line.contains("LICENSE (MIT)"), "{line}");
    assert!(out.row("LICENSE").is_none(), "{}", out.stdout);
    assert!(out.footer().contains("boilerplate 1"), "{}", out.footer());
}

#[test]
fn fr40_the_repo_license_gets_a_label() {
    let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("LICENSE"));
    let Ok(text) = text else {
        return; // Not every checkout has the file.
    };
    let fx = license_fixture(&text);
    let out = run(&fx, &[]);
    let line = chaff_line(&out, 1).expect("a chaff line");
    assert!(line.contains("LICENSE (MIT)"), "{line}");
}

#[test]
fn fr40_year_and_holder_do_not_matter() {
    let fx = license_fixture(&mit_license(1999, "Jane Q. Doe and Others"));
    let out = run(&fx, &[]);
    let line = chaff_line(&out, 1).expect("a chaff line");
    assert!(line.contains("LICENSE (MIT)"), "{line}");
}

#[test]
fn fr40_one_more_sentence_means_no_label() {
    let mut text = mit_license(2026, "Jane Doe");
    text.push_str("\nThis sentence is new.\n");
    let fx = license_fixture(&text);
    let s = scan(fx.path(), &[]);
    assert_eq!(class(&s, "LICENSE"), None);
    let out = run(&fx, &[]);
    out.must_row("LICENSE");
}

#[test]
fn fr40_every_builtin_template_matches_itself() {
    let builtin = Templates::builtin();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/boilerplate");
    let mut count = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let label = path.file_stem().unwrap().to_str().unwrap().to_string();
        let hash = fnv1a64(&normalize(&fs::read(&path).unwrap()));
        assert_eq!(
            builtin.lookup(hash),
            Some((label.as_str(), ChaffClass::Boilerplate)),
            "{label}"
        );
        count += 1;
    }
    assert_eq!(count, 13);
}

#[test]
fn fr40_normalize_and_hash() {
    assert_eq!(
        normalize(b"  Copyright (c) 2020 Jane\nHello \t World,\n\n 1999 TWICE\n"),
        b"hello world, twice"
    );
    assert_eq!(
        normalize(b"MIT License\r\n\r\nCOPYRIGHT 2001 X\r\n"),
        b"mit license"
    );
    // FNV-1a, 64 bit: fixed values.
    assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
}

// FR-41: User templates

fn user_templates() -> Fixture {
    let tpl = Fixture::new();
    tpl.file(
        "boilerplate/acme-header.txt",
        "ACME Corp internal code.\nDo not share outside ACME.\n",
    )
    .file("generated/stub.txt", "This is a stub. Replace it.\n");
    tpl
}

#[test]
fn fr41_user_templates_label_and_classify() {
    let tpl = user_templates();
    let fx = Fixture::new();
    fx.file(
        "src/HEADER",
        "acme corp   internal code.\n\n  do not share outside acme.",
    )
    .file("src/stub.txt", "This is a stub.\nReplace it.\n")
    .file("src/main.rs", "fn main() {}\n");
    let t = tpl.path().to_str().unwrap();
    let s = scan(fx.path(), &["--templates", t]);
    assert_eq!(class(&s, "src/HEADER"), Some(ChaffClass::Boilerplate));
    assert_eq!(class(&s, "src/stub.txt"), Some(ChaffClass::Generated));
    let out = run(&fx, &["--templates", t]);
    let line = chaff_line(&out, 2).expect("a chaff line in src/");
    assert!(line.contains("HEADER (acme-header)"), "{line}");
}

#[test]
fn fr41_templates_only_drops_the_builtin_table() {
    let tpl = user_templates();
    let fx = license_fixture(&mit_license(2026, "Jane Doe"));
    let t = tpl.path().to_str().unwrap();
    let out = run(&fx, &["--templates", t, "--templates-only"]);
    out.must_row("LICENSE");
    let out = run(&fx, &["--templates", t]);
    assert!(out.row("LICENSE").is_none(), "{}", out.stdout);
}

#[test]
fn fr41_dump_and_load_give_the_same_labels() {
    let out_dir = Fixture::new();
    let o = out_dir.path().to_str().unwrap();
    let dump = support::run_in(out_dir.path(), &["--dump-templates", o]);
    assert_eq!(dump.code, Some(0), "{dump:?}");
    assert!(out_dir.abs("boilerplate/MIT.txt").is_file());
    assert!(out_dir.abs("boilerplate/gitignore-Rust.txt").is_file());
    let fx = license_fixture(&mit_license(2026, "Jane Doe"));
    let covenant = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("templates/boilerplate/Contributor-Covenant-2.1.txt");
    fx.file("CODE_OF_CONDUCT.md", fs::read(covenant).unwrap());
    let builtin = run(&fx, &[]);
    let loaded = run(&fx, &["--templates", o, "--templates-only"]);
    assert_eq!(builtin.stdout, loaded.stdout);
    assert!(
        builtin.stdout.contains("LICENSE (MIT)"),
        "{}",
        builtin.stdout
    );
    assert!(
        builtin
            .stdout
            .contains("CODE_OF_CONDUCT.md (Contributor-Covenant-2.1)"),
        "{}",
        builtin.stdout
    );
}

#[test]
fn fr41_a_missing_template_dir_is_a_usage_error() {
    let fx = license_fixture("x\n");
    let out = run(&fx, &["--templates", "missing/"]);
    assert_eq!(out.code, Some(2), "{out:?}");
    assert!(out.stderr.contains("missing"), "{}", out.stderr);
    let out = run(&fx, &["--dump-templates", "missing/"]);
    assert_eq!(out.code, Some(2), "{out:?}");
}

#[test]
fn fr41_dump_never_overwrites() {
    let out_dir = Fixture::new();
    out_dir.file("boilerplate/MIT.txt", "mine\n");
    let o = out_dir.path().to_str().unwrap();
    let out = support::run_in(out_dir.path(), &["--dump-templates", o]);
    assert_eq!(out.code, Some(2), "{out:?}");
    assert!(out.stderr.contains("MIT.txt"), "{}", out.stderr);
    assert_eq!(
        fs::read_to_string(out_dir.abs("boilerplate/MIT.txt")).unwrap(),
        "mine\n"
    );
    assert!(!out_dir.abs("boilerplate/ISC.txt").exists());
}
