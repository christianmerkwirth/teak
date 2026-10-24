//! The `tty` and `plain` formats: the lines, with SGR codes when colors
//! are on.

use std::io::{self, Write};

use crate::layout::Line;
use crate::style::Style;
use crate::tree::Tree;

pub fn write(lines: &[Line], tree: &Tree, style: &Style, out: &mut impl Write) -> io::Result<()> {
    let mut buf = String::new();
    for line in lines {
        buf.clear();
        for span in &line.spans {
            match style.sgr(tree, line, span.paint) {
                Some(codes) if style.color && !span.text.trim().is_empty() => {
                    buf.push_str("\x1b[");
                    buf.push_str(&codes);
                    buf.push('m');
                    buf.push_str(&span.text);
                    buf.push_str("\x1b[0m");
                }
                _ => buf.push_str(&span.text),
            }
        }
        buf.push('\n');
        out.write_all(buf.as_bytes())?;
    }
    Ok(())
}
