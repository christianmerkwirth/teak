//! Samples for the type mix of folded directories (FR-50, PRD §8.2).
//!
//! The files of a folded subtree fall into (directory, extension) buckets.
//! teak types up to `k` files per bucket and applies the result to the
//! whole bucket. Files with no extension are typed one by one, until the
//! global cap.

use std::collections::HashMap;

use super::{ext, names};
use crate::chaff::templates::fnv1a64;
use crate::tree::{Kind, NodeId, Tree, flag};

/// Files per (directory, extension) bucket.
pub const K: usize = 3;

/// The bucket of a file: its directory and a hash of its lower-case
/// extension. `None` for a file with no extension (its own bucket).
pub fn bucket(tree: &Tree, id: NodeId) -> Option<(NodeId, u64)> {
    let e = ext::ext_of(tree.name(id))?;
    Some((tree.node(id).parent, fnv1a64(&e.to_ascii_lowercase())))
}

/// True for a file whose type needs its content: a non-empty regular file
/// with no exact-name rule, not on a pseudo file system.
pub fn needs_content(tree: &Tree, id: NodeId) -> bool {
    let n = tree.node(id);
    n.kind == Kind::File
        && n.size > 0
        && !n.has(flag::PSEUDO_FS)
        && names::by_name(tree.name(id)).is_none()
}

/// Calls `f` for each file below the given nodes (a file stands for
/// itself) that needs its content, in id order. `skip` leaves out hidden
/// subtrees.
pub fn for_each_member(
    tree: &Tree,
    folded: &[NodeId],
    skip: &impl Fn(NodeId) -> bool,
    mut f: impl FnMut(NodeId),
) {
    for &root in folded {
        let mut id = root;
        let end = tree.node(root).end;
        while id < end {
            if skip(id) {
                id = tree.node(id).end;
                continue;
            }
            if needs_content(tree, id) {
                f(id);
            }
            id += 1;
        }
    }
}

/// The files to type, best first: the first file of every bucket (and
/// every file with no extension), then the second file of every bucket, and
/// so on, up to `k` per bucket. Ties go by id, which is name order.
pub fn plan(
    tree: &Tree,
    folded: &[NodeId],
    k: usize,
    skip: impl Fn(NodeId) -> bool,
) -> Vec<NodeId> {
    let mut taken: HashMap<(NodeId, u64), usize> = HashMap::new();
    let mut ranked: Vec<(usize, NodeId)> = Vec::new();
    for_each_member(tree, folded, &skip, |id| match bucket(tree, id) {
        None => ranked.push((0, id)),
        Some(key) => {
            let n = taken.entry(key).or_default();
            if *n < k {
                ranked.push((*n, id));
                *n += 1;
            }
        }
    });
    ranked.sort_unstable();
    ranked.into_iter().map(|(_, id)| id).collect()
}
