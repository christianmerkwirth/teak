//! Columns and display: FR-22, FR-23, FR-25 to FR-29, FR-34 (bold size),
//! FR-58, and FR-62. Pty tests use `portable-pty`.

mod support;

use support::{
    Fixture, cells, command, depth, run, run_pty, strip_ansi, strip_glyphs, width, word,
};

/// The SGR code sequence right before `text`, if any.
fn sgr_before<'a>(line: &'a str, text: &str) -> Option<&'a str> {
    let at = line.find(text)?;
    let before = &line[..at];
    let start = before.rfind("\x1b[")?;
    let seq = &before[start + 2..];
    seq.strip_suffix('m').filter(|s| !s.contains('\x1b'))
}

fn has_ansi(text: &str) -> bool {
    text.contains("\x1b[")
}

// FR-22: Default columns

#[test]
fn fr22_permissions_show_only_when_unusual() {
    let fx = Fixture::new();
    fx.file("run.sh", "#!/bin/sh\necho hi\n")
        .mode("run.sh", 0o755)
        .file("notes.txt", "n\n")
        .mode("notes.txt", 0o644)
        .file("setuid.bin", "x")
        .mode("setuid.bin", 0o4755)
        .file("open.txt", "o\n")
        .mode("open.txt", 0o666)
        .file("sub/inner.txt", "i\n");
    let out = run(&fx, &[]);
    assert!(out.cells("run.sh").contains(&"rwx"), "{}", out.stdout);
    let notes = out.cells("notes.txt");
    for p in ["rwx", "suid", "sgid", "o+w"] {
        assert!(!notes.iter().any(|c| c.contains(p)), "{notes:?}");
    }
    assert!(
        out.must_row("setuid.bin").contains("suid"),
        "{}",
        out.stdout
    );
    assert!(out.cells("open.txt").contains(&"o+w"), "{}", out.stdout);
    // A directory row has a file count; a file row has none.
    assert_eq!(out.cells("sub/")[2], "1", "{}", out.stdout);
    let notes = out.cells("notes.txt");
    assert_eq!(notes[2], "txt", "{notes:?}");
}

// FR-23: Size units

#[test]
fn fr23_sizes_in_three_units() {
    let fx = Fixture::new();
    fx.sized("f.bin", 1536).file("empty.txt", "");
    assert_eq!(run(&fx, &[]).cells("f.bin")[1], "1.5K");
    assert_eq!(run(&fx, &["--bytes"]).cells("f.bin")[1], "1536");
    assert_eq!(run(&fx, &["--si"]).cells("f.bin")[1], "1.5k");
    let out = run(&fx, &["--chaff", "show"]);
    assert_eq!(out.cells("empty.txt")[1], "0", "{}", out.stdout);
}

#[test]
fn fr23_size_format_edges() {
    use teak::layout::{SizeUnits, size};
    assert_eq!(size(0, SizeUnits::Iec), "0");
    assert_eq!(size(1023, SizeUnits::Iec), "1023");
    assert_eq!(size(1024, SizeUnits::Iec), "1.0K");
    assert_eq!(size(3 * 1024, SizeUnits::Iec), "3.0K");
    assert_eq!(size(10 * 1024 - 1, SizeUnits::Iec), "10K");
    assert_eq!(size(14 << 20, SizeUnits::Iec), "14M");
    assert_eq!(size((1 << 30) - 1, SizeUnits::Iec), "1.0G");
    assert_eq!(size(1500, SizeUnits::Si), "1.5k");
    assert_eq!(teak::layout::compact(812), "812");
    assert_eq!(teak::layout::compact(3200), "3.2k");
    assert_eq!(teak::layout::compact(44_190), "44k");
    assert_eq!(teak::layout::thousands(48_213), "48,213");
}

// FR-25: Width

