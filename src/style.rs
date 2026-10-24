//! Colors: the color policy, type-group colors, and `LS_COLORS` for names.
//!
//! Colors only add to the text: every span keeps its text, so the output
//! with the codes taken out is the same as with `--color never` (FR-28).

use std::os::unix::ffi::OsStrExt;

use lscolors::{Indicator, LsColors};

use crate::detect::TypeGroup;
use crate::layout::{Line, Paint};
use crate::tree::{NodeId, Tree};

/// The resolved styling for one run.
pub struct Style {
    pub color: bool,
    ls: Option<LsColors>,
}

impl Style {
    /// `ls_colors` is the `LS_COLORS` value; it changes name colors only.
    pub fn new(color: bool, ls_colors: Option<&str>) -> Style {
        Style {
            color,
            ls: ls_colors.filter(|_| color).map(LsColors::from_string),
        }
    }

    pub fn plain() -> Style {
        Style::new(false, None)
    }

    /// The SGR codes for a span on a line, or `None` for no codes.
    pub fn sgr(&self, tree: &Tree, line: &Line, paint: Paint) -> Option<String> {
        if !self.color {
            return None;
        }
        let mut codes: Vec<String> = Vec::new();
        if line.dim {
            codes.push("2".into());
        }
        match paint {
            Paint::Plain => {}
            Paint::Bold => codes.push("1".into()),
            Paint::Warn => codes.push("33".into()),
            Paint::Name { node, group, dir } => {
                let code = self
                    .ls_code(tree, node, dir)
                    .or_else(|| group_code(group, dir).map(String::from));
                codes.extend(code);
            }
        }
        (!codes.is_empty()).then(|| codes.join(";"))
    }

    /// The `LS_COLORS` codes for a name, if a rule matches.
    fn ls_code(&self, tree: &Tree, node: NodeId, dir: bool) -> Option<String> {
        let ls = self.ls.as_ref()?;
        let style = if dir {
            ls.style_for_indicator(Indicator::Directory)?
        } else {
            let name = std::ffi::OsStr::from_bytes(tree.name(node));
            ls.style_for_str(&name.to_string_lossy())?
        };
        let codes = ls_codes(style);
        (!codes.is_empty()).then_some(codes)
    }
}

/// One color per type group; directories are bold blue.
fn group_code(group: TypeGroup, dir: bool) -> Option<&'static str> {
    if dir {
        return Some("1;34");
    }
    Some(match group {
        TypeGroup::Code => "32",
        TypeGroup::Text => "37",
        TypeGroup::Data => "33",
        TypeGroup::Image => "35",
        TypeGroup::Av => "36",
        TypeGroup::Archive => "31",
        TypeGroup::Exec => "1;32",
        TypeGroup::Binary => "90",
        TypeGroup::Unknown => return None,
    })
}

/// SGR codes for an `LS_COLORS` style.
fn ls_codes(style: &lscolors::Style) -> String {
    let f = &style.font_style;
    let mut codes: Vec<String> = Vec::new();
    for (on, code) in [
        (f.bold, 1),
        (f.dimmed, 2),
        (f.italic, 3),
        (f.underline, 4),
        (f.slow_blink, 5),
        (f.rapid_blink, 6),
        (f.reverse, 7),
        (f.hidden, 8),
        (f.strikethrough, 9),
    ] {
        if on {
            codes.push(code.to_string());
        }
    }
    if let Some(c) = style.foreground {
        codes.push(color_code(c, 30));
    }
    if let Some(c) = style.background {
        codes.push(color_code(c, 40));
    }
    codes.join(";")
}

fn color_code(c: lscolors::Color, base: u8) -> String {
    use lscolors::Color::*;
    let basic = |n: u8| (base + n).to_string();
    let bright = |n: u8| (base + 60 + n).to_string();
    match c {
        Black => basic(0),
        Red => basic(1),
        Green => basic(2),
        Yellow => basic(3),
        Blue => basic(4),
        Magenta => basic(5),
        Cyan => basic(6),
        White => basic(7),
        BrightBlack => bright(0),
        BrightRed => bright(1),
        BrightGreen => bright(2),
        BrightYellow => bright(3),
        BrightBlue => bright(4),
        BrightMagenta => bright(5),
        BrightCyan => bright(6),
        BrightWhite => bright(7),
        Fixed(n) => format!("{};5;{n}", base + 8),
        RGB(r, g, b) => format!("{};2;{r};{g};{b}", base + 8),
    }
}
