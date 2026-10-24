//! Rows to lines: cells, the column fit, name cuts, and glyphs.
//!
//! One row gives exactly one line. Layout never wraps; it cuts. When the
//! width is short, columns drop out in the PRD §5.3 order (preview, git,
//! age, type-mix detail), then type, permissions, count, and flags. Size
//! stays.

use std::fmt::Write as _;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::Scan;
use crate::aggregate::DirAgg;
use crate::chaff::{ChaffClass, ChaffTag};
use crate::detect::{LabelId, TypeGroup, label};
use crate::series::{Part, Series};
use crate::tree::{Kind, NodeId, ROOT, Tree, flag};
use crate::view::{ChaffItem, Row, RowKind};

/// A glyph set: Unicode, or ASCII with `--ascii`.
pub struct Glyphs {
    pub tee: &'static str,
    pub last: &'static str,
    pub pipe: &'static str,
    pub blank: &'static str,
    pub ellipsis: &'static str,
    pub dash: &'static str,
    pub rule: &'static str,
    pub sep: &'static str,
    pub chaff: &'static str,
    pub no_access: &'static str,
    pub cycle: &'static str,
    pub link: &'static str,
    pub mount: &'static str,
    pub approx: &'static str,
    pub warn: &'static str,
    pub ahead: &'static str,
    pub behind: &'static str,
}

pub static UNICODE: Glyphs = Glyphs {
    tee: "├── ",
    last: "└── ",
    pipe: "│   ",
    blank: "    ",
    ellipsis: "…",
    dash: "–",
    rule: "──",
    sep: " · ",
    chaff: "▪",
    no_access: "⊘",
    cycle: "↻",
    link: "⇢",
    mount: "⊞",
    approx: "≈",
    warn: "⚠",
    ahead: "↑",
    behind: "↓",
};

pub static ASCII: Glyphs = Glyphs {
    tee: "|-- ",
    last: "`-- ",
    pipe: "|   ",
    blank: "    ",
    ellipsis: "...",
    dash: "-",
    rule: "--",
    sep: " | ",
    chaff: "*",
    no_access: "!",
    cycle: "@loop",
    link: "->",
    mount: "[mnt]",
    approx: "~",
    warn: "!",
    ahead: "^",
    behind: "v",
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeUnits {
    /// Base 1024: `1.5K`.
    Iec,
    /// `--si`: base 1000, `1.5k`.
    Si,
    /// `--bytes`.
    Bytes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeMode {
    Relative,
    Iso,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewMode {
    Off,
    Short,
    Full,
}

pub struct LayoutOpts {
    pub width: usize,
    pub glyphs: &'static Glyphs,
    pub units: SizeUnits,
    pub time: TimeMode,
    pub preview: PreviewMode,
}

/// What a span means, for the colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint {
    Plain,
    /// An entry name.
    Name {
        node: NodeId,
        group: TypeGroup,
        dir: bool,
    },
    /// A large chaff size, or the size in a chaff-first line.
    Bold,
    /// `⊘` and other problems.
    Warn,
}

#[derive(Clone, Debug)]
pub struct Span {
    pub text: String,
    pub paint: Paint,
}

/// One output line. `dim` lines are chaff and overflow rows.
#[derive(Clone, Debug)]
pub struct Line {
    pub spans: Vec<Span>,
    pub dim: bool,
}

impl Line {
    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }
}

fn plain(text: impl Into<String>) -> Span {
    Span {
        text: text.into(),
        paint: Paint::Plain,
    }
}

/// The cells of a table row.
#[derive(Default)]
struct Cells {
    prefix: String,
    name: String,
    name_paint: Option<Paint>,
    size: String,
    size_bold: bool,
    count: String,
    ty: String,
    ty_short: String,
    /// The type cell is a `⚠` mismatch.
    ty_warn: bool,
    age: String,
    perms: String,
    flags: Vec<Span>,
    git: String,
    preview: String,
}

enum Prepared {
    Table(Box<Cells>, bool),
    Free(Vec<Span>, bool),
    Chaff {
        prefix: String,
        parent: NodeId,
        items: Vec<ChaffItem>,
        bold: bool,
    },
}

/// Lays out the view of `scan` in at most `opts.width` columns per line.
///
/// Post: one line per row; no line is wider than `opts.width`.
pub fn fit(scan: &Scan, opts: &LayoutOpts) -> Vec<Line> {
    let cx = Cx { scan, opts };
    let prefixes = prefixes(&scan.view.rows, opts.glyphs);
    let prepared: Vec<Prepared> = scan
        .view
        .rows
        .iter()
        .zip(prefixes)
        .map(|(row, prefix)| cx.prepare(row, prefix))
        .collect();
    let plan = cx.columns(&prepared);
    prepared
        .into_iter()
        .map(|p| {
            let line = match p {
                Prepared::Table(cells, dim) => cx.table_line(*cells, dim, &plan),
                Prepared::Free(spans, dim) => Line { spans, dim },
                Prepared::Chaff {
                    prefix,
                    parent,
                    items,
                    bold,
                } => cx.chaff_line(prefix, parent, &items, bold),
            };
            cut_line(line, opts.width, opts.glyphs.ellipsis)
        })
        .collect()
}

/// The tree glyphs before each row.
fn prefixes(rows: &[Row], g: &Glyphs) -> Vec<String> {
    let mut more: Vec<bool> = Vec::new();
    rows.iter()
        .map(|row| {
            let d = row.depth as usize;
            if d == 0 {
                return String::new();
            }
            more.resize(d, false);
            let mut s = String::new();
            for &m in &more[..d - 1] {
                s.push_str(if m { g.pipe } else { g.blank });
            }
            s.push_str(if row.last { g.last } else { g.tee });
            more[d - 1] = !row.last;
            s
        })
        .collect()
}

/// Which optional columns show, and the width of each.
struct ColumnPlan {
    name: usize,
    size: usize,
    count: Option<usize>,
    ty: Option<(usize, bool)>,
    age: Option<usize>,
    perms: Option<usize>,
    flags: Option<usize>,
    git: Option<usize>,
    preview: Option<usize>,
}

const MIN_NAME: usize = 24;
const SEP: usize = 2;

struct Cx<'a> {
    scan: &'a Scan,
    opts: &'a LayoutOpts,
}