/// A tree like PRD §5.1, with a secret so the preview column shows.
fn shop() -> Fixture {
    let fx = Fixture::new();
    fx.file("shop/src/main.rs", "fn main() {}\n")
        .file("shop/src/api/mod.rs", "pub mod v1;\n")
        .file("shop/src/api/v1.rs", "pub fn get() {}\n")
        .file("shop/Cargo.toml", "[package]\nname = \"shop\"\n")
        .file("shop/Cargo.lock", "# lock\n")
        .file("shop/package.json", "{}\n")
        .file("shop/node_modules/x/index.js", "x\n")
        .file("shop/deploy.sh", "#!/usr/bin/env bash\n")
        .mode("shop/deploy.sh", 0o755)
        .file("shop/README.md", "# Shop backend\n")
        .file("shop/server.pem", "-----BEGIN PRIVATE KEY-----\n")
        .sized("shop/data/dump.sql.gz", 640 << 10)
        .sized("shop/data/report.pdf", 220 << 10);
    for i in 1..=12 {
        fx.sized(&format!("shop/assets/IMG_{i:04}.png"), 350 << 10);
    }
    fx.mtime_all(3 * 86_400);
    fx
}

#[test]
fn fr25_narrow_width_drops_previews_before_ages() {
    let fx = shop();
    let wide = run(&fx, &["shop"]);
    assert!(
        wide.stdout.contains("no preview (secret)"),
        "{}",
        wide.stdout
    );
    let out = run(&fx, &["shop", "--width", "60"]);
    for line in out.lines() {
        assert!(width(line) <= 60, "{} columns: {line}", width(line));
    }
    assert!(!out.stdout.contains("no preview"), "{}", out.stdout);
    assert!(out.must_row("README.md").ends_with("3d"), "{}", out.stdout);
}

#[test]
fn fr25_very_narrow_rows_keep_glyphs_name_and_size() {
    let fx = shop();
    let out = run(&fx, &["shop", "--width", "20"]);
    for line in out.lines() {
        assert!(width(line) <= 20, "{} columns: {line}", width(line));
    }
    let readme = out
        .lines()
        .into_iter()
        .find(|l| strip_glyphs(l).starts_with("REA"))
        .unwrap_or_else(|| panic!("{}", out.stdout));
    assert!(
        readme.starts_with("├── ") || readme.starts_with("└── "),
        "{readme}"
    );
    assert!(cells(readme).contains(&"15"), "size: {readme}");
}

// FR-26: Time format

#[test]
fn fr26_relative_and_iso_times() {
    let fx = Fixture::new();
    fx.file("a.txt", "a\n").mtime("a.txt", 7200);
    assert_eq!(run(&fx, &[]).cells("a.txt")[3], "2h");
    // SCAN_TIME is 2025-06-15T15:06:40Z.
    assert_eq!(
        run(&fx, &["--time", "iso"]).cells("a.txt")[3],
        "2025-06-15T13:06"
    );
}

#[test]
fn fr26_age_units() {
    use teak::layout::relative_age;
    assert_eq!(relative_age(0), "0s");
    assert_eq!(relative_age(59), "59s");
    assert_eq!(relative_age(60), "1m");
    assert_eq!(relative_age(3 * 86_400), "3d");
    assert_eq!(relative_age(3 * 7 * 86_400), "3w");
    assert_eq!(relative_age(150 * 86_400), "5mo");
    assert_eq!(relative_age(400 * 86_400), "1y");
}

// FR-27: Colors

fn color_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file("main.rs", "fn main() {}\n")
        .file("photo.png", "png")
        .file("Cargo.lock", "# lock\n");
    fx
}

#[test]
fn fr27_type_groups_have_colors_and_chaff_is_dim() {
    let fx = color_fixture();
    let out = run(&fx, &["--color", "always"]);
    let rs = sgr_before(&out.stdout, "main.rs").expect("main.rs has a color");
    let png = sgr_before(&out.stdout, "photo.png").expect("photo.png has a color");
    assert_ne!(rs, png, "{:?}", out.stdout);
    let chaff = out
        .lines()
        .into_iter()
        .find(|l| strip_ansi(l).contains("▪ chaff"))
        .expect("a chaff line");
    assert!(
        chaff.contains("\x1b[2m") || chaff.contains("\x1b[2;"),
        "{chaff:?}"
    );
}

