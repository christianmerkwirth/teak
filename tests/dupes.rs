//! Duplicates (FR-43): `--dupes` folds copies of visible files and marks
//! them `= original`.

mod support;

use support::{Fixture, run};

/// 200 KiB of bytes that depend on `seed`.
fn blob(seed: u32) -> Vec<u8> {
    (0..200 * 1024u32)
        .map(|i| (i.wrapping_mul(2_654_435_761).wrapping_add(seed) >> 13) as u8)
        .collect()
}

fn dupes_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file("a.bin", blob(1))
        .file("copy/b.bin", blob(1))
        .file("copy/keep.txt", "keep\n")
        .file("c.bin", blob(2))
        .file("e1", "")
        .file("e2", "");
    fx
}

#[test]
fn fr43_a_copy_is_folded_and_marked() {
    let fx = dupes_fixture();
    let out = run(&fx, &["--dupes"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(
        out.row("b.bin").is_none(),
        "b.bin has no row:\n{}",
        out.stdout
    );
    let chaff = out
        .lines()
        .into_iter()
        .find(|l| l.contains("▪ chaff") && l.contains("b.bin"))
        .unwrap_or_else(|| panic!("no chaff line with b.bin:\n{}", out.stdout));
    assert!(chaff.contains("b.bin = a.bin"), "{chaff}");
    let c = out.must_row("c.bin");
    assert!(!c.contains('='), "{c}");
    assert!(out.row("a.bin").is_some(), "{}", out.stdout);
    assert!(out.footer().contains("duplicate 1"), "{}", out.footer());
}

#[test]
fn fr43_why_names_the_original() {
    let fx = dupes_fixture();
    let out = run(&fx, &["--dupes", "--why", "copy/b.bin"]);
    assert!(
        out.stdout.contains("class: duplicate (= a.bin)"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("decision: folded into"),
        "{}",
        out.stdout
    );
}

#[test]
fn fr43_empty_files_are_empty_not_duplicates() {
    let fx = dupes_fixture();
    for name in ["e1", "e2"] {
        let out = run(&fx, &["--dupes", "--why", name]);
        assert!(out.stdout.contains("class: empty"), "{}", out.stdout);
    }
}

#[test]
fn fr43_without_dupes_nothing_is_marked() {
    let fx = dupes_fixture();
    let out = run(&fx, &[]);
    assert!(out.row("b.bin").is_some(), "{}", out.stdout);
    assert!(!out.stdout.contains(" = a.bin"), "{}", out.stdout);
}

#[cfg(feature = "test-hooks")]
#[test]
fn fr43_default_off_reads_only_what_detection_reads() {
    let fx = dupes_fixture();
    let full = 200 * 1024;
    let (_, off) = support::run_report(fx.path(), &[]);
    for (path, bytes) in &off.reads {
        assert!(*bytes < full, "{path}: {bytes} bytes without --dupes");
    }
    let (_, on) = support::run_report(fx.path(), &["--dupes"]);
    let read = |name: &str| {
        on.reads
            .iter()
            .find(|(p, _)| p.ends_with(name))
            .map_or(0, |(_, n)| *n)
    };
    assert!(read("a.bin") >= full, "{:?}", on.reads);
    assert!(read("b.bin") >= full, "{:?}", on.reads);
}
