//! Project kinds from marker files (FR-59).

use crate::detect::{LabelId, label};
use crate::tree::{NodeId, Tree};

/// Marker names and their kinds, in display order.
const MARKERS: &[(&[u8], &str)] = &[
    (b"Cargo.toml", "rust crate"),
    (b"package.json", "node pkg"),
    (b"pyproject.toml", "python pkg"),
    (b"pyvenv.cfg", "venv"),
];

/// Image labels that carry EXIF data.
const EXIF_LABELS: &[&str] = &["jpeg", "tiff", "heic"];

/// More than this share of the files must be images with EXIF data.
const PHOTO_SHARE: f64 = 0.8;

/// The most images that the EXIF check reads.
pub const EXIF_SAMPLE: usize = 16;

/// The kinds of `dir`: marker files, `photos` when more than 80% of its
/// files are images with EXIF data, and `git repo` for a `.git/`.
///
/// `labels` gives the type of each node. `has_exif` reads the head of an
/// image; it runs for at most [`EXIF_SAMPLE`] images, and all of them must
/// have EXIF data.
pub fn detect(
    tree: &Tree,
    dir: NodeId,
    labels: &[LabelId],
    mut has_exif: impl FnMut(NodeId) -> bool,
) -> Vec<&'static str> {
    let mut out = Vec::new();
    if !tree.node(dir).is_dir() {
        return out;
    }
    let names: Vec<&[u8]> = tree.children(dir).map(|c| tree.name(c)).collect();
    for &(marker, kind) in MARKERS {
        if names.contains(&marker) {
            out.push(kind);
        }
    }
    let files: Vec<NodeId> = tree
        .children(dir)
        .filter(|&c| !tree.node(c).is_dir())
        .collect();
    let images: Vec<NodeId> = files
        .iter()
        .copied()
        .filter(|&c| EXIF_LABELS.contains(&label(labels[c as usize]).name))
        .collect();
    if !files.is_empty()
        && images.len() as f64 > PHOTO_SHARE * files.len() as f64
        && images.iter().take(EXIF_SAMPLE).all(|&c| has_exif(c))
    {
        out.push("photos");
    }
    if tree
        .children(dir)
        .any(|c| tree.node(c).is_dir() && tree.name(c) == b".git")
    {
        out.push("git repo");
    }
    out
}

/// An EXIF APP1 segment in the first bytes of a JPEG, or a TIFF header.
pub fn head_has_exif(head: &[u8]) -> bool {
    if head.starts_with(b"II*\0") || head.starts_with(b"MM\0*") {
        return true;
    }
    head.windows(6).take(4096).any(|w| w == b"Exif\0\0")
}