#[test]
fn fr27_no_color_on_a_terminal() {
    let fx = color_fixture();
    let text = run_pty(fx.path(), &[], 40, 100, &[("NO_COLOR", "1")]);
    assert!(!has_ansi(&text), "{text:?}");
}

#[test]
fn fr27_clicolor_force_in_a_pipe() {
    let fx = color_fixture();
    let out = command(fx.path())
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap();
    assert!(has_ansi(&String::from_utf8_lossy(&out.stdout)));
}

#[test]
fn fr27_color_flag_beats_no_color() {
    let fx = color_fixture();
    let out = command(fx.path())
        .args(["--color", "always"])
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(has_ansi(&String::from_utf8_lossy(&out.stdout)));
}

#[test]
fn fr27_ls_colors_change_only_the_name() {
    let fx = color_fixture();
    let run_with = |ls: Option<&str>| {
        let mut cmd = command(fx.path());
        cmd.args(support::DEFAULT_FLAGS).args(["--color", "always"]);
        if let Some(ls) = ls {
            cmd.env("LS_COLORS", ls);
        }
        String::from_utf8(cmd.output().unwrap().stdout).unwrap()
    };
    let plain = run_with(None);
    let ls = run_with(Some("*.rs=01;31"));
    let code = sgr_before(&ls, "main.rs").expect("main.rs has a color");
    let codes: Vec<u32> = code.split(';').filter_map(|c| c.parse().ok()).collect();
    assert!(
        codes.contains(&1) && codes.contains(&31),
        "bold red: {ls:?}"
    );
    // Remove the name with its codes; the rest of the line is the same.
    let line = |text: &str| -> String {
        let l = text.lines().find(|l| l.contains("main.rs")).unwrap();
        let at = l.find("main.rs").unwrap();
        let start = l[..at].rfind("\x1b[").unwrap();
        let end = at + l[at..].find("\x1b[0m").unwrap() + 4;
        format!("{}{}", &l[..start], &l[end..])
    };
    assert_eq!(line(&plain), line(&ls));
}

// FR-28: Not color only

#[test]
fn fr28_stripped_colors_equal_no_colors() {
    let fx = shop();
    let colored = run(&fx, &["shop", "--color", "always"]);
    let plain = run(&fx, &["shop", "--color", "never"]);
    assert!(has_ansi(&colored.stdout));
    assert_eq!(strip_ansi(&colored.stdout), plain.stdout);
}

// FR-29: ASCII glyphs

#[test]
fn fr29_ascii_output_is_ascii() {
    let fx = shop();
    let out = run(&fx, &["shop", "--ascii", "-A"]);
    assert!(out.stdout.bytes().all(|b| b < 0x80), "{}", out.stdout);
    assert!(out.stdout.contains("|-- "), "{}", out.stdout);
    assert!(out.stdout.contains("`-- "), "{}", out.stdout);
}

// FR-34: the size of a large chaff line is bold

#[test]
fn fr34_large_chaff_size_is_bold() {
    let fx = Fixture::new();
    fx.file("Cargo.toml", "[package]\n")
        .sized("target/release/app", 1229 << 20)
        .sized("src/big.rs", 14 << 20);
    let out = run(&fx, &["--color", "always"]);
    let first = out.lines()[1];
    assert!(strip_ansi(first).contains("▪ chaff"), "{first:?}");
    let code = sgr_before(first, "1.2G").expect("the size has codes");
    assert!(code.split(';').any(|c| c == "1"), "{first:?}");
}

// FR-58: Folded-directory summary

