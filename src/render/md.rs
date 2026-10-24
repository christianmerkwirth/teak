//! The `md` format (FR-63): a heading, a line of facts, the `plain` rows in
//! ASCII in a fenced block, the type table, and the folded classes.
//!
//! The budget holds for the lines inside the fenced block: the header and
//! footer rows of the view become the lines around it.

use std::fmt::Write as _;
use std::io::{self, Write};

use crate::Scan;
use crate::layout::{self, ASCII, LayoutOpts};
use crate::tree::ROOT;
use crate::view::RowKind;

/// Writes the Markdown view of `scan`. With `stats`, there is no tree block.
pub fn write(scan: &Scan, opts: &LayoutOpts, out: &mut impl Write) -> io::Result<()> {
    write_view(scan, opts, false, out)
}

/// [`write`] for `--stats`: the heading, the facts, the table, and the
/// folded line.
pub fn write_stats(scan: &Scan, opts: &LayoutOpts, out: &mut impl Write) -> io::Result<()> {
    write_view(scan, opts, true, out)
}

fn write_view(scan: &Scan, opts: &LayoutOpts, stats: bool, out: &mut impl Write) -> io::Result<()> {
    let opts = LayoutOpts {
        glyphs: &ASCII,
        ..*opts
    };
    let mut s = String::new();
    let tree = &scan.tree;
    let (size, files) = layout::root_totals(tree);
    let mut name = tree.root_label.clone();
    if tree.node(ROOT).is_dir() && !name.ends_with('/') {
        name.push('/');
    }
    let _ = writeln!(
        s,
        "# {} — {}, {}",
        escape_inline(&layout::escape(name.as_bytes())),
        layout::size(size, opts.units),
        layout::files_text(files)
    );
    s.push_str(&facts(scan));
    s.push_str("\n\n");

    if !stats {
        let lines = layout::fit(scan, &opts);
        let rows: Vec<String> = lines
            .iter()
            .zip(&scan.view.rows)
            .filter(|(_, r)| !matches!(r.kind, RowKind::Header | RowKind::Footer { .. }))
            .map(|(l, _)| l.text())
            .collect();
        let fence = "`".repeat(
            rows.iter()
                .map(|r| longest_run(r, b'`'))
                .max()
                .unwrap_or(0)
                .max(2)
                + 1,
        );
        let _ = writeln!(s, "{fence}text");
        for r in &rows {
            s.push_str(r);
            s.push('\n');
        }
        let _ = writeln!(s, "{fence}\n");
    }

    s.push_str("| Group | Files | Size |\n|---|---:|---:|\n");
    for (g, files, bytes) in layout::type_groups(scan) {
        let _ = writeln!(
            s,
            "| {} | {} | {} |",
            g.name(),
            layout::thousands(files),
            layout::size(bytes, opts.units)
        );
    }
    s.push('\n');

    let list = |counts: &crate::view::ClassCounts| {
        counts
            .iter()
            .map(|(c, n)| format!("{c} {}", layout::thousands(*n)))
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let (folded, hidden) = match super::footer(&scan.view) {
        Some((_, _, _, f, h)) => (list(f), list(h)),
        None => (String::new(), String::new()),
    };
    let _ = write!(
        s,
        "Folded: {}.",
        if folded.is_empty() { "none" } else { &folded }
    );
    if !hidden.is_empty() {
        let _ = write!(s, " Hidden: {hidden}.");
    }
    s.push_str(" Run `teak -a` to show all.\n");
    out.write_all(s.as_bytes())
}

/// Line 2: the project kinds, git, the changed count, and the scan time.
fn facts(scan: &Scan) -> String {
    let mut parts = Vec::new();
    if !scan.kinds.is_empty() {
        parts.push(scan.kinds.join(" + "));
    }
    if let Some(head) = &scan.git_head {
        let mut git = format!("git `{}`", head.branch.replace('`', "'"));
        if head.ahead > 0 {
            let _ = write!(git, " ↑{}", head.ahead);
        }
        if head.behind > 0 {
            let _ = write!(git, " ↓{}", head.behind);
        }
        parts.push(git);
    }
    if let Some(git) = &scan.git {
        let changed = git.changed(&scan.tree);
        if changed > 0 {
            parts.push(format!("{} changed", layout::thousands(changed)));
        }
    }
    parts.push(format!(
        "scanned {}",
        layout::iso_time(scan.tree.scan_time).replace('T', " ")
    ));
    parts.join(" · ")
}

/// The longest run of `byte` in `text`.
fn longest_run(text: &str, byte: u8) -> usize {
    text.as_bytes()
        .split(|&b| b != byte)
        .map(<[u8]>::len)
        .max()
        .unwrap_or(0)
}

/// Escapes the characters that Markdown reads as markup in a heading.
fn escape_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(
            ch,
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '#' | '|' | '~' | '!' | '&'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}
