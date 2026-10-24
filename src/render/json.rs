//! The `json` format (FR-64): `schema_version` 1, the header, one object per
//! visible row, the footer, and the type groups. `--json-full` adds every
//! node in `entries`. The schema is `schema/teak.schema.json`.
//!
//! Keys come in a fixed order; arrays follow the display order (rows) or
//! the tree order (entries). Paths are relative to the root, joined with
//! `/`, with a `/` after a directory, and use the `\xHH` escape of the text
//! output. Times are seconds since the epoch. The budget holds for the
//! number of objects in `rows`: the header and footer rows of the view are
//! the top-level `header` and `footer`.

use std::io::{self, Write};

use serde_json::{Map, Value, json};

use crate::Scan;
use crate::detect::label;
use crate::layout::{self, LayoutOpts, PreviewMode};
use crate::tree::{Kind, NodeId, ROOT, flag};
use crate::view::RowKind;

/// The version of the JSON output. A change to the schema needs a new
/// version and a changelog entry.
pub const SCHEMA_VERSION: u64 = 1;

/// Writes the JSON document of `scan` and a line break.
pub fn write(scan: &Scan, opts: &LayoutOpts, full: bool, out: &mut impl Write) -> io::Result<()> {
    let mut doc = document(scan, opts, full, false);
    doc.push('\n');
    out.write_all(doc.as_bytes())
}

/// The JSON document of `scan`, without a final line break. With `stats`
/// (`--stats`), `rows` is empty.
pub fn document(scan: &Scan, opts: &LayoutOpts, full: bool, stats: bool) -> String {
    let rows: Vec<Value> = if stats {
        Vec::new()
    } else {
        scan.view
            .rows
            .iter()
            .filter_map(|r| row(scan, opts, r))
            .collect()
    };
    let mut parts: Vec<(&str, Part)> = vec![
        ("schema_version", Part::One(json!(SCHEMA_VERSION))),
        ("root", Part::One(json!(root_name(scan)))),
        ("scan_time", Part::One(json!(scan.tree.scan_time))),
        ("header", Part::One(header(scan))),
        ("rows", Part::Many(rows)),
        ("footer", Part::One(footer(scan))),
        ("types", Part::One(types(scan))),
    ];
    if full {
        parts.push(("entries", Part::Many(entries(scan, stats))));
    }
    let mut s = String::from("{\n");
    for (i, (key, part)) in parts.iter().enumerate() {
        s.push_str("  ");
        s.push_str(&Value::from(*key).to_string());
        s.push_str(": ");
        match part {
            Part::One(v) => s.push_str(&v.to_string()),
            Part::Many(items) if items.is_empty() => s.push_str("[]"),
            Part::Many(items) => {
                s.push_str("[\n");
                for (j, item) in items.iter().enumerate() {
                    s.push_str("    ");
                    s.push_str(&item.to_string());
                    if j + 1 < items.len() {
                        s.push(',');
                    }
                    s.push('\n');
                }
                s.push_str("  ]");
            }
        }
        if i + 1 < parts.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push('}');
    s
}

/// A top-level value: one line, or an array with one item per line.
enum Part {
    One(Value),
    Many(Vec<Value>),
}

fn root_name(scan: &Scan) -> String {
    let tree = &scan.tree;
    let mut name = tree.root_label.clone();
    if tree.node(ROOT).is_dir() && !name.ends_with('/') {
        name.push('/');
    }
    layout::escape(name.as_bytes())
}

/// The path of a node: relative to the root, `/` after a directory, `./`
/// for the root.
fn path(scan: &Scan, id: NodeId) -> String {
    let tree = &scan.tree;
    let mut p = if id == ROOT {
        ".".to_string()
    } else {
        layout::escape(&tree.rel_path(id))
    };
    if tree.node(id).is_dir() {
        p.push('/');
    }
    p
}

fn secs(mtime_ns: i64) -> Value {
    if mtime_ns == i64::MIN || mtime_ns == i64::MAX {
        Value::Null
    } else {
        json!(mtime_ns.div_euclid(1_000_000_000))
    }
}

fn header(scan: &Scan) -> Value {
    let (size, files) = layout::root_totals(&scan.tree);
    let git = match &scan.git_head {
        Some(head) => json!({
            "branch": head.branch,
            "ahead": head.ahead,
            "behind": head.behind,
            "changed": scan.git.as_ref().map_or(0, |g| g.changed(&scan.tree)),
        }),
        None => Value::Null,
    };
    json!({
        "size": size,
        "files": files,
        "kinds": scan.kinds,
        "git": git,
    })
}

fn footer(scan: &Scan) -> Value {
    let counts = |c: &crate::view::ClassCounts| {
        c.iter()
            .map(|(class, files)| json!({"class": class, "files": files}))
            .collect::<Vec<_>>()
    };
    match super::footer(&scan.view) {
        Some((used, budget, total_files, folded, hidden)) => json!({
            "used": used,
            "budget": budget,
            "total_files": total_files,
            "folded": counts(folded),
            "hidden": counts(hidden),
        }),
        None => json!({
            "used": scan.view.rows.len(),
            "budget": scan.view.budget,
            "total_files": scan.tree.file_count(ROOT),
            "folded": [],
            "hidden": [],
        }),
    }
}

fn types(scan: &Scan) -> Value {
    layout::type_groups(scan)
        .into_iter()
        .map(|(g, files, size)| json!({"group": g.name(), "files": files, "size": size}))
        .collect()
}

/// The type mix of subtrees: `[{type, files, size}]`, by size, and true
/// when a label is an estimate.
fn mix(scan: &Scan, roots: &[NodeId]) -> (Value, bool) {
    let (mix, estimated) = layout::mix(scan, roots);
    let v = mix
        .into_iter()
        .map(|(l, files, size)| json!({"type": label(l).name, "files": files, "size": size}))
        .collect();
    (v, estimated)
}

fn node_flags(scan: &Scan, id: NodeId) -> Vec<&'static str> {
    let n = scan.tree.node(id);
    [
        (flag::NO_ACCESS, "no_access"),
        (flag::LOOP, "loop"),
        (flag::BROKEN, "broken"),
        (flag::MOUNT, "mount"),
        (flag::PSEUDO_FS, "pseudo_fs"),
        (flag::SECRET, "secret"),
    ]
    .into_iter()
    .filter(|(bit, _)| n.has(*bit))
    .map(|(_, name)| name)
    .collect()
}