#[test]
fn fr58_folded_summary_shows_types_dirs_and_age() {
    let fx = Fixture::new();
    let kib = |n: u64| n * 1024;
    fx.sized("pkg/a/lib.rs", kib(50))
        .sized("pkg/b/x.toml", kib(20))
        .sized("pkg/doc.md", kib(15))
        .sized("pkg/data.json", kib(10))
        .sized("pkg/notes.txt", kib(5));
    fx.mtime_all(86_400 * 30);
    fx.mtime("pkg/notes.txt", 3600);
    let out = run(&fx, &["-L", "1"]);
    let row = out.must_row("pkg/");
    let c = cells(row);
    assert_eq!(c[1], "100K", "{row}");
    assert_eq!(c[2], "5", "{row}");
    for part in ["2 dirs", "rust", "toml", "markdown", "+2 types"] {
        assert!(row.contains(part), "{part}: {row}");
    }
    assert!(row.ends_with("1h"), "{row}");
}

#[test]
fn fr58_empty_folded_directory() {
    let fx = Fixture::new();
    fx.dir("nothing").file("a.txt", "a\n");
    let out = run(&fx, &["-L", "1", "--chaff", "show"]);
    let c = out.cells("nothing/");
    assert_eq!(c[1], "0", "{c:?}");
    assert!(c.contains(&"empty"), "{c:?}");
}

// FR-62: Default format

#[test]
fn fr62_pipe_is_plain_120() {
    let fx = shop();
    let out = command(fx.path()).arg("shop").output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!has_ansi(&text), "{text:?}");
    for line in text.lines() {
        assert!(width(line) <= 120, "{line}");
    }
}

#[test]
fn fr62_terminal_is_tty_at_its_width() {
    let fx = shop();
    let text = run_pty(fx.path(), &["shop"], 40, 100, &[]);
    assert!(has_ansi(&text), "tty format has colors: {text:?}");
    for line in strip_ansi(&text).lines() {
        assert!(width(line) <= 100, "{line}");
    }
}

#[test]
fn fr62_plain_on_a_terminal() {
    let fx = shop();
    let text = run_pty(
        fx.path(),
        &["shop", "--format", "plain", "--width", "80"],
        40,
        100,
        &[],
    );
    assert!(!has_ansi(&text), "{text:?}");
    for line in text.lines() {
        assert!(width(line) <= 80, "{line}");
    }
}

/// The tree glyphs nest: a last child's children get blank columns.
#[test]
fn glyph_columns_follow_the_tree() {
    let fx = Fixture::new();
    fx.file("a/x.txt", "x\n")
        .file("a/y.txt", "y\n")
        .file(&format!("{}.txt", word(1)), "b\n");
    let out = run(&fx, &[]);
    let x = out.must_row("x.txt");
    assert!(x.starts_with("│   ├── "), "{}", out.stdout);
    assert_eq!(depth(x), 2);
}

/// `insta` snapshots of whole views. The fixtures fix every mtime, so the
/// output is stable.
mod snapshots {
    use super::support::{Fixture, run};

    const JPEG: &[u8] = b"\xff\xd8\xff\xe0\0\x10JFIF\0\x01\x01\0\0\x01\0\x01\0\0";
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";

    fn magic(fx: &Fixture, path: &str, head: &[u8], len: usize) {
        let mut b = head.to_vec();
        b.resize(len, 0);
        fx.file(path, b);
    }

    fn view(fx: &Fixture, root: &str, extra: &[&str]) -> String {
        let mut args = vec![root, "-n", "40"];
        args.extend_from_slice(extra);
        run(fx, &args).stdout
    }

    #[test]
    fn rust_repo() {
        let fx = Fixture::new();
        fx.file("rustrepo/Cargo.toml", "[package]\nname = \"rustrepo\"\n")
            .file(
                "rustrepo/Cargo.lock",
                "# This file is automatically @generated by Cargo.\n",
            )
            .file("rustrepo/README.md", "# rustrepo\n\nA small tool.\n")
            .file("rustrepo/.gitignore", "/target\n")
            .file("rustrepo/.git/HEAD", "ref: refs/heads/main\n")
            .file(
                "rustrepo/src/main.rs",
                "fn main() {\n    rustrepo::run();\n}\n",
            )
            .file("rustrepo/src/lib.rs", "pub mod cli;\npub fn run() {}\n")
            .file("rustrepo/src/cli/mod.rs", "pub mod args;\n")
            .file("rustrepo/src/cli/args.rs", "pub struct Args;\n")
            .file("rustrepo/tests/it.rs", "#[test]\nfn it() {}\n")
            .sized("rustrepo/target/debug/rustrepo", 9 << 20)
            .sized("rustrepo/target/debug/deps/librustrepo.rlib", 3 << 20);
        fx.mtime_all(2 * 86_400);
        fx.mtime("rustrepo/src/lib.rs", 3600);
        insta::assert_snapshot!(view(&fx, "rustrepo", &[]));
    }

