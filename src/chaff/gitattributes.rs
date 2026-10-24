//! `.gitattributes`: `linguist-generated` and `linguist-vendored` (FR-31).
//!
//! For one attribute, the lines of a file act like gitignore lines: a set
//! value is a pattern, an unset value (`-attr`, `attr=false`) is a `!`
//! pattern, and the last match wins. A pattern `dir/**` also marks `dir`.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

/// The two linguist attributes of one `.gitattributes` file.
pub struct Attrs {
    generated: Gitignore,
    vendored: Gitignore,
}

impl Attrs {
    /// Parses the file of the directory at `dir` (relative to the root).
    pub fn parse(dir: &[u8], text: &[u8]) -> Option<Attrs> {
        let root = Path::new(OsStr::from_bytes(dir));
        let mut generated = GitignoreBuilder::new(root);
        let mut vendored = GitignoreBuilder::new(root);
        let mut any = false;
        for line in String::from_utf8_lossy(text).lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let Some(pattern) = parts.next() else {
                continue;
            };
            for attr in parts {
                let (name, set) = parse_attr(attr);
                let builder = match name {
                    "linguist-generated" => &mut generated,
                    "linguist-vendored" => &mut vendored,
                    _ => continue,
                };
                let Some(set) = set else { continue };
                any = true;
                let bang = if set { "" } else { "!" };
                builder.add_line(None, &format!("{bang}{pattern}")).ok();
                if let Some(dir) = pattern.strip_suffix("/**") {
                    builder.add_line(None, &format!("{bang}{dir}")).ok();
                }
            }
        }
        if !any {
            return None;
        }
        Some(Attrs {
            generated: generated.build().ok()?,
            vendored: vendored.build().ok()?,
        })
    }

    /// `Some(true)` set, `Some(false)` unset, `None` no line matches.
    pub fn generated(&self, rel: &[u8], is_dir: bool) -> Option<bool> {
        lookup(&self.generated, rel, is_dir)
    }

    pub fn vendored(&self, rel: &[u8], is_dir: bool) -> Option<bool> {
        lookup(&self.vendored, rel, is_dir)
    }
}

fn lookup(g: &Gitignore, rel: &[u8], is_dir: bool) -> Option<bool> {
    match g.matched(Path::new(OsStr::from_bytes(rel)), is_dir) {
        Match::Ignore(_) => Some(true),
        Match::Whitelist(_) => Some(false),
        Match::None => None,
    }
}

/// `attr` and `attr=true` set; `-attr` and `attr=false` unset; `!attr`
/// (back to unspecified) gives `None`.
fn parse_attr(attr: &str) -> (&str, Option<bool>) {
    if let Some(name) = attr.strip_prefix('-') {
        return (name, Some(false));
    }
    if let Some(name) = attr.strip_prefix('!') {
        return (name, None);
    }
    match attr.split_once('=') {
        Some((name, "false")) => (name, Some(false)),
        Some((name, _)) => (name, Some(true)),
        None => (attr, Some(true)),
    }
}
