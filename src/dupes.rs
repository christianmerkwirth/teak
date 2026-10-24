//! Duplicate files for `--dupes` (FR-43).
//!
//! Same size, then the same head and tail, then the same full hash. Every
//! read goes through `io`. Empty files are never duplicates: they are in
//! class `empty`.

use std::collections::HashMap;
use std::hash::Hasher;

use crate::fnv::Fnv;
use crate::tree::{Kind, NodeId, Tree, flag};

/// The bytes at each end that the quick check reads.
const END: usize = 4096;

/// The copies of the `visible` files: copy → original. The original of a
/// group is its first visible file in path order; every other member is a
/// copy, visible or not. `skip` leaves out nodes (folded or hidden ones).
pub fn find(
    tree: &Tree,
    visible: &[NodeId],
    skip: impl Fn(NodeId) -> bool,
) -> HashMap<NodeId, NodeId> {
    let candidate = |id: NodeId| {
        let n = tree.node(id);
        n.kind == Kind::File && n.size > 0 && !n.has(flag::PSEUDO_FS | flag::LINK_DUP)
    };
    let mut wanted: HashMap<u64, Vec<NodeId>> = HashMap::new();
    for &v in visible {
        if candidate(v) {
            wanted.entry(tree.node(v).size).or_default();
        }
    }
    let mut out = HashMap::new();
    if wanted.is_empty() {
        return out;
    }
    let mut id: NodeId = 0;
    while (id as usize) < tree.len() {
        if skip(id) {
            id = tree.node(id).end;
            continue;
        }
        if candidate(id)
            && let Some(group) = wanted.get_mut(&tree.node(id).size)
        {
            group.push(id);
        }
        id += 1;
    }
    let visible: std::collections::HashSet<NodeId> = visible.iter().copied().collect();
    let mut sizes: Vec<u64> = wanted.keys().copied().collect();
    sizes.sort_unstable();
    for size in sizes {
        let group = &wanted[&size];
        if group.len() < 2 {
            continue;
        }
        for quick in split(group, |id| quick_hash(tree, id, size)) {
            if quick.len() < 2 || !quick.iter().any(|m| visible.contains(m)) {
                continue;
            }
            for same in split(&quick, |id| full_hash(tree, id)) {
                let Some(&orig) = same.iter().find(|m| visible.contains(m)) else {
                    continue;
                };
                for &m in &same {
                    if m != orig {
                        out.insert(m, orig);
                    }
                }
            }
        }
    }
    out
}

/// Groups `ids` by `key`, in id order; a file that cannot be read
/// (`None`) is in no group.
fn split(ids: &[NodeId], mut key: impl FnMut(NodeId) -> Option<Vec<u8>>) -> Vec<Vec<NodeId>> {
    let mut groups: Vec<(Vec<u8>, Vec<NodeId>)> = Vec::new();
    for &id in ids {
        let Some(k) = key(id) else { continue };
        match groups.iter_mut().find(|(g, _)| *g == k) {
            Some((_, members)) => members.push(id),
            None => groups.push((k, vec![id])),
        }
    }
    groups.into_iter().map(|(_, m)| m).collect()
}

/// FNV-1a of the first and the last [`END`] bytes (the whole file when it
/// is small).
fn quick_hash(tree: &Tree, id: NodeId, size: u64) -> Option<Vec<u8>> {
    let path = tree.path(id);
    let mut h = Fnv::default();
    if size <= 2 * END as u64 {
        h.write(&crate::io::read_at(&path, 0, size as usize).ok()?);
    } else {
        h.write(&crate::io::read_at(&path, 0, END).ok()?);
        h.write(&crate::io::read_at(&path, size - END as u64, END).ok()?);
    }
    Some(h.finish().to_le_bytes().to_vec())
}

/// The SHA-1 of the whole file.
fn full_hash(tree: &Tree, id: NodeId) -> Option<Vec<u8>> {
    let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
    crate::io::read_each(&tree.path(id), |b| hasher.update(b)).ok()?;
    Some(hasher.try_finalize().ok()?.as_bytes().to_vec())
}
