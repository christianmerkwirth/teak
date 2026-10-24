//! Previews (FR-56): one fixture file per type, `--preview off`, the short
//! cut, control characters, and the reads that previews make.

mod support;

use support::{Fixture, cells, run};

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// A gzip file with one stored deflate block.
fn gzip(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3];
    let len = data.len() as u16;
    out.push(1);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

/// A zip file with `n` stored entries.
fn zip(n: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for i in 0..n {
        let name = format!("f{i:02}.txt");
        let data = format!("entry {i}\n");
        let offset = out.len() as u32;
        let crc = crc32(data.as_bytes());
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data.as_bytes());
        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(n as u16).to_le_bytes());
    out.extend_from_slice(&(n as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    out
}

/// A PNG header of `w` × `h` with a short image chunk.
fn png(w: u32, h: u32) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = b"IHDR".to_vec();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    out.extend_from_slice(&13u32.to_be_bytes());
    out.extend_from_slice(&ihdr);
    out.extend_from_slice(&crc32(&ihdr).to_be_bytes());
    let idat: Vec<u8> = (0..600u32).map(|i| (i * 7 % 251) as u8).collect();
    out.extend_from_slice(&(idat.len() as u32).to_be_bytes());
    out.extend_from_slice(b"IDAT");
    out.extend_from_slice(&idat);
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(b"IEND\xaeB`\x82");
    out
}

/// A 64-bit little-endian ELF header for x86-64, with some code after it.
fn elf() -> Vec<u8> {
    let mut b = vec![0u8; 64];
    b[..4].copy_from_slice(b"\x7fELF");
    b[4] = 2; // 64-bit
    b[5] = 1; // little endian
    b[6] = 1;
    b[16] = 2; // executable
    b[18] = 0x3E; // x86-64
    b[20] = 1;
    b.extend((0..4000u32).map(|i| (i * 31 % 253) as u8));
    b
}

fn csv_rows(n: usize) -> String {
    let mut s = String::from("id,name,price\n");
    for i in 0..n {
        s.push_str(&format!("{i:04},item{i:04},{:06}\n", i * 3));
    }
    s
}

fn typed_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file(
        "main.rs",
        "// SPDX-License-Identifier: MIT\n\nfn main() {\n    println!(\"hi\");\n}\n",
    )
    .file("README.md", "# Shop backend\n\nThe shop.\n")
    .file(
        "package.json",
        "{\n  \"name\": \"shop\",\n  \"version\": \"1.0.0\",\n  \"scripts\": {\"start\": \"node .\"}\n}\n",
    )
    .file("data.csv", csv_rows(1000))
    .file("photo.png", png(1920, 1080))
    .file(
        "dump.sql.gz",
        gzip(b"CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);\nINSERT INTO items VALUES (1, 'a');\n"),
    )
    .file("a.zip", zip(12))
    .file("app", elf())
    .file("target", "the target\n")
    .symlink("l", "target")
    .symlink("d", "nowhere");
    fx
}

