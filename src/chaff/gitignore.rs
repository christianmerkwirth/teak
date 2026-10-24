//! `.gitignore` and `.git/info/exclude`, matched by teak itself, so ignored
//! entries are still walked and counted (design.md §Walk all entries).

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

/// The rules of one directory. Paths are relative to the scan root.
pub struct DirRules {
    matcher: Gitignore,
}

impl DirRules {
    /// Builds the rules of the directory at `dir` (relative to the scan
    /// root) from rule file texts. `None` when no line is a rule.
    pub fn new(dir: &[u8], texts: &[(&Path, &[u8])]) -> Option<DirRules> {
        let mut b = GitignoreBuilder::new(Path::new(OsStr::from_bytes(dir)));
        let mut any = false;
        for (from, text) in texts {
            for line in String::from_utf8_lossy(text).lines() {
                if b.add_line(Some(from.to_path_buf()), line).is_ok() {
                    any = true;
                }
            }
        }
        let matcher = b.build().ok()?;
        (any && !matcher.is_empty()).then_some(DirRules { matcher })
    }

    /// `Some(true)` when ignored, `Some(false)` when a `!` rule takes it
    /// back, `None` when no rule matches.
    pub fn matched(&self, rel: &[u8], is_dir: bool) -> Option<bool> {
        match self
            .matcher
            .matched(Path::new(OsStr::from_bytes(rel)), is_dir)
        {
            Match::Ignore(_) => Some(true),
            Match::Whitelist(_) => Some(false),
            Match::None => None,
        }
    }
}
