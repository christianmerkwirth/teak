//! Config files (FR-66) and salience weights (FR-67).

mod support;

use std::fs;
use std::path::Path;

use support::{DEFAULT_FLAGS, Fixture, Run, command, word};

/// Runs teak in `dir` with `home` as `$HOME` and no `XDG_CONFIG_HOME`, so
/// the user file is `<home>/.config/teak/config.toml`.
fn run_home(dir: &Path, home: &Path, args: &[&str]) -> Run {
    Run::from_output(
        command(dir)
            .env_remove("XDG_CONFIG_HOME")
            .env("HOME", home)
            .args(DEFAULT_FLAGS)
            .args(args)
            .output()
            .expect("run teak"),
    )
}

/// Runs teak in `dir` with `config` as `XDG_CONFIG_HOME`.
fn run_xdg(dir: &Path, config: &Path, args: &[&str]) -> Run {
    Run::from_output(
        command(dir)
            .env("XDG_CONFIG_HOME", config)
            .args(DEFAULT_FLAGS)
            .args(args)
            .output()
            .expect("run teak"),
    )
}

/// Writes `<dir>/teak/config.toml` or another file, with parents.
fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// 60 files, so a budget of 30, 40, or 50 is full.
fn wide() -> Fixture {
    let fx = Fixture::new();
    for i in 0..60 {
        fx.sized(&format!("d{:02}/{}.txt", i % 12, word(i)), 100 + i as u64);
    }
    fx
}

fn budget(out: &Run) -> String {
    assert_eq!(out.code, Some(0), "{out:?}");
    let footer = out.footer();
    let of = footer.split(" of ").nth(1).expect("`of N lines`");
    of.split(' ').next().unwrap().to_string()
}

// FR-66: Config files

#[test]
fn fr66_flags_then_project_then_user() {
    let fx = wide();
    let home = tempfile::tempdir().unwrap();
    write(
        &home.path().join(".config/teak/config.toml"),
        "lines = 50\n",
    );
    write(&fx.abs(".teak.toml"), "lines = 40\n");
    assert_eq!(
        budget(&run_home(fx.path(), home.path(), &["-n", "30"])),
        "30"
    );
    assert_eq!(budget(&run_home(fx.path(), home.path(), &[])), "40");
    fs::remove_file(fx.abs(".teak.toml")).unwrap();
    assert_eq!(budget(&run_home(fx.path(), home.path(), &[])), "50");
}

#[test]
fn fr66_xdg_config_home_holds_the_user_file() {
    let fx = wide();
    let config = tempfile::tempdir().unwrap();
    write(&config.path().join("teak/config.toml"), "lines = 45\n");
    assert_eq!(budget(&run_xdg(fx.path(), config.path(), &[])), "45");
}

#[test]
fn fr66_a_project_file_in_a_parent_directory() {
    let fx = wide();
    write(&fx.abs(".teak.toml"), "lines = 40\n");
    fx.dir("sub/deeper");
    fx.file("sub/deeper/x.txt", "x\n");
    // The root is below the project file.
    let out = support::run(&fx, &["sub/deeper"]);
    assert_eq!(budget(&out), "40");
    let out = support::run_in(&fx.abs("sub"), &["deeper"]);
    assert_eq!(budget(&out), "40");
    // The nearest file wins.
    write(&fx.abs("sub/.teak.toml"), "lines = 35\n");
    let out = support::run(&fx, &["sub/deeper"]);
    assert_eq!(budget(&out), "35");
}

#[test]
fn fr66_a_syntax_error_names_the_file_and_line() {
    let fx = wide();
    write(&fx.abs(".teak.toml"), "lines = 40\nsort = = \"name\"\n");
    let out = support::run(&fx, &[]);
    assert_eq!(out.code, Some(2), "{out:?}");
    assert_eq!(out.stdout, "");
    let err = out.stderr_lines();
    assert_eq!(err.len(), 1, "{}", out.stderr);
    assert!(err[0].contains(".teak.toml"), "{}", err[0]);
    assert!(err[0].contains("line 2"), "{}", err[0]);
}

#[test]
fn fr66_a_syntax_error_in_the_user_file() {
    let fx = wide();
    let config = tempfile::tempdir().unwrap();
    let file = config.path().join("teak/config.toml");
    write(&file, "[weights\nsize = 1\n");
    let out = run_xdg(fx.path(), config.path(), &[]);
    assert_eq!(out.code, Some(2), "{out:?}");
    assert!(
        out.stderr.contains(&*file.to_string_lossy()),
        "{}",
        out.stderr
    );
    assert!(out.stderr.contains("line 1"), "{}", out.stderr);
}

#[test]
fn fr66_an_unknown_key_is_one_warning() {
    let fx = wide();
    write(
        &fx.abs(".teak.toml"),
        "lines = 40\ncolour = \"never\"\n[chaff]\nshow = [\"lock\"]\n",
    );
    let out = support::run(&fx, &[]);
    assert_eq!(budget(&out), "40");
    let err = out.stderr_lines();
    assert_eq!(err.len(), 1, "{}", out.stderr);
    assert!(err[0].contains("colour"), "{}", err[0]);
    assert!(err[0].contains(".teak.toml"), "{}", err[0]);
}

