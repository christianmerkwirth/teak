//! Boilerplate templates (FR-40, FR-41).
//!
//! A file matches a template when both texts normalize to the same bytes:
//! teak compares the FNV-1a hash of the normalized text. The built-in table
//! comes from `templates/boilerplate/`; `--templates DIR` adds more.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::ChaffClass;
use crate::cli::UsageError;
use crate::tree::io_message;

/// The built-in templates: (label, text).
static BUILTIN: &[(&str, &str)] = &[
    ("MIT", include_str!("../../templates/boilerplate/MIT.txt")),
    (
        "Apache-2.0",
        include_str!("../../templates/boilerplate/Apache-2.0.txt"),
    ),
    (
        "BSD-2-Clause",
        include_str!("../../templates/boilerplate/BSD-2-Clause.txt"),
    ),
    (
        "BSD-3-Clause",
        include_str!("../../templates/boilerplate/BSD-3-Clause.txt"),
    ),
    ("ISC", include_str!("../../templates/boilerplate/ISC.txt")),
    (
        "MPL-2.0",
        include_str!("../../templates/boilerplate/MPL-2.0.txt"),
    ),
    (
        "GPL-2.0",
        include_str!("../../templates/boilerplate/GPL-2.0.txt"),
    ),
    (
        "GPL-3.0",
        include_str!("../../templates/boilerplate/GPL-3.0.txt"),
    ),
    (
        "Unlicense",
        include_str!("../../templates/boilerplate/Unlicense.txt"),
    ),
    (
        "Contributor-Covenant-2.1",
        include_str!("../../templates/boilerplate/Contributor-Covenant-2.1.txt"),
    ),
    (
        "gitignore-Rust",
        include_str!("../../templates/boilerplate/gitignore-Rust.txt"),
    ),
    (
        "gitignore-Node",
        include_str!("../../templates/boilerplate/gitignore-Node.txt"),
    ),
    (
        "gitignore-Python",
        include_str!("../../templates/boilerplate/gitignore-Python.txt"),
    ),
];

/// Lower case; no lines that start with `copyright`; no 4-digit years; each
/// run of white space becomes one space; no space at either end.
///
/// Post: the MIT text with any year and any holder gives the same bytes as
/// the built-in MIT template.
pub fn normalize(text: &[u8]) -> Vec<u8> {
    let lower = text.to_ascii_lowercase();
    let mut kept: Vec<u8> = Vec::with_capacity(lower.len());
    for line in lower.split(|&b| b == b'\n') {
        if !line.trim_ascii_start().starts_with(b"copyright") {
            kept.extend_from_slice(line);
            kept.push(b'\n');
        }
    }
    let mut out = Vec::with_capacity(kept.len());
    let mut i = 0;
    while i < kept.len() {
        let b = kept[i];
        if b.is_ascii_digit() {
            let end = i + kept[i..].iter().take_while(|b| b.is_ascii_digit()).count();
            let digits = &kept[i..end];
            let year =
                digits.len() == 4 && (digits.starts_with(b"19") || digits.starts_with(b"20"));
            if !year {
                out.extend_from_slice(digits);
            }
            i = end;
            continue;
        }
        if b.is_ascii_whitespace() {
            if out.last().is_some_and(|&l| l != b' ') {
                out.push(b' ');
            }
        } else {
            out.push(b);
        }
        i += 1;
    }
    while out.last() == Some(&b' ') {
        out.pop();
    }
    out
}

/// FNV-1a, 64 bit: a fixed hash that does not change between Rust versions.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

struct Template {
    label: String,
    class: ChaffClass,
    hash: u64,
}

/// A template table. The id of a template is its index + 1, so 0 can mean
/// no template in a `ChaffTag`.
#[derive(Default)]
pub struct Templates {
    list: Vec<Template>,
    /// Normalized hash → index. A later template with the same text wins.
    index: HashMap<u64, usize>,
}

impl Templates {
    pub fn builtin() -> Templates {
        let mut t = Templates::default();
        for (label, text) in BUILTIN {
            t.push(label.to_string(), ChaffClass::Boilerplate, text.as_bytes());
        }
        t
    }

    /// The table for the options: the built-in table unless
    /// `--templates-only`, then each `--templates DIR` in order.
    pub fn for_opts(dirs: &[PathBuf], only: bool) -> Result<Templates, UsageError> {
        let mut t = if only {
            Templates::default()
        } else {
            Templates::builtin()
        };
        for dir in dirs {
            t.load_into(dir)?;
        }
        Ok(t)
    }

