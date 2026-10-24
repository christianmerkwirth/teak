//! Writes the shell completions and the man page from the clap definitions
//! (FR-69): `teak.bash`, `_teak` (zsh), `teak.fish`, and `teak.1`.
//!
//! ```text
//! cargo run --example gen_assets -- assets
//! ```
//!
//! The release archive takes these files from `assets/`, and a test checks
//! that `assets/` matches the CLI. Tests call [`write_all`] in process.

use std::fs;
use std::io;
use std::path::Path;

use clap_complete::Shell;
use clap_mangen::roff::{Roff, bold, italic, roman};

/// Writes the four files into `dir` and creates it when needed.
pub fn write_all(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let mut cmd = teak::cli::command().version(env!("CARGO_PKG_VERSION"));
    for (shell, file) in [
        (Shell::Bash, "teak.bash"),
        (Shell::Zsh, "_teak"),
        (Shell::Fish, "teak.fish"),
    ] {
        let mut buf = Vec::new();
        clap_complete::generate(shell, &mut cmd, "teak", &mut buf);
        fs::write(dir.join(file), buf)?;
    }
    fs::write(dir.join("teak.1"), man_page(cmd)?)
}

/// The man page: the clap sections, then files, exit status, and the
/// environment.
fn man_page(cmd: clap::Command) -> io::Result<Vec<u8>> {
    let man = clap_mangen::Man::new(cmd).manual("teak manual");
    let mut buf = Vec::new();
    man.render_title(&mut buf)?;
    man.render_name_section(&mut buf)?;
    man.render_synopsis_section(&mut buf)?;
    man.render_description_section(&mut buf)?;
    man.render_options_section(&mut buf)?;

    let mut roff = Roff::default();
    roff.control("SH", ["FILES"]);
    for (path, text) in [
        (
            "~/.config/teak/config.toml",
            "The user config. $XDG_CONFIG_HOME/teak/config.toml when XDG_CONFIG_HOME is set.",
        ),
        (
            ".teak.toml",
            "The project config: the nearest one in the root or a parent directory.",
        ),
    ] {
        roff.control("TP", []);
        roff.text([italic(path)]);
        roff.text([roman(text)]);
    }
    roff.text([roman(
        "Flags override the project file; the project file overrides the user file. \
         Keys: lines, depth, sort, series_min, detect, preview, color, width, time, \
         format, templates, [chaff] show/fold/hide, [[chaff.rules]] pattern and class, \
         and [weights] size, recency, git, rarity, role, anomaly. \
         A syntax error or a bad value exits with code 2; an unknown key is a warning.",
    )]);
    roff.control("SH", ["EXIT STATUS"]);
    for (code, text) in [
        ("0", "Success."),
        (
            "1",
            "Success with errors: a missing root, or entries that teak cannot read.",
        ),
        ("2", "A usage error: a bad flag value or a bad config file."),
    ] {
        roff.control("TP", []);
        roff.text([bold(code)]);
        roff.text([roman(text)]);
    }
    roff.control("SH", ["ENVIRONMENT"]);
    for (var, text) in [
        (
            "SOURCE_DATE_EPOCH",
            "The scan time, so relative ages and the output are fixed.",
        ),
        ("NO_COLOR", "No colors, unless --color always."),
        ("CLICOLOR_FORCE", "Colors, also in a pipe."),
        ("LS_COLORS", "The colors of the names."),
        (
            "XDG_CACHE_HOME",
            "The type cache lives in its teak directory.",
        ),
    ] {
        roff.control("TP", []);
        roff.text([bold(var)]);
        roff.text([roman(text)]);
    }
    roff.to_writer(&mut buf)?;
    man.render_version_section(&mut buf)?;
    Ok(buf)
}

fn main() {
    let Some(dir) = std::env::args_os().nth(1) else {
        eprintln!("usage: gen_assets <out-dir>");
        std::process::exit(2);
    };
    if let Err(e) = write_all(Path::new(&dir)) {
        eprintln!("gen_assets: {e}");
        std::process::exit(1);
    }
}
