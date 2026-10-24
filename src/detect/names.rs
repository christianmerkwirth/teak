//! Exact name rules: names that say the type with no doubt.

use std::sync::OnceLock;

use super::{LabelId, label_id};
use crate::fnv::FnvMap;

const NAMES: &[(&str, &str)] = &[
    ("Makefile", "makefile"),
    ("makefile", "makefile"),
    ("GNUmakefile", "makefile"),
    ("Kbuild", "makefile"),
    ("Dockerfile", "dockerfile"),
    ("Containerfile", "dockerfile"),
    ("CMakeLists.txt", "cmake"),
    ("Kconfig", "kconfig"),
    ("Jenkinsfile", "groovy"),
    ("Vagrantfile", "ruby"),
    ("Rakefile", "ruby"),
    ("Gemfile", "ruby"),
    ("Podfile", "ruby"),
    ("Brewfile", "ruby"),
    ("BUILD", "python"),
    ("BUILD.bazel", "python"),
    ("WORKSPACE", "python"),
    ("SConstruct", "python"),
    ("justfile", "makefile"),
    ("Justfile", "makefile"),
    ("Cargo.lock", "toml"),
    ("Pipfile", "toml"),
    ("go.mod", "txt"),
    ("go.sum", "txt"),
    ("LICENSE", "txt"),
    ("LICENCE", "txt"),
    ("COPYING", "txt"),
    ("NOTICE", "txt"),
    ("AUTHORS", "txt"),
    ("CONTRIBUTORS", "txt"),
    ("README", "txt"),
    ("CHANGELOG", "txt"),
    ("MAINTAINERS", "txt"),
    ("CREDITS", "txt"),
    ("Procfile", "txt"),
    (".gitignore", "txt"),
    (".gitattributes", "txt"),
    (".gitmodules", "ini"),
    (".editorconfig", "ini"),
    (".dockerignore", "txt"),
    (".bashrc", "shell"),
    (".bash_profile", "shell"),
    (".zshrc", "shell"),
    (".profile", "shell"),
    (".env", "txt"),
    (".netrc", "txt"),
    ("id_rsa", "pem"),
    ("id_ed25519", "pem"),
    ("id_ecdsa", "pem"),
];

/// The label for an exact name, or for a name that starts like `Kconfig.`
/// or `Makefile.` (the kernel and many C trees use those).
pub fn by_name(name: &[u8]) -> Option<LabelId> {
    if !maybe(name) {
        return None;
    }
    static INDEX: OnceLock<FnvMap<&'static [u8], LabelId>> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        NAMES
            .iter()
            .map(|(n, l)| (n.as_bytes(), label_id(l)))
            .collect()
    });
    if let Some(&l) = index.get(name) {
        return Some(l);
    }
    let prefixed = |p: &[u8]| name.len() > p.len() && name.starts_with(p);
    if prefixed(b"Kconfig.") {
        return Some(label_id("kconfig"));
    }
    if prefixed(b"Makefile.") || prefixed(b"Dockerfile.") {
        let l = if name.starts_with(b"M") {
            "makefile"
        } else {
            "dockerfile"
        };
        return Some(label_id(l));
    }
    None
}

/// A quick reject: false when no rule can match. Per first byte, a bit
/// mask of the lengths of the exact names; the prefix rules start with `K`,
/// `M`, or `D`. Most names in a large tree fail here.
fn maybe(name: &[u8]) -> bool {
    static LENGTHS: OnceLock<[u64; 256]> = OnceLock::new();
    let lengths = LENGTHS.get_or_init(|| {
        let mut t = [0u64; 256];
        for (n, _) in NAMES {
            t[n.as_bytes()[0] as usize] |= 1 << n.len().min(63);
        }
        t
    });
    let Some(&first) = name.first() else {
        return false;
    };
    lengths[first as usize] & (1 << name.len().min(63)) != 0 || matches!(first, b'K' | b'M' | b'D')
}
