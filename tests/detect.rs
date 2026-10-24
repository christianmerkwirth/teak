//! Content detection: Magika on visible files (FR-49), detection modes
//! (FR-52), mismatches (FR-53), builds without Magika (FR-55), and the
//! detection corpus in `tests/corpus/`.

mod support;

use teak::detect::{self, Source, label_id};

use support::{Fixture, corpus, node, run, scan};

/// The type cell of a file row: name, size, type, age.
#[cfg_attr(not(any(feature = "magika", feature = "test-hooks")), allow(dead_code))]
fn type_cell(out: &support::Run, name: &str) -> String {
    out.cells(name)[2].to_string()
}

// FR-49: Magika on visible files

#[cfg(feature = "magika")]
#[test]
fn fr49_magika_types_a_file_with_no_extension() {
    let fx = Fixture::new();
    fx.file("script", corpus("tool"));
    let out = run(&fx, &[]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert_eq!(type_cell(&out, "script"), "python", "{}", out.stdout);
    let s = scan(fx.path(), &[]);
    let info = s.type_of(node(&s, "script"));
    assert_eq!(info.source, Source::Magika, "{info:?}");
}

#[cfg(feature = "magika")]
#[test]
fn fr49_low_magika_scores_keep_the_extension() {
    // Magika gives a short Rust file a low score; the row keeps `rust`.
    let fx = Fixture::new();
    fx.file("a.rs", "fn main() {}\n")
        .file("config.js", corpus("config.js"));
    let out = run(&fx, &[]);
    assert_eq!(type_cell(&out, "a.rs"), "rust", "{}", out.stdout);
    assert_eq!(type_cell(&out, "config.js"), "javascript", "{}", out.stdout);
}

// FR-52: Detection modes

fn modes_fixture() -> Fixture {
    let fx = Fixture::new();
    for name in ["tool", "report.pdf", "photo.jpg", "icon.dat", "README.md"] {
        fx.file(name, corpus(name));
    }
    fx
}

#[cfg(feature = "test-hooks")]
#[test]
fn fr52_detect_off_reads_nothing() {
    let fx = modes_fixture();
    let (out, report) = support::run_report(fx.path(), &["--detect", "off"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(report.reads.is_empty(), "{:?}", report.reads);
    assert_eq!(type_cell(&out, "report.pdf"), "pdf", "{}", out.stdout);
    assert_eq!(type_cell(&out, "tool"), "unknown", "{}", out.stdout);
    assert_eq!(type_cell(&out, "README.md"), "markdown", "{}", out.stdout);
}

#[test]
fn fr52_detect_fast_uses_no_magika() {
    let fx = modes_fixture();
    let s = scan(fx.path(), &["--detect", "fast"]);
    for (id, info) in &s.types {
        assert_ne!(info.source, Source::Magika, "{:?}", s.tree.rel_path(*id));
    }
    // Magic bytes still type the PNG with the wrong extension.
    assert_eq!(s.type_of(node(&s, "icon.dat")).name(), "png");
}

#[test]
fn fr52_unknown_mode_is_a_usage_error() {
    let fx = modes_fixture();
    let out = run(&fx, &["--detect", "max"]);
    assert_eq!(out.code, Some(2), "{out:?}");
    assert!(out.stdout.is_empty(), "{}", out.stdout);
}

// FR-53: Mismatches

#[cfg(feature = "magika")]
#[test]
fn fr53_html_in_a_pdf_gets_a_mark() {
    let fx = Fixture::new();
    fx.file("report.pdf", corpus("report.pdf"))
        .file("main.rs", "fn main() {}\n");
    let out = run(&fx, &[]);
    assert_eq!(
        type_cell(&out, "report.pdf"),
        "html ⚠ ext says pdf",
        "{}",
        out.stdout
    );
    assert_eq!(type_cell(&out, "main.rs"), "rust", "{}", out.stdout);
}

#[cfg(feature = "magika")]
#[test]
fn fr53_mismatches_lists_only_mismatched_files() {
    let fx = Fixture::new();
    fx.file("data/report.pdf", corpus("report.pdf"))
        .file("data/paper.pdf", corpus("paper.pdf"))
        .file("src/main.rs", "fn main() {}\n")
        .file("README.md", corpus("README.md"));
    let out = run(&fx, &["--mismatches"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let lines = out.lines();
    assert_eq!(lines.len(), 3, "{}", out.stdout);
    assert_eq!(
        lines[1], "data/report.pdf  html ⚠ ext says pdf",
        "{}",
        out.stdout
    );
    assert!(lines[2].starts_with("──"), "{}", out.stdout);
}

#[cfg(feature = "magika")]
#[test]
fn fr53_mismatches_are_sorted_by_path_and_fit_the_budget() {
    let fx = Fixture::new();
    for name in ["f", "b", "d", "a", "e", "c"] {
        fx.file(&format!("{name}.pdf"), corpus("report.pdf"));
    }
    let out = run(&fx, &["--mismatches", "-n", "5"]);
    let lines = out.lines();
    assert_eq!(lines.len(), 5, "{}", out.stdout);
    assert!(lines[1].starts_with("a.pdf  "), "{}", out.stdout);
    assert!(lines[2].starts_with("b.pdf  "), "{}", out.stdout);
    assert_eq!(lines[3], "… 4 more", "{}", out.stdout);
}

#[test]
fn fr53_no_mismatches_says_so() {
    let fx = Fixture::new();
    fx.file("main.rs", "fn main() {}\n")
        .file("README.md", corpus("README.md"));
    let out = run(&fx, &["--mismatches"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let lines = out.lines();
    assert_eq!(lines.len(), 3, "{}", out.stdout);
    assert_eq!(lines[1], "no mismatches", "{}", out.stdout);
}

#[cfg(feature = "magika")]
#[test]
fn fr53_equal_names_unknown_extensions_and_low_scores_get_no_mark() {
    let fx = Fixture::new();
    let blob: Vec<u8> = (0..3000u32).map(|i| (i * 7919 % 251) as u8).collect();
    fx.file("a.yml", corpus("ci.yml"))
        .file("b.jpg", corpus("photo.jpg"))
        .file("blob.dat", blob)
        .file("a.rs", "fn main() {}\n");
    let out = run(&fx, &[]);
    assert!(!out.stdout.contains('⚠'), "{}", out.stdout);
    assert_eq!(type_cell(&out, "a.yml"), "yaml", "{}", out.stdout);
    assert_eq!(type_cell(&out, "b.jpg"), "jpeg", "{}", out.stdout);
}

#[test]
fn fr53_the_mismatch_rule() {
    let (html, pdf) = (label_id("html"), label_id("pdf"));
    assert_eq!(detect::mismatch(html, 0.95, b"report.pdf"), Some(pdf));
    // A low score, no known extension, or the same type: no mark.
    assert_eq!(detect::mismatch(html, 0.5, b"report.pdf"), None);
    assert_eq!(detect::mismatch(html, 0.99, b"report.dat"), None);
    assert_eq!(detect::mismatch(html, 0.99, b"index.htm"), None);
    assert_eq!(detect::mismatch(label_id("yaml"), 1.0, b"a.yml"), None);
    assert_eq!(detect::mismatch(label_id("jpeg"), 1.0, b"b.jpg"), None);
    assert_eq!(detect::mismatch(label_id("unknown"), 1.0, b"x.pdf"), None);
}

// FR-55: Build without Magika

#[cfg(not(feature = "magika"))]
#[test]
fn fr55_full_acts_as_fast_without_magika() {
    let fx = modes_fixture();
    let full = run(&fx, &["--detect", "full"]);
    assert_eq!(full.code, Some(0), "{full:?}");
    let fast = run(&fx, &["--detect", "fast"]);
    assert_eq!(full.stdout, fast.stdout);
    let version = support::teak(&fx, &["--version"]);
    assert!(String::from_utf8_lossy(&version.stdout).contains("magika: off"));
}

// The detection corpus

#[cfg(feature = "magika")]
#[test]
fn corpus_files_get_their_expected_labels() {
    use support::corpus_dir;
    use teak::detect::label;
    let s = scan(&corpus_dir(), &[]);
    let manifest = std::fs::read_to_string(corpus_dir().join("EXPECTED.tsv")).unwrap();
    let mut wrong = Vec::new();
    for line in manifest.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        let info = s.type_of(node(&s, f[0]));
        let mismatch = info.mismatch.map_or("-", |l| label(l).name);
        if (info.name(), mismatch) != (f[1], f[2]) {
            wrong.push(format!(
                "{}: got {} / {mismatch}, want {} / {}",
                f[0],
                info.name(),
                f[1],
                f[2]
            ));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}
