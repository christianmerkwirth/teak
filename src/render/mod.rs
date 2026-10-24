//! Output formats. P1 has `tty` and `plain`; P4 adds `md` and `json`.

pub mod text;

/// `--format`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Tty,
    Plain,
}

pub use text::write;
