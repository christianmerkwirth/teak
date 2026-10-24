//! Samples for the type mix of folded directories (FR-50, PRD §8.2).
//!
//! The files of a folded subtree fall into (directory, extension) buckets.
//! teak types up to `k` files per bucket and applies the result to the
//! whole bucket. Files with no extension are typed one by one, until the
//! global cap.

use super::{ext, names};
use crate::tree::{Kind, NONE, NodeId, Tree, flag};

/// Files per (directory, extension) bucket.
pub const K: usize = 3;

/// The bucket of a file with no extension: its own.
pub const NO_BUCKET: u32 = u32::MAX;

/// A hash of the lower-case extension, or `None` for a name with none.
fn ext_hash(name: &[u8]) -> Option<u64> {
    let e = ext::ext_of(name)?;
    // FNV-1a of the lower-case bytes, with no copy.
    Some(e.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, &b| {
        (h ^ u64::from(b.to_ascii_lowercase())).wrapping_mul(0x0100_0000_01b3)
    }))
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

/// A sample plan.
pub struct Plan {
    /// The files to type, best first, with their bucket.
    pub samples: Vec<(NodeId, u32)>,
    /// Every member file of the folded subtrees, with its bucket.
    pub members: Vec<(NodeId, u32)>,
    /// The number of buckets (bucket ids are `0..buckets`).
    pub buckets: usize,
}

struct Builder<'a> {
    tree: &'a Tree,
    k: usize,
    /// Samples per round: round `r` holds the `r`-th file of each bucket.
    rounds: Vec<Vec<(NodeId, u32)>>,
    members: Vec<(NodeId, u32)>,
    /// Files taken so far, per bucket.
    taken: Vec<usize>,
}

impl Builder<'_> {
    /// Adds one file. `local` holds the buckets of its directory: one per
    /// extension.
    fn add(&mut self, id: NodeId, local: &mut Vec<(u64, u32)>) {
        if !needs_content(self.tree, id) {
            return;
        }
        let Some(h) = ext_hash(self.tree.name(id)) else {
            self.members.push((id, NO_BUCKET));
            self.rounds[0].push((id, NO_BUCKET));
            return;
        };
        let b = match local.iter().find(|(e, _)| *e == h) {
            Some(&(_, b)) => b,
            None => {
                let b = self.taken.len() as u32;
                self.taken.push(0);
                local.push((h, b));
                b
            }
        };
        self.members.push((id, b));
        let n = &mut self.taken[b as usize];
        if *n < self.k {
            self.rounds[*n].push((id, b));
            *n += 1;
        }
    }
}

/// The files to type, best first: the first file of every bucket (and
/// every file with no extension), then the second file of every bucket, and
/// so on, up to `k` per bucket. Within a round, files go in id order (tree
/// order). `skip` leaves out hidden subtrees.
pub fn plan(tree: &Tree, folded: &[NodeId], k: usize, skip: impl Fn(NodeId) -> bool) -> Plan {
    let mut b = Builder {
        tree,
        k,
        rounds: vec![Vec::new(); k.max(1)],
        members: Vec::new(),
        taken: Vec::new(),
    };
    // Loose files of an overflow row share one directory.
    let mut loose: (NodeId, Vec<(u64, u32)>) = (NONE, Vec::new());
    // The open directories of the pre-order walk: (end, buckets).
    let mut stack: Vec<(NodeId, Vec<(u64, u32)>)> = Vec::new();
    for &root in folded {
        if !tree.node(root).is_dir() {
            let parent = tree.node(root).parent;
            if loose.0 != parent {
                loose = (parent, Vec::new());
            }
            if !skip(root) {
                b.add(root, &mut loose.1);
            }
            continue;
        }
        let end = tree.node(root).end;
        let mut id = root;
        while id < end {
            if skip(id) {
                id = tree.node(id).end;
                continue;
            }
            while stack.last().is_some_and(|(e, _)| id >= *e) {
                stack.pop();
            }
            let node = tree.node(id);
            if node.is_dir() {
                stack.push((node.end, Vec::new()));
            } else if let Some((_, local)) = stack.last_mut() {
                b.add(id, local);
            }
            id += 1;
        }
        stack.clear();
    }
    Plan {
        samples: b.rounds.into_iter().flatten().collect(),
        members: b.members,
        buckets: b.taken.len(),
    }
}