fn class(scan: &Scan, id: NodeId) -> Value {
    scan.tags[id as usize]
        .class
        .map_or(Value::Null, |c| json!(c.name()))
}

/// The keys that every row has, in order.
#[allow(clippy::too_many_arguments)]
fn base(
    kind: &str,
    path: String,
    depth: u16,
    size: u64,
    files: u64,
    dirs: u64,
    mtime: Value,
    ty: Value,
    (type_mix, estimated): (Value, bool),
    class: Value,
    flags: Vec<&str>,
) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("kind".into(), json!(kind));
    m.insert("path".into(), json!(path));
    m.insert("depth".into(), json!(depth));
    m.insert("size".into(), json!(size));
    m.insert("files".into(), json!(files));
    m.insert("dirs".into(), json!(dirs));
    m.insert("mtime".into(), mtime);
    m.insert("type".into(), ty);
    m.insert("type_mix".into(), type_mix);
    m.insert("estimated".into(), json!(estimated));
    m.insert("class".into(), class);
    m.insert("flags".into(), json!(flags));
    m
}

/// The optional keys of a node row: `git`, `preview`, `mismatch`,
/// `dup_of`, and `target` (a symlink).
fn extras(scan: &Scan, opts: &LayoutOpts, id: NodeId, m: &mut Map<String, Value>) {
    let cell = layout::git_cell(scan, id);
    if !cell.is_empty() {
        m.insert("git".into(), json!(cell));
    }
    let n = scan.tree.node(id);
    if !n.is_dir() && opts.preview != PreviewMode::Off {
        if n.has(flag::SECRET) {
            m.insert("preview".into(), json!("no preview (secret)"));
        } else if let Some(p) = scan.previews.get(&id) {
            m.insert("preview".into(), json!(p));
        }
    }
    if !n.is_dir()
        && let Some(by_ext) = scan.type_of(id).mismatch
    {
        m.insert("mismatch".into(), json!(label(by_ext).name));
    }
    if let Some(&orig) = scan.dupes.get(&id) {
        m.insert("dup_of".into(), json!(path(scan, orig)));
    }
    if let Some(target) = scan.tree.link_targets.get(&id) {
        m.insert("target".into(), json!(layout::escape(target)));
    }
}