    #[test]
    fn node_repo() {
        let fx = Fixture::new();
        fx.file("noderepo/package.json", "{\"name\": \"noderepo\"}\n")
            .file("noderepo/package-lock.json", "{}\n")
            .file("noderepo/src/index.ts", "export const a = 1;\n")
            .file("noderepo/src/app.tsx", "export function App() {}\n")
            .file("noderepo/dist/bundle.min.js", "var a=1;\n")
            .file("noderepo/.env", "TOKEN=abc\n");
        for (pkg, files) in [("react", 40), ("lodash", 120), ("typescript", 60)] {
            for i in 0..files {
                fx.sized(
                    &format!("noderepo/node_modules/{pkg}/lib/m{i}.js"),
                    2000 + i * 13,
                );
            }
        }
        fx.mtime_all(10 * 86_400);
        insta::assert_snapshot!(view(&fx, "noderepo", &[]));
    }

    #[test]
    fn photo_folder() {
        let fx = Fixture::new();
        for i in 1..=40 {
            magic(
                &fx,
                &format!("photos/2019/IMG_{i:04}.jpg"),
                JPEG,
                2000 + i * 31,
            );
        }
        for i in (1..=30).filter(|i| i % 7 != 0) {
            magic(
                &fx,
                &format!("photos/2020/IMG_{i:04}.jpg"),
                JPEG,
                2500 + i * 17,
            );
        }
        for i in 1..=5 {
            magic(&fx, &format!("photos/2020/scan_{i:02}.png"), PNG, 900 + i);
        }
        fx.file("photos/.DS_Store", "ds")
            .file("photos/index.txt", "photos\n");
        fx.mtime_all(400 * 86_400);
        insta::assert_snapshot!(view(&fx, "photos", &[]));
    }

    #[test]
    fn data_lake() {
        let fx = Fixture::new();
        for i in 0..100 {
            fx.sized(&format!("lake/raw/part-{i:05}.parquet"), 40_000 + i * 97);
        }
        for d in 1..=30 {
            fx.file(&format!("lake/logs/log-2024-05-{d:02}.txt"), "ok\n");
        }
        for i in 0..20u64 {
            let h = format!("{:040x}", (i + 3).wrapping_mul(0x9e37_79b9_7f4a_7c15));
            fx.file(&format!("lake/blobs/{h}"), "blob\n");
        }
        fx.file("lake/README.md", "# Lake\n")
            .file("lake/schema.json", "{}\n");
        fx.mtime_all(3 * 7 * 86_400);
        insta::assert_snapshot!(view(&fx, "lake", &[]));
    }

    #[test]
    fn odd_permissions_and_broken_links() {
        if super::support::is_root() {
            eprintln!("skipped: root can read mode-000 directories");
            return;
        }
        let fx = Fixture::new();
        fx.file("odd/run.sh", "#!/bin/sh\n")
            .mode("odd/run.sh", 0o755)
            .file("odd/setuid", "x")
            .mode("odd/setuid", 0o4755)
            .file("odd/world.txt", "w\n")
            .mode("odd/world.txt", 0o666)
            .file("odd/locked/secret.txt", "s\n")
            .file("odd/plain.txt", "p\n")
            .symlink("odd/dead", "nowhere")
            .symlink("odd/self", "self")
            .symlink("odd/to_plain", "plain.txt");
        fx.mtime_all(5 * 3600);
        fx.mode("odd/locked", 0o000);
        let out = view(&fx, "odd", &[]);
        fx.mode("odd/locked", 0o755);
        insta::assert_snapshot!(out);
    }
}