#[test]
fn fr66_bad_values_are_usage_errors() {
    for text in [
        "lines = 2\n",
        "lines = \"many\"\n",
        "sort = \"bogus\"\n",
        "format = 3\n",
        "width = 4\n",
        "[chaff]\nshow = [\"nothing\"]\n",
        "[[chaff.rules]]\npattern = \"a[\"\nclass = \"build\"\n",
        "[[chaff.rules]]\npattern = \"gen/\"\nclass = \"stuff\"\n",
        "[weights]\nsize = -1\n",
        "[weights]\nsize = \"big\"\n",
    ] {
        let fx = wide();
        write(&fx.abs(".teak.toml"), text);
        let out = support::run(&fx, &[]);
        assert_eq!(out.code, Some(2), "{text}: {out:?}");
        assert!(out.stderr.contains(".teak.toml"), "{text}: {}", out.stderr);
    }
}

#[test]
fn fr66_config_keys_set_the_defaults() {
    let fx = Fixture::new();
    fx.file("Cargo.lock", "# lock\n")
        .file("main.rs", "fn main() {}\n")
        .file("docs/gen/api.md", "# API\n")
        .file("docs/guide.md", "# Guide\n");
    write(
        &fx.abs(".teak.toml"),
        r#"format = "json"
preview = "off"

[chaff]
show = ["lock"]

[[chaff.rules]]
pattern = "docs/gen/"
class = "generated"
"#,
    );
    // The test flags set `--format plain`, so run without them.
    let out = Run::from_output(command(fx.path()).output().unwrap());
    assert_eq!(out.code, Some(0), "{out:?}");
    let doc: serde_json::Value = serde_json::from_str(&out.stdout).expect("json from the config");
    let rows = doc["rows"].as_array().unwrap();
    let lock = rows.iter().find(|r| r["path"] == "Cargo.lock").unwrap();
    assert_eq!(lock["kind"], "entry");
    assert_eq!(lock["class"], "lock");
    let main = rows.iter().find(|r| r["path"] == "main.rs").unwrap();
    assert!(main.get("preview").is_none(), "{main}");
    let chaff = rows
        .iter()
        .find(|r| r["kind"] == "chaff" && r["path"] == "docs/")
        .unwrap_or_else(|| panic!("a docs/ chaff row: {doc:#}"));
    assert_eq!(chaff["items"][0]["path"], "docs/gen/");
    assert_eq!(chaff["items"][0]["class"], "generated");
    // A flag wins over the config.
    let out = support::run(&fx, &["--fold=lock"]);
    assert!(out.row("Cargo.lock").is_none(), "{}", out.stdout);
}

#[test]
fn fr66_a_user_rule_wins_over_gitattributes() {
    let fx = Fixture::new();
    fx.file(".gitattributes", "vendor/** -linguist-vendored\n")
        .file("vendor/lib.js", "x\n")
        .file("main.rs", "fn main() {}\n");
    write(
        &fx.abs(".teak.toml"),
        "[[chaff.rules]]\npattern = \"*.js\"\nclass = \"minified\"\n",
    );
    let s = support::run(&fx, &["--why", "vendor/lib.js"]);
    assert_eq!(s.code, Some(0), "{s:?}");
    assert!(s.stdout.contains("user rule: *.js"), "{}", s.stdout);
}

#[test]
fn fr66_config_templates_resolve_from_the_config_file() {
    let fx = Fixture::new();
    fx.file("NOTICE", "Acme internal notice.\nDo not share.\n")
        .file("main.rs", "fn main() {}\n");
    let cfg = tempfile::tempdir().unwrap();
    write(
        &cfg.path().join("teak/tpl/boilerplate/acme.txt"),
        "Acme internal notice.\nDo not share.\n",
    );
    write(
        &cfg.path().join("teak/config.toml"),
        "templates = [\"tpl\"]\n",
    );
    let out = run_xdg(fx.path(), cfg.path(), &[]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(out.row("NOTICE").is_none(), "{}", out.stdout);
}

// FR-67: Salience weights

/// The salience score that `--why` prints for `path`.
fn score(out: &Run) -> f32 {
    assert_eq!(out.code, Some(0), "{out:?}");
    let line = out
        .lines()
        .into_iter()
        .find(|l| l.starts_with("salience:"))
        .unwrap_or_else(|| panic!("no salience line: {}", out.stdout));
    line.rsplit("= ").next().unwrap().parse().unwrap()
}

#[test]
fn fr67_weights_change_what_opens() {
    let fx = Fixture::new();
    fx.sized("a/old.txt", 1000);
    for i in 0..50 {
        fx.sized(&format!("z/deep/deeper/f{}.dat", word(i)), 200);
    }
    fx.mtime_all(3 * 365 * 86_400);
    fx.mtime("a/old.txt", 60);
    fx.mtime("a", 60);
    let cfg = tempfile::tempdir().unwrap();
    write(
        &cfg.path().join("teak/config.toml"),
        "[weights]\nsize = 0\nrecency = 1\n",
    );
    let out = run_xdg(fx.path(), cfg.path(), &["-n", "12"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    // a/ opens, and its file shows.
    out.must_row("old.txt");
    // The weights reach the score: with the defaults, size carries z/; with
    // size 0 and recency 1, only the age counts.
    let z_default = score(&support::run(&fx, &["--why", "z", "-n", "12"]));
    let z = score(&run_xdg(fx.path(), cfg.path(), &["--why", "z", "-n", "12"]));
    let a = score(&run_xdg(fx.path(), cfg.path(), &["--why", "a", "-n", "12"]));
    assert!(z_default > 0.25, "{z_default}");
    assert!(z < 0.1, "{z}");
    assert!(a > 0.8, "{a}");
}