impl Cx<'_> {
    fn tree(&self) -> &Tree {
        &self.scan.tree
    }

    fn agg(&self, id: NodeId) -> DirAgg {
        self.scan.aggs.get(&self.scan.tree, id)
    }

    fn prepare(&self, row: &Row, prefix: String) -> Prepared {
        let g = self.opts.glyphs;
        match &row.kind {
            RowKind::Header => Prepared::Free(self.header(), false),
            RowKind::Footer {
                used,
                budget,
                total_files,
                folded,
                hidden,
            } => Prepared::Free(
                vec![plain(self.footer(
                    *used,
                    *budget,
                    *total_files,
                    folded,
                    hidden,
                ))],
                false,
            ),
            RowKind::Mismatch { node } => {
                let tree = self.tree();
                let mut spans = vec![plain(format!("{}  ", escape(&tree.rel_path(*node))))];
                spans.extend(self.mismatch_spans(*node));
                Prepared::Free(spans, false)
            }
            RowKind::More { count } => Prepared::Free(
                vec![plain(format!("{} {} more", g.ellipsis, thousands(*count)))],
                true,
            ),
            RowKind::Note(text) => Prepared::Free(vec![plain(*text)], false),
            RowKind::Entry { node, joined, open } => Prepared::Table(
                Box::new(self.node_cells(prefix, *node, joined, *open)),
                self.dimmed(*node),
            ),
            RowKind::Summary {
                node,
                joined,
                uniform,
            } => {
                let mut cells = self.node_cells(prefix, *node, joined, false);
                if let Some(u) = uniform {
                    let tail = joined.last().copied().unwrap_or(*node);
                    let (_, estimated) = self.mix(&[tail]);
                    let ty = format!("{} ~{}", label(u.label).name, self.size(u.median_size));
                    (cells.ty, cells.ty_short) = self.approx(estimated, (ty.clone(), ty));
                }
                Prepared::Table(Box::new(cells), self.dimmed(*node))
            }
            RowKind::Overflow {
                dirs,
                files,
                size,
                total_files,
                newest,
                nodes,
                ..
            } => {
                let mut what = Vec::new();
                if *dirs > 0 {
                    what.push(plural(*dirs, "dir", "dirs"));
                }
                if *files > 0 {
                    what.push(plural(*files, "file", "files"));
                }
                let (ty, ty_short) = if *dirs > 0 {
                    self.size_mix(nodes)
                } else {
                    self.count_mix(nodes)
                };
                let cells = Cells {
                    prefix,
                    name: format!("{} {}", g.ellipsis, what.join(", ")),
                    size: self.size(*size),
                    count: if *dirs > 0 {
                        compact(*total_files)
                    } else {
                        String::new()
                    },
                    ty,
                    ty_short,
                    age: self.age(*newest),
                    ..Cells::default()
                };
                Prepared::Table(Box::new(cells), true)
            }
            RowKind::Series { series, .. } => {
                Prepared::Table(Box::new(self.series_cells(prefix, series)), false)
            }
            RowKind::Chaff {
                parent,
                items,
                first,
            } => Prepared::Chaff {
                prefix,
                parent: *parent,
                items: items.clone(),
                bold: *first,
            },
        }
    }

    /// The footer. When it is too wide, each group lists fewer classes and
    /// sums up the rest as `+N`.
    fn footer(
        &self,
        used: usize,
        budget: usize,
        total_files: u64,
        folded: &crate::view::ClassCounts,
        hidden: &crate::view::ClassCounts,
    ) -> String {
        let g = self.opts.glyphs;
        let most = folded.len().max(hidden.len());
        let text = |limit: usize| {
            let mut s = format!(
                "{} {used} of {budget} lines{}{}",
                g.rule,
                g.sep,
                files_text(total_files)
            );
            for (word, counts) in [("folded", folded), ("hidden", hidden)] {
                if counts.is_empty() {
                    continue;
                }
                let total: u64 = counts.iter().map(|(_, n)| n).sum();
                let _ = write!(s, "{}{word} {}", g.sep, thousands(total));
                if limit == 0 {
                    continue;
                }
                let mut parts: Vec<String> = counts
                    .iter()
                    .take(limit)
                    .map(|(c, n)| format!("{c} {}", thousands(*n)))
                    .collect();
                let rest: u64 = counts.iter().skip(limit).map(|(_, n)| n).sum();
                if counts.len() > limit {
                    parts.push(format!("+{}", thousands(rest)));
                }
                let _ = write!(s, " ({})", parts.join(g.sep));
            }
            s
        };
        (0..=most)
            .rev()
            .map(text)
            .find(|t| t.width() <= self.opts.width)
            .unwrap_or_else(|| text(0))
    }

    /// A row of an entry with the `dim` action.
    fn dimmed(&self, id: NodeId) -> bool {
        self.scan.tags[id as usize].action == crate::chaff::Action::Dim
    }

    fn header(&self) -> Vec<Span> {
        let tree = self.tree();
        let g = self.opts.glyphs;
        let root = tree.node(ROOT);
        let mut name = tree.root_label.clone();
        if root.is_dir() && !name.ends_with('/') {
            name.push('/');
        }
        let mut spans = vec![Span {
            text: escape(name.as_bytes()),
            paint: Paint::Name {
                node: ROOT,
                group: TypeGroup::Unknown,
                dir: root.is_dir(),
            },
        }];
        if root.has(flag::NO_ACCESS) {
            spans.push(plain(" "));
            spans.push(Span {
                text: g.no_access.to_string(),
                paint: Paint::Warn,
            });
        }
        let size: u64 = tree.nodes.iter().map(|n| n.size).sum();
        let mut text = format!(
            "  {}{}{}",
            self.size(size),
            g.sep,
            files_text(tree.file_count(ROOT))
        );
        if !self.scan.kinds.is_empty() {
            let _ = write!(text, "{}{}", g.sep, self.scan.kinds.join(" + "));
        }
        if let Some(head) = &self.scan.git_head {
            let _ = write!(text, "{}git {}", g.sep, head.branch);
            if head.ahead > 0 {
                let _ = write!(text, " {}{}", g.ahead, head.ahead);
            }
            if head.behind > 0 {
                let _ = write!(text, " {}{}", g.behind, head.behind);
            }
        }
        if let Some(git) = &self.scan.git {
            let changed = git.changed(tree);
            if changed > 0 {
                let _ = write!(text, "{}{} changed", g.sep, thousands(changed));
            }
        }
        spans.push(plain(text));
        spans
    }

    /// The git cell: the status letter of a file, `M<n>` for a directory
    /// with changed files below it.
    fn git_cell(&self, node: NodeId) -> String {
        let Some(git) = &self.scan.git else {
            return String::new();
        };
        let tree = self.tree();
        if tree.node(node).is_dir() {
            match git.changed_below(tree, node) {
                0 => String::new(),
                n => format!("M{n}"),
            }
        } else {
            git.of(node)
                .map_or(String::new(), |c| c.letter().to_string())
        }
    }

    fn node_cells(&self, prefix: String, node: NodeId, joined: &[NodeId], open: bool) -> Cells {
        let tree = self.tree();
        let n = tree.node(node);
        let tail = joined.last().copied().unwrap_or(node);
        let mut name = if node == ROOT {
            tree.root_label.clone()
        } else {
            escape(tree.name(node))
        };
        for &j in joined {
            name.push('/');
            name.push_str(&escape(tree.name(j)));
        }
        if n.is_dir() {
            name.push('/');
        }
        if let Some(l) = self.scan.template_label(node) {
            let _ = write!(name, " ({l})");
        }
        let info = self.scan.type_of(node);
        let group = if n.is_dir() {
            TypeGroup::Unknown
        } else {
            info.group
        };
        let mut cells = Cells {
            prefix,
            name,
            name_paint: Some(Paint::Name {
                node,
                group,
                dir: n.is_dir(),
            }),
            perms: perms(n.kind, n.mode),
            flags: self.flags(tail),
            git: self.git_cell(tail),
            ..Cells::default()
        };
        if n.is_dir() {
            let agg = self.agg(tail);
            cells.size = self.size(agg.size);
            cells.count = compact(agg.files);
            let (mut ty, mut ty_short) = self.dir_mix(tail);
            if !open && agg.dirs > 0 {
                let dirs = format!(
                    "{}{}",
                    self.opts.glyphs.sep,
                    plural(agg.dirs, "dir", "dirs")
                );
                ty.push_str(&dirs);
                ty_short.push_str(&dirs);
            }
            cells.ty = ty;
            cells.ty_short = ty_short;
            // teak could not list it: the content is not known, not empty.
            if tree.node(tail).has(flag::NO_ACCESS) {
                cells.count.clear();
                cells.ty.clear();
                cells.ty_short.clear();
            }
            cells.age = self.age(if agg.newest == i64::MIN {
                n.mtime_ns
            } else {
                agg.newest
            });
        } else {
            cells.size = self.size(n.size);
            let mut ty = info.name().to_string();
            if info.estimated {
                ty.push_str(self.opts.glyphs.approx);
            }
            if info.mismatch.is_some() {
                cells.ty = self
                    .mismatch_spans(node)
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect();
                cells.ty_short = format!("{ty} {}", self.opts.glyphs.warn);
                cells.ty_warn = true;
            } else {
                cells.ty_short = ty.clone();
                cells.ty = ty;
            }
            cells.age = self.age(n.mtime_ns);
            if n.has(flag::SECRET) && self.opts.preview != PreviewMode::Off {
                cells.preview = "no preview (secret)".to_string();
            }
        }
        cells
    }

    /// `html ⚠ ext says pdf` (FR-53).
    fn mismatch_spans(&self, node: NodeId) -> Vec<Span> {
        let info = self.scan.type_of(node);
        let Some(by_ext) = info.mismatch else {
            return vec![plain(info.name())];
        };
        vec![
            plain(format!("{} ", info.name())),
            Span {
                text: self.opts.glyphs.warn.to_string(),
                paint: Paint::Warn,
            },
            plain(format!(" ext says {}", label(by_ext).name)),
        ]
    }

    fn series_cells(&self, prefix: String, series: &Series) -> Cells {
        let g = self.opts.glyphs;
        let tree = self.tree();
        let mut name = String::new();
        for part in &series.parts {
            match part {
                Part::Lit(b) => name.push_str(&escape(b)),
                Part::Range(a, b) => {
                    let _ = write!(name, "[{}{}{}]", escape(a), g.dash, escape(b));
                }
                Part::Hash => name.push_str("[hash]"),
                Part::Uuid => name.push_str("[uuid]"),
            }
        }
        let count = if series.missing > 0 {
            format!(
                "{} of {}",
                thousands(series.range - series.missing),
                thousands(series.range)
            )
        } else {
            thousands(series.members.len() as u64)
        };
        let (mix, estimated) = self.series_mix(series);
        let ty = format!(
            "{mix}{} ~{}",
            if estimated { g.approx } else { "" },
            self.size(series.median_size)
        );
        let newest = series
            .members
            .iter()
            .map(|&m| tree.node(m).mtime_ns)
            .max()
            .unwrap_or(i64::MIN);
        Cells {
            prefix,
            name,
            name_paint: Some(Paint::Name {
                node: series.members[0],
                group: self.scan.type_of(series.members[0]).group,
                dir: false,
            }),
            size: self.size(series.total_size),
            count,
            ty_short: ty.clone(),
            ty,
            age: self.age(newest),
            ..Cells::default()
        }
    }

    /// The type mix of a series by count (`png 9 · jpeg 1`), from the typed
    /// members. A sampled series shows its main type, marked as estimated.
    fn series_mix(&self, series: &Series) -> (String, bool) {
        let typed: Vec<LabelId> = series
            .members
            .iter()
            .filter_map(|m| self.scan.types.get(m).map(|t| t.label))
            .collect();
        let estimated = typed.len() < series.members.len();
        let mut counts: Vec<(LabelId, u64)> = Vec::new();
        let labels: Vec<LabelId> = if typed.is_empty() {
            series
                .members
                .iter()
                .map(|&m| self.scan.labels[m as usize])
                .collect()
        } else {
            typed
        };
        for l in labels {
            match counts.iter_mut().find(|(c, _)| *c == l) {
                Some((_, n)) => *n += 1,
                None => counts.push((l, 1)),
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        if estimated || counts.len() == 1 {
            return (label(counts[0].0).name.to_string(), estimated);
        }
        let parts: Vec<String> = counts
            .iter()
            .take(3)
            .map(|(l, n)| format!("{} {}", label(*l).name, thousands(*n)))
            .collect();
        (parts.join(self.opts.glyphs.sep), false)
    }

    fn flags(&self, id: NodeId) -> Vec<Span> {
        let tree = self.tree();
        let n = tree.node(id);
        let g = self.opts.glyphs;
        let mut out = Vec::new();
        let mut push = |text: String, paint: Paint| {
            if !out.is_empty() {
                out.push(plain(" "));
            }
            out.push(Span { text, paint });
        };
        if n.has(flag::NO_ACCESS) {
            push(g.no_access.to_string(), Paint::Warn);
        }
        if n.has(flag::LOOP) {
            push(g.cycle.to_string(), Paint::Warn);
        }
        if n.has(flag::MOUNT) {
            push(g.mount.to_string(), Paint::Plain);
        }
        if let Some(target) = tree.link_targets.get(&id) {
            let mut text = format!("{} {}", g.link, escape(target));
            if n.has(flag::BROKEN) {
                text.push_str(" (broken)");
            }
            let paint = if n.has(flag::BROKEN) {
                Paint::Warn
            } else {
                Paint::Plain
            };
            push(text, paint);
        }
        out
    }

    fn size(&self, bytes: u64) -> String {
        size(bytes, self.opts.units)
    }

    fn age(&self, mtime_ns: i64) -> String {
        if mtime_ns == i64::MIN {
            return String::new();
        }
        let secs = mtime_ns.div_euclid(1_000_000_000);
        match self.opts.time {
            TimeMode::Relative => relative_age(self.tree().scan_time - secs),
            TimeMode::Iso => iso_time(secs),
        }
    }

    /// Label → (files, bytes) over the files of the given subtrees, without
    /// hidden entries. Sorted by bytes, then files, then label. True when a
    /// label in the mix is an estimate.
    fn mix(&self, roots: &[NodeId]) -> (Vec<(LabelId, u64, u64)>, bool) {
        let tree = self.tree();
        let mut acc: Vec<(u64, u64)> = vec![(0, 0); crate::detect::LABELS.len()];
        let mut estimated = false;
        let tags: &[ChaffTag] = &self.scan.tags;
        for &r in roots {
            let range = r..tree.node(r).end;
            let mut id = range.start;
            while id < range.end {
                if crate::chaff::is_hidden(tags, id) {
                    id = tree.node(id).end;
                    continue;
                }
                let n = tree.node(id);
                if !n.is_dir() {
                    let l = self.scan.labels[id as usize] as usize;
                    acc[l].0 += 1;
                    acc[l].1 += n.size;
                    estimated |= self.scan.estimated[id as usize];
                }
                id += 1;
            }
        }
        let mut v: Vec<(LabelId, u64, u64)> = acc
            .into_iter()
            .enumerate()
            .filter(|(_, (f, _))| *f > 0)
            .map(|(l, (f, b))| (l as LabelId, f, b))
            .collect();
        v.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)).then(a.0.cmp(&b.0)));
        (v, estimated)
    }

    /// `≈ ` before a mix with an estimate in it (FR-50).
    fn approx(&self, estimated: bool, (full, short): (String, String)) -> (String, String) {
        if !estimated {
            return (full, short);
        }
        let g = self.opts.glyphs.approx;
        (format!("{g} {full}"), format!("{g} {short}"))
    }

    /// `rust 50% toml 20% md 15% +2 types` (by size), and a short form.
    fn size_mix(&self, roots: &[NodeId]) -> (String, String) {
        let (mix, estimated) = self.mix(roots);
        if mix.is_empty() {
            return ("empty".into(), "empty".into());
        }
        let name = |l: LabelId| label(l).name;
        if mix.len() == 1 {
            return self.approx(estimated, (name(mix[0].0).into(), name(mix[0].0).into()));
        }
        let total_bytes: u64 = mix.iter().map(|m| m.2).sum();
        let total_files: u64 = mix.iter().map(|m| m.1).sum();
        let pct = |m: &(LabelId, u64, u64)| {
            (m.2 * 100 + total_bytes / 2)
                .checked_div(total_bytes)
                .unwrap_or((m.1 * 100 + total_files / 2) / total_files)
        };
        // Types that round to 0% count in `+N types`.
        let mut full: Vec<String> = mix
            .iter()
            .take(3)
            .filter(|m| pct(m) > 0)
            .map(|m| format!("{} {}%", name(m.0), pct(m)))
            .collect();
        let rest = mix.len() - full.len();
        if rest > 0 {
            full.push(format!("+{}", plural(rest as u64, "type", "types")));
        }
        let short = format!(
            "{} +{}",
            name(mix[0].0),
            plural(mix.len() as u64 - 1, "type", "types")
        );
        self.approx(estimated, (full.join(" "), short))
    }

    /// `csv 8 · json 2` (by count), and a short form.
    fn count_mix(&self, roots: &[NodeId]) -> (String, String) {
        let (mut mix, estimated) = self.mix(roots);
        if mix.is_empty() {
            return ("empty".into(), "empty".into());
        }
        mix.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)).then(a.0.cmp(&b.0)));
        let name = |l: LabelId| label(l).name;
        let sep = self.opts.glyphs.sep;
        let mut full: Vec<String> = mix
            .iter()
            .take(3)
            .map(|m| format!("{} {}", name(m.0), thousands(m.1)))
            .collect();
        let rest = mix.len().saturating_sub(3);
        if rest > 0 {
            full.push(format!("+{}", plural(rest as u64, "type", "types")));
        }
        let short = if mix.len() == 1 {
            name(mix[0].0).to_string()
        } else {
            format!(
                "{} +{}",
                name(mix[0].0),
                plural(mix.len() as u64 - 1, "type", "types")
            )
        };
        self.approx(estimated, (full.join(sep), short))
    }

    fn dir_mix(&self, dir: NodeId) -> (String, String) {
        self.size_mix(&[dir])
    }

    fn chaff_line(&self, prefix: String, parent: NodeId, items: &[ChaffItem], bold: bool) -> Line {
        let _ = (parent, bold);
        let g = self.opts.glyphs;
        let tree = self.tree();
        let head = format!("{} chaff  ", g.chaff);
        let mut spans = vec![plain(prefix.clone()), plain(head.clone())];
        let room = self
            .opts
            .width
            .saturating_sub(prefix.width() + head.width());
        let mut used = 0;
        let mut more = 0u64;
        let mut first = true;
        let named: Vec<&ChaffItem> = items
            .iter()
            .filter(|i| i.large || self.scan.tags[i.node as usize].class != Some(ChaffClass::Empty))
            .collect();
        let unnamed = items.len() - named.len();
        // Worst case for the `+N` tail.
        let tail_w = g.sep.width() + 1 + items.len().to_string().len();
        let mut stopped = false;
        for (k, item) in named.iter().enumerate() {
            let n = tree.node(item.node);
            let mut text = escape(tree.name(item.node));
            if n.is_dir() {
                text.push('/');
            }
            if let Some(l) = self.scan.template_label(item.node) {
                let _ = write!(text, " ({l})");
            }
            let mut parts = vec![Span {
                text: text.clone(),
                paint: Paint::Plain,
            }];
            if item.large {
                parts.push(plain(" "));
                parts.push(Span {
                    text: self.size(item.size),
                    paint: Paint::Bold,
                });
                if n.is_dir() {
                    parts.push(plain(format!(" {}", compact(item.files))));
                }
            }
            let w: usize = parts.iter().map(|p| p.text.width()).sum::<usize>()
                + if first { 0 } else { g.sep.width() };
            let is_last = k + 1 == named.len() && unnamed == 0;
            let need = used + w + if is_last { 0 } else { tail_w };
            if stopped || need > room {
                stopped = true;
                more += 1;
                continue;
            }
            if !first {
                spans.push(plain(g.sep));
            }
            spans.extend(parts);
            used += w;
            first = false;
        }
        more += unnamed as u64;
        if more > 0 {
            if !first {
                spans.push(plain(g.sep));
            }
            spans.push(plain(format!("+{more}")));
        }
        Line { spans, dim: true }
    }

    /// Picks the columns and widths so every table line fits.
    fn columns(&self, rows: &[Prepared]) -> ColumnPlan {
        let cells: Vec<&Cells> = rows
            .iter()
            .filter_map(|p| match p {
                Prepared::Table(c, _) => Some(&**c),
                _ => None,
            })
            .collect();
        let max = |f: &dyn Fn(&Cells) -> usize| cells.iter().map(|c| f(c)).max().unwrap_or(0);
        let name_nat = max(&|c| c.prefix.width() + c.name.width());
        let size = max(&|c| c.size.width());
        let count = max(&|c| c.count.width());
        let ty = max(&|c| c.ty.width());
        let ty_short = max(&|c| c.ty_short.width());
        let age = max(&|c| c.age.width());
        let perms = max(&|c| c.perms.width());
        let flags = max(&|c| c.flags.iter().map(|s| s.text.width()).sum());
        let git = max(&|c| c.git.width());
        let preview = max(&|c| c.preview.width());
        let some = |w: usize| (w > 0).then_some(w);
        let mut plan = ColumnPlan {
            name: name_nat,
            size,
            count: some(count),
            ty: some(ty).map(|w| (w, true)),
            age: some(age),
            perms: some(perms),
            flags: some(flags),
            git: some(git),
            preview: some(preview),
        };
        let width = self.opts.width;
        let others = |p: &ColumnPlan| {
            SEP + p.size
                + [
                    p.count,
                    p.ty.map(|t| t.0),
                    p.age,
                    p.perms,
                    p.flags,
                    p.git,
                    p.preview,
                ]
                .iter()
                .flatten()
                .map(|w| SEP + w)
                .sum::<usize>()
        };
        let min_name = name_nat.min(MIN_NAME);
        type Drop = fn(&mut ColumnPlan, usize);
        let steps: [Drop; 8] = [
            |p, _| p.preview = None,
            |p, _| p.git = None,
            |p, _| p.age = None,
            |p, short| p.ty = p.ty.map(|_| (short, false)).filter(|t| t.0 > 0),
            |p, _| p.ty = None,
            |p, _| p.perms = None,
            |p, _| p.count = None,
            |p, _| p.flags = None,
        ];
        let mut step = 0;
        loop {
            let rest = others(&plan);
            if width >= rest + min_name || step == steps.len() {
                plan.name = name_nat.min(width.saturating_sub(rest)).max(1);
                return plan;
            }
            steps[step](&mut plan, ty_short);
            step += 1;
        }
    }

    fn table_line(&self, c: Cells, dim: bool, plan: &ColumnPlan) -> Line {
        let mut spans = vec![plain(c.prefix.clone())];
        let room = plan.name.saturating_sub(c.prefix.width());
        let name = cut_middle(&c.name, room.max(1), self.opts.glyphs.ellipsis);
        let name_w = name.width();
        spans.push(Span {
            text: name,
            paint: c.name_paint.unwrap_or(Paint::Plain),
        });
        let pad = plan.name.saturating_sub(c.prefix.width() + name_w);
        let mut tail = " ".repeat(pad);
        let cell = |spans: &mut Vec<Span>,
                    tail: &mut String,
                    text: &str,
                    w: usize,
                    right: bool,
                    paint: Paint| {
            tail.push_str("  ");
            let fill = w.saturating_sub(text.width());
            if right {
                tail.push_str(&" ".repeat(fill));
            }
            spans.push(plain(std::mem::take(tail)));
            spans.push(Span {
                text: text.to_string(),
                paint,
            });
            if !right {
                tail.push_str(&" ".repeat(fill));
            }
        };
        let size_paint = if c.size_bold {
            Paint::Bold
        } else {
            Paint::Plain
        };
        cell(&mut spans, &mut tail, &c.size, plan.size, true, size_paint);
        if let Some(w) = plan.count {
            cell(&mut spans, &mut tail, &c.count, w, true, Paint::Plain);
        }
        if let Some((w, full)) = plan.ty {
            let text = if full { &c.ty } else { &c.ty_short };
            let paint = if c.ty_warn { Paint::Warn } else { Paint::Plain };
            cell(&mut spans, &mut tail, text, w, false, paint);
        }
        if let Some(w) = plan.age {
            cell(&mut spans, &mut tail, &c.age, w, true, Paint::Plain);
        }
        if let Some(w) = plan.perms {
            cell(&mut spans, &mut tail, &c.perms, w, false, Paint::Plain);
        }
        if let Some(w) = plan.flags {
            tail.push_str("  ");
            spans.push(plain(std::mem::take(&mut tail)));
            let fw: usize = c.flags.iter().map(|s| s.text.width()).sum();
            spans.extend(c.flags);
            tail.push_str(&" ".repeat(w.saturating_sub(fw)));
        }
        if let Some(w) = plan.git {
            cell(&mut spans, &mut tail, &c.git, w, false, Paint::Plain);
        }
        if plan.preview.is_some() && !c.preview.is_empty() {
            cell(&mut spans, &mut tail, &c.preview, 0, false, Paint::Plain);
        }
        trim_end(&mut spans);
        Line { spans, dim }
    }
}