#[test]
fn fr56_each_type_has_its_preview() {
    let fx = typed_fixture();
    let out = run(&fx, &["--preview", "full"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    for (name, preview) in [
        ("main.rs", "fn main() {"),
        ("README.md", "# Shop backend"),
        ("package.json", "{name, version, scripts}"),
        ("data.csv", "id,name,price ≈1.0k rows"),
        ("photo.png", "1920×1080"),
        ("dump.sql.gz", "gzip → sql"),
        ("a.zip", "12 entries"),
        ("app", "ELF x86-64"),
        ("l", "⇢ target"),
        ("d", "⇢ nowhere (broken)"),
    ] {
        let row = out.must_row(name);
        assert!(cells(row).contains(&preview), "{name}: {row}");
    }
}

#[test]
fn fr56_short_previews_by_default() {
    let fx = typed_fixture();
    let out = run(&fx, &[]);
    assert!(
        cells(out.must_row("main.rs")).contains(&"fn main() {"),
        "{}",
        out.stdout
    );
    let fx = Fixture::new();
    let long = format!("# {}\n", "word ".repeat(30));
    fx.file("NOTES.md", &long);
    let short = run(&fx, &[]);
    let preview = *cells(short.must_row("NOTES.md")).last().unwrap();
    assert!(preview.starts_with("# word"), "{}", short.stdout);
    assert_eq!(support::width(preview), 40, "{preview:?}");
    assert!(preview.ends_with('…'), "{preview:?}");
    let full = run(&fx, &["--preview", "full"]);
    let preview = *cells(full.must_row("NOTES.md")).last().unwrap();
    assert!(support::width(preview) > 40, "{preview:?}");
}

#[test]
fn fr56_preview_off() {
    let fx = typed_fixture();
    let out = run(&fx, &["--preview", "off"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    for text in [
        "fn main", "# Shop", "{name", "rows", "1920", "gzip →", "entries", "ELF x86",
    ] {
        assert!(!out.stdout.contains(text), "{text}: {}", out.stdout);
    }
}

#[test]
fn fr56_control_characters_are_escaped() {
    let fx = Fixture::new();
    fx.file("ctl.txt", b"a\x01b\x1b[31mred\x7f\n".as_slice());
    let out = run(&fx, &["--preview", "full"]);
    let row = out.must_row("ctl.txt");
    assert!(row.contains("a\\x01b\\x1B[31mred\\x7F"), "{row}");
    assert!(
        !out.stdout.bytes().any(|b| b < 0x20 && b != b'\n'),
        "{:?}",
        out.stdout
    );
}

#[test]
fn fr56_ascii_previews() {
    let fx = typed_fixture();
    let out = run(&fx, &["--ascii", "--preview", "full"]);
    assert!(out.stdout.is_ascii(), "{}", out.stdout);
    assert!(
        cells(out.must_row("photo.png")).contains(&"1920x1080"),
        "{}",
        out.stdout
    );
    assert!(
        cells(out.must_row("dump.sql.gz")).contains(&"gzip -> sql"),
        "{}",
        out.stdout
    );
}

#[test]
fn fr56_no_content_previews_with_detect_off() {
    let fx = typed_fixture();
    let out = run(&fx, &["--detect", "off", "--preview", "full"]);
    assert!(!out.stdout.contains("fn main"), "{}", out.stdout);
    // Symlink targets come from stat.
    assert!(out.must_row("l").contains("⇢ target"), "{}", out.stdout);
}

#[cfg(feature = "test-hooks")]
#[test]
fn fr56_previews_read_no_more_than_detection() {
    let fx = Fixture::new();
    fx.file("main.rs", "fn main() {}\n")
        .file("README.md", "# Title\n")
        .file("notes.txt", "notes\n".repeat(2000));
    let (_, off) = support::run_report(fx.path(), &["--preview", "off"]);
    let (on, with) = support::run_report(fx.path(), &["--preview", "full"]);
    assert!(
        on.must_row("main.rs").contains("fn main() {}"),
        "{}",
        on.stdout
    );
    assert_eq!(off.reads, with.reads);
    assert!(with.preview_reads.is_empty(), "{:?}", with.preview_reads);
}

#[cfg(feature = "test-hooks")]
#[test]
fn fr56_warm_cache_previews_read_only_for_previews() {
    let fx = Fixture::new();
    fx.file("main.rs", "fn main() {}\n")
        .file("README.md", "# Title\n");
    let cache = support::fresh_cache_dir();
    let cold = support::run_cached(fx.path(), &cache, &[]);
    let report_dir = tempfile::tempdir().unwrap();
    let report = report_dir.path().join("r.json");
    let warm = support::Run::from_output(
        support::command(fx.path())
            .args(support::DEFAULT_FLAGS)
            .env("XDG_CACHE_HOME", &cache)
            .env("TEAK_TEST_REPORT", &report)
            .output()
            .unwrap(),
    );
    let report = support::read_report(&report);
    assert_eq!(cold.stdout, warm.stdout);
    assert!(
        warm.must_row("main.rs").contains("fn main() {}"),
        "{}",
        warm.stdout
    );
    assert!(report.reads.is_empty(), "{:?}", report.reads);
    assert!(!report.preview_reads.is_empty());
}
