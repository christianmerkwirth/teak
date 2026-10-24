//! Global name rules (PRD §6.1), and the user rules from the config files.
//!
//! A global pattern ending in `/` matches a directory; `*x` matches a name
//! that ends in `x`; any other pattern is an exact file name. `LICENSE` is
//! not chaff by name: task-3 adds template matching.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::OnceLock;

use globset::{GlobBuilder, GlobMatcher};

use super::ChaffClass::{self, *};

pub static GLOBAL: &[(&str, ChaffClass)] = &[
    ("node_modules/", Vendor),
    ("vendor/", Vendor),
    (".venv/", Vendor),
    ("site-packages/", Vendor),
    ("Pods/", Vendor),
    ("bower_components/", Vendor),
    ("*.o", Build),
    ("*.obj", Build),
    ("*.class", Build),
    ("*.pyc", Build),
    ("*.pyo", Build),
    ("__pycache__/", Cache),
    (".cache/", Cache),
    (".pytest_cache/", Cache),
    (".mypy_cache/", Cache),
    (".ruff_cache/", Cache),
    (".gradle/", Cache),
    (".sass-cache/", Cache),
    (".eslintcache", Cache),
    (".git/", Vcs),
    (".git", Vcs),
    (".hg/", Vcs),
    (".svn/", Vcs),
    (".bzr/", Vcs),
    (".idea/", Editor),
    (".vscode/", Editor),
    (".settings/", Editor),
    ("*.swp", Editor),
    ("*.swo", Editor),
    ("*~", Editor),
    (".DS_Store", Editor),
    ("Thumbs.db", Editor),
    ("desktop.ini", Editor),
    ("Cargo.lock", Lock),
    ("package-lock.json", Lock),
    ("yarn.lock", Lock),
    ("pnpm-lock.yaml", Lock),
    ("poetry.lock", Lock),
    ("Pipfile.lock", Lock),
    ("uv.lock", Lock),
    ("Gemfile.lock", Lock),
    ("composer.lock", Lock),
    ("go.sum", Lock),
    ("flake.lock", Lock),
    ("mix.lock", Lock),
    ("bun.lockb", Lock),
    ("*_pb2.py", Generated),
    ("*_pb2_grpc.py", Generated),
    ("*.pb.go", Generated),
    ("*.pb.cc", Generated),
    ("*.pb.h", Generated),
    ("*.min.js", Minified),
    ("*.min.css", Minified),
    ("*.map", Minified),
    (".gitignore", Boilerplate),
    (".gitattributes", Boilerplate),
    (".editorconfig", Boilerplate),
    (".prettierrc", Boilerplate),
    (".prettierignore", Boilerplate),
    (".dockerignore", Boilerplate),
    (".npmignore", Boilerplate),
    ("CODE_OF_CONDUCT.md", Boilerplate),
    (".gitkeep", Empty),
    (".keep", Empty),
];

/// The compiled form of [`GLOBAL`]: exact names in maps, suffixes in a list.
struct Compiled {
    files: HashMap<&'static [u8], ChaffClass>,
    dirs: HashMap<&'static [u8], ChaffClass>,
    suffixes: Vec<(&'static [u8], ChaffClass)>,
}

fn compiled() -> &'static Compiled {
    static C: OnceLock<Compiled> = OnceLock::new();
    C.get_or_init(|| {
        let mut c = Compiled {
            files: HashMap::new(),
            dirs: HashMap::new(),
            suffixes: Vec::new(),
        };
        for &(pat, class) in GLOBAL {
            if let Some(dir) = pat.strip_suffix('/') {
                c.dirs.insert(dir.as_bytes(), class);
            } else if let Some(suffix) = pat.strip_prefix('*') {
                c.suffixes.push((suffix.as_bytes(), class));
            } else {
                c.files.insert(pat.as_bytes(), class);
            }
        }
        c
    })
}

/// The class from the global name rules.
pub fn by_name(name: &[u8], is_dir: bool) -> Option<ChaffClass> {
    let c = compiled();
    if is_dir {
        return c.dirs.get(name).copied();
    }
    if let Some(&class) = c.files.get(name) {
        return Some(class);
    }
    c.suffixes
        .iter()
        .find(|(s, _)| name.len() > s.len() && name.ends_with(s))
        .map(|&(_, class)| class)
}

/// The [`GLOBAL`] pattern that gives [`by_name`] its answer, for `--why`.
pub fn pattern_of(name: &[u8], is_dir: bool) -> Option<&'static str> {
    let exact = |want_dir: bool| {
        GLOBAL.iter().find_map(|&(pat, _)| {
            let (p, dir) = match pat.strip_suffix('/') {
                Some(p) => (p, true),
                None => (pat, false),
            };
            (dir == want_dir && !p.starts_with('*') && p.as_bytes() == name).then_some(pat)
        })
    };
    if is_dir {
        return exact(true);
    }
    exact(false).or_else(|| {
        GLOBAL.iter().find_map(|&(pat, _)| {
            let suffix = pat.strip_prefix('*')?;
            (name.len() > suffix.len() && name.ends_with(suffix.as_bytes())).then_some(pat)
        })
    })
}

/// A `[[chaff.rules]]` entry of a config file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserRule {
    pub pattern: String,
    pub class: ChaffClass,
}

/// The compiled user rules, in config order: the first match wins.
///
/// Patterns are globs, as in `.gitignore`: a trailing `/` matches only a
/// directory; a pattern with a `/` matches the path from the scan root;
/// any other pattern matches the name. `*` does not cross a `/`; `**` does.
#[derive(Default)]
pub struct UserRules {
    rules: Vec<UserRuleMatcher>,
}

struct UserRuleMatcher {
    matcher: GlobMatcher,
    dir_only: bool,
    anchored: bool,
    class: ChaffClass,
    pattern: String,
}

impl UserRules {
    pub fn new(rules: &[UserRule]) -> Result<UserRules, String> {
        let mut out = Vec::new();
        for r in rules {
            let (body, dir_only) = match r.pattern.strip_suffix('/') {
                Some(b) => (b, true),
                None => (r.pattern.as_str(), false),
            };
            let anchored = body.contains('/');
            let body = body.trim_start_matches('/');
            let glob = GlobBuilder::new(body)
                .literal_separator(true)
                .build()
                .map_err(|e| format!("bad pattern `{}`: {}", r.pattern, e.kind()))?;
            out.push(UserRuleMatcher {
                matcher: glob.compile_matcher(),
                dir_only,
                anchored,
                class: r.class,
                pattern: r.pattern.clone(),
            });
        }
        Ok(UserRules { rules: out })
    }

    /// The class and the pattern of the first rule that matches. `rel` is
    /// the path from the scan root.
    pub fn find(&self, rel: &[u8], name: &[u8], is_dir: bool) -> Option<(ChaffClass, &str)> {
        self.rules.iter().find_map(|r| {
            if r.dir_only && !is_dir {
                return None;
            }
            let subject = if r.anchored { rel } else { name };
            r.matcher
                .is_match(Path::new(OsStr::from_bytes(subject)))
                .then_some((r.class, r.pattern.as_str()))
        })
    }
}