/// Drops trailing spaces and empty spans.
fn trim_end(spans: &mut Vec<Span>) {
    while let Some(last) = spans.last_mut() {
        let t = last.text.trim_end_matches(' ').len();
        last.text.truncate(t);
        if last.text.is_empty() {
            spans.pop();
        } else {
            break;
        }
    }
}

/// Cuts a line to `width` columns, with `ellipsis` at the end of a cut.
fn cut_line(mut line: Line, width: usize, ellipsis: &str) -> Line {
    let total: usize = line.spans.iter().map(|s| s.text.width()).sum();
    if total <= width {
        return line;
    }
    let room = width.saturating_sub(ellipsis.width());
    let mut used = 0;
    let mut out = Vec::new();
    for span in line.spans {
        let w = span.text.width();
        if used + w <= room {
            used += w;
            out.push(span);
            continue;
        }
        let mut text = String::new();
        for ch in span.text.chars() {
            let cw = ch.width().unwrap_or(0);
            if used + cw > room {
                break;
            }
            used += cw;
            text.push(ch);
        }
        if !text.is_empty() {
            out.push(Span {
                text,
                paint: span.paint,
            });
        }
        break;
    }
    out.push(plain(ellipsis));
    line.spans = out;
    line
}

/// Cuts a name in the middle so it fits `room` columns; the extension (and a
/// trailing `/`) stays.
pub fn cut_middle(name: &str, room: usize, ellipsis: &str) -> String {
    if name.width() <= room {
        return name.to_string();
    }
    let ell = ellipsis.width();
    if room <= ell {
        return name.chars().take(room).collect();
    }
    let avail = room - ell;
    let chars: Vec<char> = name.chars().collect();
    // Keep the extension: the text from the last dot, if it is short.
    let body_end = if name.ends_with('/') {
        chars.len() - 1
    } else {
        chars.len()
    };
    let ext_len = chars[..body_end]
        .iter()
        .rposition(|&c| c == '.')
        .filter(|&i| i > 0 && body_end - i <= 8)
        .map_or(0, |i| body_end - i)
        + (chars.len() - body_end);
    let tail_len = ext_len.max(avail / 3).min(avail.saturating_sub(1));
    let head_len = avail - tail_len;
    let mut out = String::new();
    let mut w = 0;
    for &c in &chars {
        let cw = c.width().unwrap_or(0);
        if w + cw > head_len {
            break;
        }
        w += cw;
        out.push(c);
    }
    out.push_str(ellipsis);
    let mut tail: Vec<char> = Vec::new();
    let mut tw = 0;
    for &c in chars.iter().rev() {
        let cw = c.width().unwrap_or(0);
        if tw + cw > tail_len {
            break;
        }
        tw += cw;
        tail.push(c);
    }
    out.extend(tail.into_iter().rev());
    out
}

