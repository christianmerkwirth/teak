//! Series rows, special slots, series content, and uniform directories
//! (FR-44 to FR-47).

mod support;

use support::{Fixture, cells, run};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";
const JPEG: &[u8] = b"\xff\xd8\xff\xe0\0\x10JFIF\0\x01\x01\0\0\x01\0\x01\0\0";

/// A file of `len` bytes that starts with `magic`.
fn with_magic(fx: &Fixture, path: &str, magic: &[u8], len: usize) {
    let mut bytes = magic.to_vec();
    bytes.resize(len, 0);
    fx.file(path, bytes);
}

// FR-44: Series rows

#[test]
fn fr44_series_row_with_range_and_missing() {
    let fx = Fixture::new();
    for i in (1..=10).filter(|&i| i != 5) {
        with_magic(&fx, &format!("IMG_{i:04}.png"), PNG, 100);
    }
    let out = run(&fx, &[]);
    let row = out.must_row("IMG_[0001–0010].png");
    assert!(row.contains("9 of 10"), "{row}");
    assert_eq!(out.lines().len(), 3, "{}", out.stdout);
}

#[test]
fn fr44_short_runs_stay_rows_unless_series_min_allows() {
    let fx = Fixture::new();
    for i in 1..=3 {
        with_magic(&fx, &format!("IMG_{i:04}.png"), PNG, 100);
    }
    let out = run(&fx, &[]);
    for i in 1..=3 {
        out.must_row(&format!("IMG_{i:04}.png"));
    }
    let out = run(&fx, &["--series-min", "2"]);
    let row = out.must_row("IMG_[0001–0003].png");
    assert!(cells(row).contains(&"3"), "{row}");
}

#[test]
fn fr44_one_series_per_extension() {
    let fx = Fixture::new();
    for i in 1..=4 {
        with_magic(&fx, &format!("IMG_{i:04}.png"), PNG, 100);
        with_magic(&fx, &format!("IMG_{i:04}.jpg"), JPEG, 100);
    }
    let out = run(&fx, &[]);
    out.must_row("IMG_[0001–0004].jpg");
    out.must_row("IMG_[0001–0004].png");
}

// FR-45: Special slots

#[test]
fn fr45_dates_show_as_a_range() {
    let fx = Fixture::new();
    for d in 1..=30 {
        fx.file(&format!("log-2024-05-{d:02}.txt"), "line\n");
    }
    let out = run(&fx, &[]);
    out.must_row("log-[2024-05-01–2024-05-30].txt");
}

#[test]
fn fr45_hex_hashes_and_uuids() {
    let fx = Fixture::new();
    for i in 0..20u64 {
        let h = format!("{:040x}", (i + 1) * 0x9e37_79b9_7f4a_7c15_u64);
        fx.file(&format!("objects/{h}"), "blob\n");
    }
    for i in 0..6u32 {
        let id = format!("{:08x}-1234-4abc-8def-{:012x}", i * 7919 + 1, i + 100);
        fx.file(&format!("meta/{id}.json"), "{}\n");
    }
    let out = run(&fx, &[]);
    let hash = out.must_row("[hash]");
    assert!(cells(hash).contains(&"20"), "{hash}");
    out.must_row("[uuid].json");
}

#[test]
fn fr45_fifteen_hex_chars_are_not_a_hash() {
    let fx = Fixture::new();
    for i in 0..6u64 {
        let h = format!("{:015x}", (i + 1) * 0x0123_4567_89ab_cdu64);
        fx.file(&h, "x\n");
    }
    let out = run(&fx, &[]);
    assert!(out.row("[hash]").is_none(), "{}", out.stdout);
}

// FR-46: Series row content

const KIB: [usize; 10] = [300, 340, 350, 350, 350, 360, 360, 380, 400, 410];

#[test]
fn fr46_series_row_shows_count_total_median_and_type() {
    let fx = Fixture::new();
    for (i, k) in KIB.iter().enumerate() {
        with_magic(&fx, &format!("shot_{:02}.png", i + 1), PNG, k * 1024);
    }
    let out = run(&fx, &[]);
    let row = out.must_row("shot_[01–10].png");
    let c = cells(row);
    assert!(c.contains(&"10"), "{row}");
    let total: usize = KIB.iter().sum();
    assert!(
        c.contains(&format!("{:.1}M", total as f64 / 1024.0).as_str()),
        "{row}"
    );
    assert!(row.contains("~355K"), "{row}");
    assert!(row.contains("png"), "{row}");
}

#[test]
fn fr46_series_type_mix_from_content() {
    let fx = Fixture::new();
    for (i, k) in KIB.iter().enumerate() {
        let magic = if i == 3 { JPEG } else { PNG };
        with_magic(&fx, &format!("shot_{:02}.png", i + 1), magic, k * 1024);
    }
    let out = run(&fx, &["--detect", "fast"]);
    let row = out.must_row("shot_[01–10].png");
    assert!(row.contains("png 9 · jpeg 1"), "{row}");
}

// FR-47: Uniform directories

fn photos(series: usize, other: usize) -> Fixture {
    let fx = Fixture::new();
    for i in 1..=series {
        with_magic(&fx, &format!("photos/IMG_{i:04}.jpg"), JPEG, 1000 + i);
    }
    for i in 0..other {
        fx.file(&format!("photos/note{i}.txt"), "n\n");
    }
    fx
}

#[test]
fn fr47_uniform_directory_is_one_row() {
    let fx = photos(95, 5);
    let out = run(&fx, &[]);
    let row = out.must_row("photos/");
    assert!(row.contains("jpeg"), "{row}");
    assert!(row.contains('~'), "median: {row}");
    assert_eq!(out.lines().len(), 3, "{}", out.stdout);
}

#[test]
fn fr47_mixed_directory_opens() {
    let fx = photos(85, 15);
    let out = run(&fx, &[]);
    out.must_row("IMG_[0001–0085].jpg");
    assert!(out.lines().len() > 3, "{}", out.stdout);
}