/// The row object of a view row; `None` for the header, the footer, and
/// notes.
fn row(scan: &Scan, opts: &LayoutOpts, r: &crate::view::Row) -> Option<Value> {
    let tree = &scan.tree;
    let agg = |id| scan.aggs.get(tree, id);
    let m = match &r.kind {
        RowKind::Header | RowKind::Footer { .. } | RowKind::Note(_) => return None,
        RowKind::Entry { node, joined, open } => {
            let tail = joined.last().copied().unwrap_or(*node);
            let n = tree.node(tail);
            let mut m = if n.is_dir() {
                let a = agg(tail);
                let mut flags = node_flags(scan, tail);
                if *open {
                    flags.insert(0, "open");
                }
                let newest = if a.newest == i64::MIN {
                    n.mtime_ns
                } else {
                    a.newest
                };
                base(
                    "entry",
                    path(scan, tail),
                    r.depth,
                    a.size,
                    a.files,
                    a.dirs,
                    secs(newest),
                    Value::Null,
                    mix(scan, &[tail]),
                    class(scan, tail),
                    flags,
                )
            } else {
                let info = scan.type_of(tail);
                base(
                    "entry",
                    path(scan, tail),
                    r.depth,
                    n.size,
                    1,
                    0,
                    secs(n.mtime_ns),
                    json!(info.name()),
                    (json!([]), info.estimated),
                    class(scan, tail),
                    node_flags(scan, tail),
                )
            };
            extras(scan, opts, tail, &mut m);
            m
        }
        RowKind::Summary {
            node,
            joined,
            uniform,
        } => {
            let tail = joined.last().copied().unwrap_or(*node);
            let a = agg(tail);
            let n = tree.node(tail);
            let newest = if a.newest == i64::MIN {
                n.mtime_ns
            } else {
                a.newest
            };
            let mut m = base(
                "summary",
                path(scan, tail),
                r.depth,
                a.size,
                a.files,
                a.dirs,
                secs(newest),
                uniform.map_or(Value::Null, |u| json!(label(u.label).name)),
                mix(scan, &[tail]),
                class(scan, tail),
                node_flags(scan, tail),
            );
            extras(scan, opts, tail, &mut m);
            m
        }
        RowKind::Overflow {
            parent,
            dirs,
            size,
            total_files,
            newest,
            nodes,
            ..
        } => base(
            "overflow",
            path(scan, *parent),
            r.depth,
            *size,
            *total_files,
            *dirs,
            secs(*newest),
            Value::Null,
            mix(scan, nodes),
            Value::Null,
            Vec::new(),
        ),
        RowKind::Series { parent, series } => {
            let (counts, estimated) = layout::series_counts(scan, series);
            let newest = series
                .members
                .iter()
                .map(|&m| tree.node(m).mtime_ns)
                .max()
                .unwrap_or(i64::MIN);
            let type_mix = counts
                .iter()
                .map(|&(l, files)| {
                    let size: u64 = series
                        .members
                        .iter()
                        .filter(|&&m| scan.type_of(m).label == l)
                        .map(|&m| tree.node(m).size)
                        .sum();
                    json!({"type": label(l).name, "files": files, "size": size})
                })
                .collect();
            let mut p = path(scan, *parent);
            if *parent == ROOT {
                p.clear();
            }
            p.push_str(&layout::series_name(series, &layout::UNICODE));
            base(
                "series",
                p,
                r.depth,
                series.total_size,
                series.members.len() as u64,
                0,
                secs(newest),
                counts
                    .first()
                    .map_or(Value::Null, |&(l, _)| json!(label(l).name)),
                (type_mix, estimated),
                Value::Null,
                Vec::new(),
            )
        }
        RowKind::Chaff {
            parent,
            items,
            first,
        } => {
            let nodes: Vec<NodeId> = items.iter().map(|i| i.node).collect();
            let newest = items
                .iter()
                .map(|i| {
                    let n = tree.node(i.node);
                    if n.is_dir() {
                        agg(i.node).newest
                    } else {
                        n.mtime_ns
                    }
                })
                .max()
                .unwrap_or(i64::MIN);
            let dirs = items.iter().filter(|i| tree.node(i.node).is_dir()).count() as u64;
            let mut m = base(
                "chaff",
                path(scan, *parent),
                r.depth,
                items.iter().map(|i| i.size).sum(),
                items.iter().map(|i| i.files).sum(),
                dirs,
                secs(newest),
                Value::Null,
                mix(scan, &nodes),
                Value::Null,
                if *first { vec!["first"] } else { Vec::new() },
            );
            let items: Vec<Value> = items
                .iter()
                .map(|i| {
                    let mut item = Map::new();
                    item.insert("path".into(), json!(path(scan, i.node)));
                    item.insert(
                        "class".into(),
                        json!(scan.tags[i.node as usize].group_name()),
                    );
                    item.insert("size".into(), json!(i.size));
                    item.insert("files".into(), json!(i.files));
                    item.insert("large".into(), json!(i.large));
                    if let Some(&orig) = scan.dupes.get(&i.node) {
                        item.insert("dup_of".into(), json!(path(scan, orig)));
                    }
                    Value::Object(item)
                })
                .collect();
            m.insert("items".into(), json!(items));
            m
        }
        RowKind::Mismatch { node } => {
            let n = tree.node(*node);
            let info = scan.type_of(*node);
            let mut m = base(
                "mismatch",
                path(scan, *node),
                r.depth,
                n.size,
                1,
                0,
                secs(n.mtime_ns),
                json!(info.name()),
                (json!([]), info.estimated),
                class(scan, *node),
                node_flags(scan, *node),
            );
            if let Some(by_ext) = info.mismatch {
                m.insert("mismatch".into(), json!(label(by_ext).name));
            }
            m
        }
        RowKind::More { count } => base(
            "more",
            "./".to_string(),
            r.depth,
            0,
            *count,
            0,
            Value::Null,
            Value::Null,
            (json!([]), false),
            Value::Null,
            Vec::new(),
        ),
    };
    Some(Value::Object(m))
}