/// Shows name bytes: bytes that are not UTF-8, and control characters, as
/// `\xHH`.
pub fn escape(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for chunk in bytes.utf8_chunks() {
        for ch in chunk.valid().chars() {
            if ch.is_control() {
                let mut buf = [0u8; 4];
                for b in ch.encode_utf8(&mut buf).bytes() {
                    let _ = write!(out, "\\x{b:02X}");
                }
            } else {
                out.push(ch);
            }
        }
        for b in chunk.invalid() {
            let _ = write!(out, "\\x{b:02X}");
        }
    }
    out
}

/// `0`, `512`, `1.5K`, `14M`; `--si` uses base 1000 and `k`; `--bytes`
/// prints whole bytes.
pub fn size(bytes: u64, units: SizeUnits) -> String {
    let (base, suffixes): (f64, [&str; 6]) = match units {
        SizeUnits::Bytes => return bytes.to_string(),
        SizeUnits::Iec => (1024.0, ["K", "M", "G", "T", "P", "E"]),
        SizeUnits::Si => (1000.0, ["k", "M", "G", "T", "P", "E"]),
    };
    if (bytes as f64) < base {
        return bytes.to_string();
    }
    scaled(bytes as f64, base, &suffixes)
}

/// One decimal below 10, whole numbers above.
fn scaled(mut v: f64, base: f64, suffixes: &[&str]) -> String {
    let mut i = 0;
    v /= base;
    while v >= base && i + 1 < suffixes.len() {
        v /= base;
        i += 1;
    }
    if (v * 10.0).round() < 100.0 {
        return format!("{:.1}{}", v, suffixes[i]);
    }
    let r = v.round();
    if r >= base && i + 1 < suffixes.len() {
        return format!("{:.1}{}", r / base, suffixes[i + 1]);
    }
    format!("{}{}", r as u64, suffixes[i])
}