    /// The templates in `dir`: `boilerplate/<label>.txt` and
    /// `generated/<label>.txt`; a file at the top level is boilerplate.
    pub fn load(dir: &Path) -> Result<Templates, UsageError> {
        let mut t = Templates::default();
        t.load_into(dir)?;
        Ok(t)
    }

    fn load_into(&mut self, dir: &Path) -> Result<(), UsageError> {
        let cannot = |e: &std::io::Error| {
            UsageError(format!(
                "cannot read template directory '{}': {}",
                dir.display(),
                io_message(e)
            ))
        };
        let mut files: Vec<(PathBuf, ChaffClass)> = Vec::new();
        for entry in fs::read_dir(dir).map_err(|e| cannot(&e))? {
            let path = entry.map_err(|e| cannot(&e))?.path();
            if path.is_file() {
                files.push((path, ChaffClass::Boilerplate));
                continue;
            }
            let class = match path.file_name().and_then(|n| n.to_str()) {
                Some("boilerplate") => ChaffClass::Boilerplate,
                Some("generated") => ChaffClass::Generated,
                _ => continue,
            };
            for sub in fs::read_dir(&path).map_err(|e| cannot(&e))? {
                let p = sub.map_err(|e| cannot(&e))?.path();
                if p.is_file() {
                    files.push((p, class));
                }
            }
        }
        files.sort();
        for (path, class) in files {
            let text = fs::read(&path).map_err(|e| {
                UsageError(format!(
                    "cannot read template '{}': {}",
                    path.display(),
                    io_message(&e)
                ))
            })?;
            let label = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.push(label, class, &text);
        }
        Ok(())
    }

    fn push(&mut self, label: String, class: ChaffClass, text: &[u8]) {
        let hash = fnv1a64(&normalize(text));
        self.index.insert(hash, self.list.len());
        self.list.push(Template { label, class, hash });
    }

    /// The label and class of the template with this normalized hash.
    pub fn lookup(&self, hash: u64) -> Option<(&str, ChaffClass)> {
        let t = &self.list[*self.index.get(&hash)?];
        Some((t.label.as_str(), t.class))
    }

    /// The id (index + 1) and class of the template with this hash.
    pub fn find(&self, hash: u64) -> Option<(u16, ChaffClass)> {
        let i = *self.index.get(&hash)?;
        Some((i as u16 + 1, self.list[i].class))
    }

    /// The labels, by id - 1.
    pub fn labels(&self) -> Vec<String> {
        self.list.iter().map(|t| t.label.clone()).collect()
    }

    /// A hash of the whole table. A cached check result is valid only for
    /// the table that made it.
    pub fn fingerprint(&self) -> u64 {
        let mut bytes = Vec::new();
        for t in &self.list {
            bytes.extend_from_slice(t.label.as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(t.class.name().as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(&t.hash.to_le_bytes());
        }
        fnv1a64(&bytes)
    }

    /// `--dump-templates DIR`: writes the built-in texts to
    /// `DIR/boilerplate/<label>.txt`. It checks every target first, and
    /// writes nothing when one exists.
    pub fn dump(dir: &Path) -> Result<(), UsageError> {
        if !dir.is_dir() {
            return Err(UsageError(format!(
                "cannot write templates: '{}' is not a directory",
                dir.display()
            )));
        }
        let sub = dir.join("boilerplate");
        let targets: Vec<(PathBuf, &str)> = BUILTIN
            .iter()
            .map(|(label, text)| (sub.join(format!("{label}.txt")), *text))
            .collect();
        if let Some((path, _)) = targets
            .iter()
            .find(|(p, _)| fs::symlink_metadata(p).is_ok())
        {
            return Err(UsageError(format!(
                "will not overwrite '{}'; nothing written",
                path.display()
            )));
        }
        let failed = |p: &Path, e: std::io::Error| {
            UsageError(format!(
                "cannot write '{}': {}",
                p.display(),
                io_message(&e)
            ))
        };
        fs::create_dir_all(&sub).map_err(|e| failed(&sub, e))?;
        for (path, text) in targets {
            fs::write(&path, text).map_err(|e| failed(&path, e))?;
        }
        Ok(())
    }
}
