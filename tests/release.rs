//! Release: completions and the man page (FR-69), and the release config
//! (FR-68). The release pipeline itself runs in GitHub Actions.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

/// The asset generator from `examples/gen_assets.rs`, called in process.
#[path = "../examples/gen_assets.rs"]
#[allow(dead_code)]
mod gen_assets;

const ASSETS: [&str; 4] = ["teak.bash", "_teak", "teak.fish", "teak.1"];

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn generated() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    gen_assets::write_all(dir.path()).expect("gen_assets");
    dir
}

// FR-69: Completions and man page

#[test]
fn fr69_gen_assets_writes_all_four_files() {
    let dir = generated();
    for name in ASSETS {
        let text =
            fs::read_to_string(dir.path().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(text.contains("format"), "{name} names --format");
    }
}

#[test]
fn fr69_the_committed_assets_are_up_to_date() {
    // The release archive takes the files from `assets/`; they must match
    // the clap definitions. Run `cargo run --example gen_assets -- assets`
    // after a CLI change.
    let dir = generated();
    for name in ASSETS {
        let want = fs::read(dir.path().join(name)).unwrap();
        let have = fs::read(manifest_dir().join("assets").join(name))
            .unwrap_or_else(|e| panic!("assets/{name}: {e}"));
        assert!(
            want == have,
            "assets/{name} is stale: run `cargo run --example gen_assets -- assets`"
        );
    }
}

#[test]
fn fr69_the_man_page_renders_with_no_error() {
    if Command::new("man").arg("--version").output().is_err() {
        eprintln!("skipped: no man on this machine");
        return;
    }
    let dir = generated();
    let out = Command::new("man")
        .arg("./teak.1")
        .current_dir(dir.path())
        .env("MANPAGER", "cat")
        .env("PAGER", "cat")
        .env("MANWIDTH", "80")
        .env("MANROFFSEQ", "")
        .output()
        .expect("run man");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(stderr.trim().is_empty(), "{stderr}");
    // macOS man prints bold as overstrike (`N\bN`) even into a pipe, and
    // may print hyphens as U+2010.
    let mut text = String::new();
    for c in String::from_utf8_lossy(&out.stdout).chars() {
        match c {
            '\u{8}' => {
                text.pop();
            }
            '\u{2010}' | '\u{2212}' => text.push('-'),
            c => text.push(c),
        }
    }
    for want in ["teak", "--format", "--json-full", "config.toml"] {
        assert!(text.contains(want), "{want} in:\n{text}");
    }
}

/// The words that the bash completion offers for `teak <word>`.
fn bash_complete(script: &Path, word: &str) -> BTreeSet<String> {
    let cmd = format!(
        "source {script}; COMP_WORDS=(teak {word}); COMP_CWORD=1; \
         COMP_LINE='teak {word}'; COMP_POINT=${{#COMP_LINE}}; \
         _teak teak {word} teak; printf '%s\\n' \"${{COMPREPLY[@]}}\"",
        script = script.display()
    );
    let out = Command::new("bash")
        .args(["--norc", "--noprofile", "-c", &cmd])
        .output()
        .expect("run bash");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn fr69_bash_completes_fo() {
    let dir = generated();
    let got = bash_complete(&dir.path().join("teak.bash"), "--fo");
    let want: BTreeSet<String> = ["--focus", "--fold", "--follow", "--format"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(got, want);
}

#[test]
fn fr69_bash_completes_format_values() {
    let dir = generated();
    let cmd = format!(
        "source {}; COMP_WORDS=(teak --format ''); COMP_CWORD=2; \
         COMP_LINE='teak --format '; COMP_POINT=${{#COMP_LINE}}; \
         _teak teak '' --format; printf '%s\\n' \"${{COMPREPLY[@]}}\"",
        dir.path().join("teak.bash").display()
    );
    let out = Command::new("bash")
        .args(["--norc", "--noprofile", "-c", &cmd])
        .output()
        .unwrap();
    let got: BTreeSet<&str> = std::str::from_utf8(&out.stdout).unwrap().lines().collect();
    for v in ["tty", "plain", "md", "json"] {
        assert!(got.contains(v), "{v} in {got:?}");
    }
}

// FR-68: Install

/// The parsed `Cargo.toml`.
fn manifest() -> toml::Table {
    fs::read_to_string(manifest_dir().join("Cargo.toml"))
        .unwrap()
        .parse()
        .unwrap()
}

/// The cargo-dist settings.
fn dist() -> toml::Table {
    let m = manifest();
    m.get("workspace")
        .and_then(|w| w.get("metadata"))
        .and_then(|m| m.get("dist"))
        .and_then(toml::Value::as_table)
        .cloned()
        .expect("[workspace.metadata.dist] in Cargo.toml")
}

fn strings(v: &toml::Value) -> Vec<&str> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect()
}

#[test]
fn fr68_dist_builds_the_four_targets() {
    let d = dist();
    let targets = strings(&d["targets"]);
    for t in [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
    ] {
        assert!(targets.contains(&t), "{t} in {targets:?}");
    }
    let installers = strings(&d["installers"]);
    assert!(installers.contains(&"shell"), "{installers:?}");
    assert!(
        manifest_dir()
            .join(".github/workflows/release.yml")
            .is_file()
    );
}

#[test]
fn fr69_the_archive_includes_the_assets() {
    let d = dist();
    let include = strings(&d["include"]);
    for name in ASSETS {
        let path = format!("assets/{name}");
        assert!(include.contains(&path.as_str()), "{path} in {include:?}");
        assert!(manifest_dir().join(&path).is_file(), "{path}");
    }
}

#[test]
fn fr68_the_package_has_crates_io_metadata() {
    let m = manifest();
    let p = m["package"].as_table().unwrap();
    for key in [
        "description",
        "license",
        "repository",
        "readme",
        "keywords",
        "categories",
    ] {
        assert!(p.contains_key(key), "package.{key}");
    }
    assert!(manifest_dir().join("README.md").is_file());
}

#[test]
fn fr68_version_prints() {
    let out = Command::new(env!("CARGO_BIN_EXE_teak"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.starts_with(&format!("teak {} (magika: ", env!("CARGO_PKG_VERSION"))),
        "{text}"
    );
}

#[test]
fn changelog_records_the_v1_formats() {
    let text = fs::read_to_string(manifest_dir().join("CHANGELOG.md")).expect("CHANGELOG.md");
    assert!(text.contains("schema_version: 1"), "{text}");
    for f in ["`plain`", "`md`"] {
        assert!(text.contains(f), "{f} in the changelog");
    }
    let schema: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(manifest_dir().join("schema/teak.schema.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        schema["properties"]["schema_version"]["const"],
        teak::render::json::SCHEMA_VERSION
    );
}
