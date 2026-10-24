//! Output formats: `tty` and `plain` (text), `md`, and `json`.

pub mod json;
pub mod md;
pub mod text;

use crate::view::{ClassCounts, RowKind, View};

/// `--format`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Tty,
    Plain,
    Md,
    Json,
}

pub use text::write;

/// The footer of a view: (used, budget, total files, folded, hidden).
pub(crate) fn footer(view: &View) -> Option<(usize, usize, u64, &ClassCounts, &ClassCounts)> {
    view.rows.iter().find_map(|r| match &r.kind {
        RowKind::Footer {
            used,
            budget,
            total_files,
            folded,
            hidden,
        } => Some((*used, *budget, *total_files, folded, hidden)),
        _ => None,
    })
}
