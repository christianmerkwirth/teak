//! `--why PATH` (FR-60): the rule, the action, the salience terms, and the
//! decision for one path. Also the path checks of `--focus` and `--why`.

use std::path::Path;

use crate::Scan;
use crate::chaff::{Action, ChaffTag};
use crate::cli::UsageError;
use crate::salience::{Terms, Weights};
use crate::tree::{NodeId, ROOT, Tree};
use crate::view::RowKind;

/// The path of `path` below `root`, `/`-joined. `path` is relative to the
/// current directory. A final symlink is not followed. Fails when the path
/// does not exist or is outside the root.
pub fn resolve(root: &Path, path: &Path, flag: &str) -> Result<Vec<u8>, UsageError> {
    let err = |why: String| UsageError(format!("{flag} {}: {why}", path.display()));
    let real_root = std::fs::canonicalize(root).map_err(|e| err(crate::tree::io_message(&e)))?;
    let abs = std::path::absolute(path).map_err(|e| err(crate::tree::io_message(&e)))?;
    let full = match (abs.parent(), abs.file_name()) {
        (Some(parent), Some(name)) => std::fs::canonicalize(parent)
            .map_err(|e| err(crate::tree::io_message(&e)))?
            .join(name),
        _ => std::fs::canonicalize(&abs).map_err(|e| err(crate::tree::io_message(&e)))?,
    };
    if std::fs::symlink_metadata(&full).is_err() {
        return Err(err("no such file or directory".to_string()));
    }
    let rel = full
        .strip_prefix(&real_root)
        .map_err(|_| err(format!("not inside {}", root.display())))?;
    Ok(rel.as_os_str().as_encoded_bytes().to_vec())
}

/// The node at `rel`, or a usage error.
pub fn find(tree: &Tree, rel: &[u8], path: &Path, flag: &str) -> Result<NodeId, UsageError> {
    tree.find_bytes(rel).ok_or_else(|| {
        UsageError(format!(
            "{flag} {}: not in the scanned tree",
            path.display()
        ))
    })
}

/// The `--why` lines for `id`. `rule` is the name tag and its rule text
/// from [`crate::chaff::Classifier::why`]; `terms` are the salience terms
/// that selection used.
pub fn why(
    scan: &Scan,
    id: NodeId,
    rule: &(ChaffTag, String),
    terms: &Terms,
    w: &Weights,
) -> Vec<String> {
    let tree = &scan.tree;
    let tag = scan.tags[id as usize];
    let mut lines = vec![format!("path: {}", display(tree, id))];
    let class = match tag.class {
        None if rule.1.is_empty() => "class: none".to_string(),
        None => format!("class: none ({})", rule.1),
        Some(c) => {
            // A content check changes the tag after the names.
            let text = if tag.source == rule.0.source && tag.class == rule.0.class {
                rule.1.clone()
            } else {
                let mut t = "content check".to_string();
                if let Some(l) = scan.template_label(id) {
                    t.push_str(&format!(": {l}"));
                }
                t
            };
            if text.is_empty() {
                format!("class: {}", c.name())
            } else {
                format!("class: {} ({text})", c.name())
            }
        }
    };
    lines.push(class);
    lines.push(format!("action: {}", action_name(tag.action)));
    let mut s = format!(
        "salience: size {:.2} · recency {:.2} · git {:.2} · rarity {:.2} · role {:.2} · anomaly {:.2}",
        terms.size, terms.recency, terms.git, terms.rarity, terms.role, terms.anomaly
    );
    if terms.factor != 1.0 {
        s.push_str(&format!(" · chaff ×{:.1}", terms.factor));
    }
    if terms.focus > 0.0 {
        s.push_str(&format!(" · focus +{:.2}", terms.focus));
    }
    s.push_str(&format!(" = {:.2}", terms.score(w)));
    lines.push(s);
    lines.push(format!("decision: {}", decision(scan, id)));
    lines
}

fn action_name(a: Action) -> &'static str {
    match a {
        Action::Show => "show",
        Action::Dim => "dim",
        Action::Fold => "fold",
        Action::Hide => "hide",
    }
}

/// The path of a node as `--why` shows it: relative to the root, with a
/// `/` after a directory; the root is its label.
fn display(tree: &Tree, id: NodeId) -> String {
    let mut s = if id == ROOT {
        tree.root_label.clone()
    } else {
        crate::layout::escape(&tree.rel_path(id))
    };
    if tree.node(id).is_dir() && !s.ends_with('/') {
        s.push('/');
    }
    s
}

fn within(tree: &Tree, id: NodeId, top: NodeId) -> bool {
    top <= id && id < tree.node(top).end
}

/// Where the node is in the view.
fn decision(scan: &Scan, id: NodeId) -> String {
    let tree = &scan.tree;
    let hidden = {
        let mut cur = id;
        loop {
            if crate::chaff::is_hidden(&scan.tags, cur) {
                break true;
            }
            if cur == ROOT {
                break false;
            }
            cur = tree.node(cur).parent;
        }
    };
    if hidden {
        return "hidden, counted in footer".to_string();
    }
    for row in &scan.view.rows {
        match &row.kind {
            RowKind::Entry { node, joined, .. } if *node == id || joined.contains(&id) => {
                return "visible".to_string();
            }
            RowKind::Mismatch { node } if *node == id => return "visible".to_string(),
            RowKind::Summary { node, joined, .. } => {
                if *node == id || joined.contains(&id) {
                    return "visible".to_string();
                }
                let tail = joined.last().copied().unwrap_or(*node);
                if within(tree, id, tail) {
                    return format!("folded into {}", display(tree, tail));
                }
            }
            RowKind::Overflow { parent, nodes, .. }
                if nodes.iter().any(|&n| within(tree, id, n)) =>
            {
                return format!("in overflow of {}", display(tree, *parent));
            }
            RowKind::Series { series, .. } if series.members.contains(&id) => {
                return format!(
                    "in series {}",
                    crate::layout::series_name(series, &crate::layout::UNICODE)
                );
            }
            RowKind::Chaff { parent, items, .. }
                if items.iter().any(|it| within(tree, id, it.node)) =>
            {
                return format!("folded into the chaff line of {}", display(tree, *parent));
            }
            _ => {}
        }
    }
    "hidden, counted in footer".to_string()
}
