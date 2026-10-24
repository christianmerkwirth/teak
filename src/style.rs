//! Colors: the color policy, type-group colors, and `LS_COLORS` for names.

use crate::layout::{Line, Paint};
use crate::tree::Tree;

/// The resolved styling for one run.
pub struct Style {
    pub color: bool,
}

impl Style {
    pub fn plain() -> Style {
        Style { color: false }
    }

    /// The SGR codes for a span on a line, or `None` for no codes.
    pub fn sgr(&self, _tree: &Tree, _line: &Line, _paint: Paint) -> Option<String> {
        None
    }
}
