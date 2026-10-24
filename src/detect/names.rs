//! Exact name rules: names that say the type with no doubt.

use super::{LabelId, label_id};

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
    if let Some(&(_, l)) = NAMES.iter().find(|(n, _)| n.as_bytes() == name) {
        return Some(label_id(l));
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
