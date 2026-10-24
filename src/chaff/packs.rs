//! Project-kind rule packs: entries that are chaff only next to a marker
//! file (FR-32). `target/` is build output next to `Cargo.toml`; in a photo
//! folder it is a normal folder.

use super::ChaffClass::{self, *};

pub struct Pack {
    /// A file name, or `*.ext` for a suffix (for example `*.xcodeproj`).
    pub marker: &'static str,
    /// Names of siblings of the marker; a trailing `/` means a directory.
    pub entries: &'static [(&'static str, ChaffClass)],
}

const PYTHON: &[(&str, ChaffClass)] = &[
    ("venv/", Vendor),
    ("build/", Build),
    ("dist/", Build),
    (".tox/", Cache),
    (".nox/", Cache),
];

const GRADLE: &[(&str, ChaffClass)] = &[("build/", Build)];

pub static PACKS: &[Pack] = &[
    Pack {
        marker: "Cargo.toml",
        entries: &[("target/", Build)],
    },
    Pack {
        marker: "package.json",
        entries: &[
            ("dist/", Build),
            ("build/", Build),
            ("out/", Build),
            (".next/", Build),
            (".nuxt/", Build),
            (".svelte-kit/", Build),
            ("coverage/", Cache),
            (".turbo/", Cache),
            (".parcel-cache/", Cache),
        ],
    },
    Pack {
        marker: "pyproject.toml",
        entries: PYTHON,
    },
    Pack {
        marker: "setup.py",
        entries: PYTHON,
    },
    Pack {
        marker: "pom.xml",
        entries: &[("target/", Build)],
    },
    Pack {
        marker: "build.gradle",
        entries: GRADLE,
    },
    Pack {
        marker: "build.gradle.kts",
        entries: GRADLE,
    },
    Pack {
        marker: "build.sbt",
        entries: &[("target/", Build), ("project/target/", Build)],
    },
    Pack {
        marker: "CMakeLists.txt",
        entries: &[("build/", Build), ("cmake-build-debug/", Build)],
    },
    Pack {
        marker: "*.xcodeproj",
        entries: &[("DerivedData/", Build), ("build/", Build)],
    },
    Pack {
        marker: "mix.exs",
        entries: &[("_build/", Build), ("deps/", Vendor)],
    },
    Pack {
        marker: "pubspec.yaml",
        entries: &[("build/", Build), (".dart_tool/", Cache)],
    },
];

impl Pack {
    pub fn is_marker(&self, name: &[u8]) -> bool {
        match self.marker.strip_prefix('*') {
            Some(suffix) => name.len() > suffix.len() && name.ends_with(suffix.as_bytes()),
            None => name == self.marker.as_bytes(),
        }
    }

    /// The class of a sibling entry, if the pack names it.
    pub fn class_of(&self, name: &[u8], is_dir: bool) -> Option<ChaffClass> {
        self.entries.iter().find_map(|&(pat, class)| {
            let (pat, dir_only) = match pat.strip_suffix('/') {
                Some(p) => (p, true),
                None => (pat, false),
            };
            (pat.as_bytes() == name && dir_only == is_dir).then_some(class)
        })
    }
}
