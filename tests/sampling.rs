//! Sampled type mixes of folded directories (FR-50) and the global cap of
//! content detection (FR-51).

mod support;

use support::{Fixture, corpus, run};

/// The type cell of a directory row: name, size, count, type, age.
fn dir_type(out: &support::Run, name: &str) -> String {
    out.cells(name)[3].to_string()
}

fn mixed_fixture() -> Fixture {
    let fx = Fixture::new();
    let png = corpus("logo.png");
    for i in 0..1000 {
        fx.file(&format!("pics/{}.png", support::word(i)), &png);
    }
    for i in 0..10 {
        fx.file(&format!("pics/{}.txt", support::word(i)), "a note\n");
    }
    fx.file("main.rs", "fn main() {}\n");
    fx
}

// FR-50: Sampled type mix

#[test]
fn fr50_a_folded_directory_shows_a_sampled_mix() {
    let fx = mixed_fixture();
    let out = run(&fx, &["-L", "1"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let ty = dir_type(&out, "pics/");
    assert!(ty.starts_with("≈ png"), "{ty}\n{}", out.stdout);
}

#[cfg(feature = "test-hooks")]
#[test]
fn fr50_at_most_3_files_per_extension_are_typed() {
    let fx = mixed_fixture();
    let (out, report) = support::run_report(fx.path(), &["-L", "1"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let read = |ext: &str| {
        report
            .reads
            .keys()
            .filter(|p| p.starts_with("./pics/") && p.ends_with(ext))
            .count()
    };
    assert!((1..=3).contains(&read(".png")), "{:?}", report.reads);
    assert!((1..=3).contains(&read(".txt")), "{:?}", report.reads);
}

#[test]
fn fr50_a_sample_types_the_whole_bucket() {
    // `.dat` says nothing; the samples say PNG, and so does the mix.
    let fx = Fixture::new();
    let png = corpus("logo.png");
    for i in 0..50 {
        fx.file(&format!("data/{}.dat", support::word(i)), &png);
    }
    fx.file("main.rs", "fn main() {}\n");
    let out = run(&fx, &["-L", "1"]);
    let ty = dir_type(&out, "data/");
    assert!(ty.starts_with("≈ png"), "{ty}\n{}", out.stdout);
}

#[cfg(feature = "test-hooks")]
#[test]
fn fr50_files_with_no_extension_are_each_typed() {
    let fx = Fixture::new();
    for i in 0..20 {
        fx.file(&format!("bin/{}", support::word(i)), corpus("script"));
    }
    fx.file("main.rs", "fn main() {}\n");
    let (out, report) = support::run_report(fx.path(), &["-L", "1"]);
    let read = report
        .reads
        .keys()
        .filter(|p| p.starts_with("./bin/"))
        .count();
    assert_eq!(read, 20, "{:?}", report.reads);
    let ty = dir_type(&out, "bin/");
    assert!(ty.contains("python"), "{ty}\n{}", out.stdout);
    assert!(!ty.contains('≈'), "every file was typed: {ty}");
}

#[test]
fn fr50_detect_off_takes_no_samples() {
    let fx = mixed_fixture();
    let out = run(&fx, &["-L", "1", "--detect", "off"]);
    let ty = dir_type(&out, "pics/");
    assert!(ty.starts_with("png"), "{ty}");
}

// FR-51: Global cap

#[cfg(feature = "test-hooks")]
#[test]
fn fr51_at_most_5000_files_get_content_detection() {
    let fx = Fixture::new();
    for d in 0..10 {
        for i in 0..1000 {
            fx.file(&format!("d{d}/{}", support::word(i)), "x\n");
        }
    }
    let (out, report) = support::run_report(fx.path(), &["-L", "1"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let typed = report.reads.len();
    assert!(typed > 0 && typed <= 5000, "{typed} files typed");
    let ty = dir_type(&out, "d9/");
    assert!(ty.starts_with('≈'), "{ty}\n{}", out.stdout);
}

#[cfg(feature = "test-hooks")]
#[test]
fn fr51_detection_stops_after_about_2_seconds() {
    let fx = Fixture::new();
    for d in 0..4 {
        for i in 0..250 {
            fx.file(&format!("d{d}/{}", support::word(i)), "x\n");
        }
    }
    let (out, report) = support::run_report_env(
        fx.path(),
        &["-L", "1"],
        &[("TEAK_TEST_READ_DELAY_MS", "10")],
    );
    assert_eq!(out.code, Some(0), "{out:?}");
    let ms = report.stages_ms["detect"];
    assert!((1500.0..=2500.0).contains(&ms), "detect took {ms} ms");
    assert!(
        report.reads.len() < 1000,
        "{} files typed",
        report.reads.len()
    );
}

#[cfg(all(feature = "test-hooks", magika))]
#[test]
fn fr51_magika_types_at_most_300_files_per_run() {
    // Plan A1 (phase-0 report): a Magika sub-cap inside the global cap.
    let fx = Fixture::new();
    for d in 0..4 {
        for i in 0..250 {
            fx.file(&format!("d{d}/{}", support::word(i)), corpus("tool"));
        }
    }
    // Magika in a debug build on a slow CI machine can reach the 2 s cap
    // before the 300 Magika files; this test is about the Magika sub-cap.
    let (out, report) = support::run_report_env(
        fx.path(),
        &["-L", "1", "--detect", "full"],
        &[("TEAK_TEST_DETECT_TIME_MS", "60000")],
    );
    assert_eq!(out.code, Some(0), "{out:?}");
    let magika = report.counts.get("magika").copied().unwrap_or(0);
    assert!((1..=300).contains(&magika), "{magika} Magika calls");
    assert!(
        report.reads.len() > 300,
        "{} files typed",
        report.reads.len()
    );
}
