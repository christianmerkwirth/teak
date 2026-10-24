//! Fast detection: stat, exact names, magic bytes, extension (FR-48), and
//! the secret check that guards previews (FR-77).

mod support;

use teak::detect::{Source, fast, secret};

use support::{Fixture, run};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";
const JPEG: &[u8] = b"\xff\xd8\xff\xe0\0\x10JFIF\0\x01\x01\0\0\x01\0\x01\0\0";
const PDF: &[u8] = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n1 0 obj\n<< /Type /Catalog >>\nendobj\n";

fn types_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file("Makefile", "all:\n\techo hi\n")
        .file("x.png", PNG)
        .file("noext", PDF)
        .file("a.rs", "fn main() {\n    println!(\"hi\");\n}\n")
        .file("y.png", JPEG);
    fx
}

fn type_cell(out: &support::Run, name: &str) -> String {
    let cells = out.cells(name);
    // name, size, type, age
    cells[2].to_string()
}

#[test]
fn fr48_fast_detection_uses_names_magic_and_extension() {
    let fx = types_fixture();
    let out = run(&fx, &["--detect", "fast"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert_eq!(type_cell(&out, "Makefile"), "makefile", "{}", out.stdout);
    assert_eq!(type_cell(&out, "x.png"), "png", "{}", out.stdout);
    assert_eq!(type_cell(&out, "noext"), "pdf", "{}", out.stdout);
    assert_eq!(type_cell(&out, "a.rs"), "rust", "{}", out.stdout);
}

#[test]
fn fr48_magic_bytes_win_over_the_extension() {
    let fx = types_fixture();
    let out = run(&fx, &["--detect", "fast"]);
    assert_eq!(type_cell(&out, "y.png"), "jpeg", "{}", out.stdout);
}

#[test]
fn fr48_full_acts_as_fast_until_task_3() {
    let fx = types_fixture();
    let out = run(&fx, &["--detect", "full"]);
    assert_eq!(type_cell(&out, "y.png"), "jpeg", "{}", out.stdout);
    assert_eq!(type_cell(&out, "noext"), "pdf", "{}", out.stdout);
}

#[test]
fn fr48_type_of_order() {
    let t = fast::type_of(b"Makefile", Some(b"all:\n"));
    assert_eq!((t.name(), t.source), ("makefile", Source::Name));
    let t = fast::type_of(b"x.png", Some(PNG));
    assert_eq!((t.name(), t.source), ("png", Source::Magic));
    let t = fast::type_of(b"y.png", Some(JPEG));
    assert_eq!(t.name(), "jpeg");
    assert_eq!(t.mismatch.map(|l| teak::detect::label(l).name), Some("png"));
    let t = fast::type_of(b"a.rs", Some(b"fn main() {}\n"));
    assert_eq!((t.name(), t.source), ("rust", Source::Ext));
    let t = fast::type_of(b"notes", Some(b"just some words\n"));
    assert_eq!(t.name(), "txt");
    let t = fast::type_of(b"run", Some(b"#!/usr/bin/env bash\necho hi\n"));
    assert_eq!(t.name(), "shell");
    let t = fast::type_of(b"blob", Some(b"\x00\x01\x02\x03\xfe\xff\x00\x10"));
    assert_eq!(t.name(), "unknown");
    // No head (--detect off): names and extensions only.
    assert_eq!(fast::type_of(b"y.png", None).name(), "png");
    assert_eq!(fast::type_of(b"noext", None).name(), "unknown");
}

#[cfg(feature = "test-hooks")]
#[test]
fn fr48_detect_off_reads_no_content() {
    let fx = types_fixture();
    let (out, report) = support::run_report(fx.path(), &["--detect", "off"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(report.reads.is_empty(), "{:?}", report.reads);
    assert_eq!(type_cell(&out, "y.png"), "png", "{}", out.stdout);
}

#[cfg(feature = "test-hooks")]
#[test]
fn fr48_fast_reads_one_small_head_per_visible_file() {
    let fx = types_fixture();
    fx.sized("big.dat", 10_000);
    let (_, report) = support::run_report(fx.path(), &["--detect", "fast"]);
    let big = fx.abs("big.dat");
    assert_eq!(
        report.reads.get(big.to_str().unwrap()),
        Some(&4096),
        "{:?}",
        report.reads
    );
    assert_eq!(report.reads.len(), 6, "{:?}", report.reads);
}

#[test]
fn fr77_secret_names_and_key_headers() {
    for name in [
        &b".env"[..],
        b".env.local",
        b"server.pem",
        b"tls.key",
        b"id_rsa",
        b"id_ed25519",
        b"credentials.json",
        b".netrc",
        b".pgpass",
    ] {
        assert!(
            secret::is_secret(name, None),
            "{}",
            String::from_utf8_lossy(name)
        );
    }
    assert!(!secret::is_secret(b"main.rs", Some(b"fn main() {}\n")));
    assert!(!secret::is_secret(b"id_rsa.pub", Some(b"ssh-rsa AAAA")));
    assert!(secret::is_secret(
        b"backup.txt",
        Some(b"-----BEGIN OPENSSH PRIVATE KEY-----\nb3Blbn\n")
    ));
    assert!(secret::is_secret(
        b"x",
        Some(b"junk\n-----BEGIN RSA PRIVATE KEY-----\n")
    ));
}
