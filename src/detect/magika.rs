//! Magika: the model behind `--detect full`.
//!
//! The session is made on the first file that needs the model, so a run
//! where every type comes from names or the cache does not pay for the
//! model load. Builds without the `magika` feature have the same API; there
//! [`Model::identify`] always gives `None`, so `full` acts as `fast`.

use std::path::Path;

use super::LabelId;

/// One Magika answer.
pub struct Identified {
    pub label: LabelId,
    pub score: f32,
    /// Magika had no real answer: a rule result (tiny or white-space-only
    /// files), or a low score that it turned into `txt` or `unknown`.
    pub generic: bool,
    /// The first block that Magika read (at most 4 KiB), for the secret check
    /// and the fast fallback.
    pub head: Vec<u8>,
}

#[derive(Default)]
pub struct Model {
    #[cfg(feature = "magika")]
    session: Option<::magika::Session>,
    /// The session could not be made; do not try again.
    #[cfg(feature = "magika")]
    broken: bool,
}

impl Model {
    pub fn new() -> Model {
        Model::default()
    }

    /// The model name for `--version` and the cache header.
    pub fn version() -> &'static str {
        #[cfg(feature = "magika")]
        return ::magika::MODEL_NAME;
        #[cfg(not(feature = "magika"))]
        return "none";
    }

    /// True when this build has Magika.
    pub fn available() -> bool {
        cfg!(feature = "magika")
    }

    /// Types one regular file. All reads go through `io::MagikaInput`.
    /// `None` when the file cannot be read or the model is not there.
    #[cfg(feature = "magika")]
    pub fn identify(&mut self, path: &Path) -> Option<Identified> {
        use ::magika::{FeaturesOrRuled, FileType};
        if self.broken {
            return None;
        }
        let mut input = crate::io::MagikaInput::open(path).ok()?;
        #[cfg(feature = "test-hooks")]
        crate::io::hooks::count("magika");
        let extracted = FeaturesOrRuled::extract_sync(&mut input).ok()?;
        let head = input.into_head();
        let features = match extracted {
            FeaturesOrRuled::Ruled(ct) => {
                return Some(Identified {
                    label: label_of(ct.info().label),
                    score: 1.0,
                    generic: true,
                    head,
                });
            }
            FeaturesOrRuled::Features(f) => f,
        };
        if self.session.is_none() {
            match ::magika::Session::new() {
                Ok(s) => self.session = Some(s),
                Err(_) => {
                    self.broken = true;
                    return None;
                }
            }
        }
        let ty = self
            .session
            .as_mut()?
            .identify_features_sync(&features)
            .ok()?;
        let low = matches!(
            &ty,
            FileType::Inferred(x) if matches!(x.content_type, Some((_, ::magika::OverwriteReason::LowConfidence)))
        );
        let label = label_of(ty.info().label);
        let name = super::label(label).name;
        Some(Identified {
            label,
            score: ty.score(),
            generic: low || matches!(name, "txt" | "unknown" | "empty"),
            head,
        })
    }

    #[cfg(not(feature = "magika"))]
    pub fn identify(&mut self, _path: &Path) -> Option<Identified> {
        None
    }
}

/// Magika names that teak spells another way.
const ALIASES: &[(&str, &str)] = &[
    ("sevenzip", "7zip"),
    ("bzip", "bzip2"),
    ("pythonbytecode", "pyc"),
    ("ttf", "font"),
    ("otf", "font"),
    ("woff", "font"),
    ("woff2", "font"),
    ("latex", "tex"),
    ("directory", "dir"),
    ("undefined", "unknown"),
    ("qt", "mov"),
    ("gradle", "groovy"),
    ("gemfile", "ruby"),
    ("gemspec", "ruby"),
];

/// The teak label for a Magika label name. Every Magika label is in
/// [`super::LABELS`], directly or through an alias.
pub fn label_of(name: &str) -> LabelId {
    let name = ALIASES
        .iter()
        .find(|(m, _)| *m == name)
        .map_or(name, |(_, t)| t);
    super::find_label(name).unwrap_or_else(|| super::label_id("unknown"))
}