/// The count column: `812`, `3.2k`, `44k`.
pub fn compact(n: u64) -> String {
    if n < 1000 {
        return n.to_string();
    }
    scaled(n as f64, 1000.0, &["k", "M", "G", "T"])
}

/// `48,213`.
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn plural(n: u64, one: &str, many: &str) -> String {
    format!("{} {}", thousands(n), if n == 1 { one } else { many })
}

fn files_text(n: u64) -> String {
    plural(n, "file", "files")
}

/// The largest unit with a value of 1 or more.
pub fn relative_age(secs: i64) -> String {
    let secs = secs.max(0);
    const UNITS: [(i64, &str); 6] = [
        (365 * 86_400, "y"),
        (30 * 86_400, "mo"),
        (7 * 86_400, "w"),
        (86_400, "d"),
        (3600, "h"),
        (60, "m"),
    ];
    for (len, unit) in UNITS {
        if secs >= len {
            return format!("{}{unit}", secs / len);
        }
    }
    format!("{secs}s")
}

/// `YYYY-MM-DDTHH:MM` in UTC.
pub fn iso_time(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60
    )
}

/// Short permissions, only when unusual: `rwx`, `suid`, `sgid`, `o+w`.
pub fn perms(kind: Kind, mode: u32) -> String {
    if kind == Kind::Symlink {
        return String::new();
    }
    let mut parts = Vec::new();
    if kind == Kind::File && mode & 0o111 != 0 {
        parts.push("rwx");
    }
    if mode & 0o4000 != 0 {
        parts.push("suid");
    }
    if mode & 0o2000 != 0 {
        parts.push("sgid");
    }
    let sticky_dir = kind == Kind::Dir && mode & 0o1000 != 0;
    if mode & 0o002 != 0 && !sticky_dir {
        parts.push("o+w");
    }
    parts.join(",")
}
