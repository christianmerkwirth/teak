//! Project kinds (FR-59): marker files name the kind in the header.

mod support;

use support::{Fixture, run};

#[test]
fn fr59_marker_files_name_the_kind() {
    for (marker, kind) in [
        ("Cargo.toml", "rust crate"),
        ("package.json", "node pkg"),
        ("pyproject.toml", "python pkg"),
        ("pyvenv.cfg", "venv"),
    ] {
        let fx = Fixture::new();
        fx.file(marker, "x = 1\n").file("notes.txt", "hello\n");
        let out = run(&fx, &[]);
        assert!(
            out.header().contains(&format!(" · {kind}")),
            "{marker}: {}",
            out.header()
        );
    }
}

#[test]
fn fr59_two_markers_join_with_a_plus() {
    let fx = Fixture::new();
    fx.file("Cargo.toml", "[package]\n")
        .file("package.json", "{}\n");
    let out = run(&fx, &[]);
    assert!(
        out.header().contains("rust crate + node pkg"),
        "{}",
        out.header()
    );
}

/// A small JPEG with an EXIF APP1 segment, or without one.
fn jpeg(exif: bool) -> Vec<u8> {
    let mut b = vec![0xFF, 0xD8];
    if exif {
        b.extend_from_slice(&[0xFF, 0xE1, 0x00, 0x10]);
        b.extend_from_slice(b"Exif\0\0MM\0*\0\0\0\x08");
    } else {
        b.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
        b.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
    }
    b.extend_from_slice(&[0xFF, 0xD9]);
    b.resize(2048, 0);
    b
}

#[test]
fn fr59_mostly_exif_images_are_photos() {
    let fx = Fixture::new();
    for i in 0..9 {
        fx.file(&format!("DSC_{i:04}.jpg"), jpeg(true));
    }
    fx.file("index.txt", "list\n");
    let out = run(&fx, &[]);
    assert!(out.header().contains(" · photos"), "{}", out.header());
}

#[test]
fn fr59_images_without_exif_are_not_photos() {
    let fx = Fixture::new();
    for i in 0..9 {
        fx.file(&format!("DSC_{i:04}.jpg"), jpeg(false));
    }
    let out = run(&fx, &[]);
    assert!(!out.header().contains("photos"), "{}", out.header());
}

#[test]
fn fr59_no_marker_no_kind() {
    let fx = Fixture::new();
    fx.file("a.txt", "a\n").file("b.txt", "b\n");
    let out = run(&fx, &[]);
    assert!(out.header().ends_with("· 2 files"), "{}", out.header());
}