/// `--json-full`: every node in tree order, with the index of the row that
/// stands for it (`null` when it is hidden).
fn entries(scan: &Scan, stats: bool) -> Vec<Value> {
    let tree = &scan.tree;
    let mut visible: Vec<Option<usize>> = vec![None; tree.len()];
    if !stats {
        let hidden = |id: NodeId| crate::chaff::is_hidden(&scan.tags, id);
        let mut mark = |id: NodeId, i: usize, subtree: bool| {
            let end = if subtree { tree.node(id).end } else { id + 1 };
            let mut c = id;
            while c < end {
                if hidden(c) {
                    c = tree.node(c).end;
                    continue;
                }
                visible[c as usize] = Some(i);
                c += 1;
            }
        };
        let mut i = 0;
        for r in &scan.view.rows {
            match &r.kind {
                RowKind::Header | RowKind::Footer { .. } | RowKind::Note(_) => continue,
                RowKind::Entry { node, joined, .. } => {
                    mark(*node, i, false);
                    for &j in joined {
                        mark(j, i, false);
                    }
                }
                RowKind::Summary { node, joined, .. } => {
                    mark(*node, i, false);
                    for &j in joined {
                        mark(j, i, false);
                    }
                    mark(joined.last().copied().unwrap_or(*node), i, true);
                }
                RowKind::Overflow { nodes, .. } => {
                    for &n in nodes {
                        mark(n, i, true);
                    }
                }
                RowKind::Series { series, .. } => {
                    for &n in &series.members {
                        mark(n, i, true);
                    }
                }
                RowKind::Chaff { items, .. } => {
                    for it in items {
                        mark(it.node, i, true);
                    }
                }
                RowKind::Mismatch { node } => mark(*node, i, false),
                RowKind::More { .. } => {}
            }
            i += 1;
        }
    }
    (0..tree.len() as NodeId)
        .map(|id| {
            let n = tree.node(id);
            let mut m = Map::new();
            m.insert("path".into(), json!(path(scan, id)));
            m.insert(
                "kind".into(),
                json!(match n.kind {
                    Kind::Dir => "dir",
                    Kind::File => "file",
                    Kind::Symlink => "symlink",
                    Kind::Fifo => "fifo",
                    Kind::Socket => "socket",
                    Kind::Device => "device",
                }),
            );
            m.insert("size".into(), json!(n.size));
            m.insert("mtime".into(), secs(n.mtime_ns));
            m.insert("mode".into(), json!(layout::mode_string(n.kind, n.mode)));
            m.insert("class".into(), class(scan, id));
            if !n.is_dir() {
                let info = scan.type_of(id);
                if info.name() != "unknown" {
                    m.insert("type".into(), json!(info.name()));
                }
            }
            m.insert("visible_in".into(), json!(visible[id as usize]));
            Value::Object(m)
        })
        .collect()
}
