//! Per-directory totals in one bottom-up pass.

use crate::tree::{NONE, NodeId, Tree};

/// Totals for a directory subtree, or one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirAgg {
    pub size: u64,
    /// Non-directory entries.
    pub files: u64,
    pub dirs: u64,
    /// The newest and oldest file mtimes (ns); `i64::MIN`/`MAX` if no files.
    pub newest: i64,
    pub oldest: i64,
}

impl DirAgg {
    const EMPTY: DirAgg = DirAgg {
        size: 0,
        files: 0,
        dirs: 0,
        newest: i64::MIN,
        oldest: i64::MAX,
    };

    pub fn add(&mut self, other: &DirAgg) {
        self.size += other.size;
        self.files += other.files;
        self.dirs += other.dirs;
        self.newest = self.newest.max(other.newest);
        self.oldest = self.oldest.min(other.oldest);
    }
}

/// Totals for every directory. Files get theirs on the fly.
pub struct Aggs {
    slot: Vec<u32>,
    dirs: Vec<DirAgg>,
}

impl Aggs {
    pub fn get(&self, tree: &Tree, id: NodeId) -> DirAgg {
        match self.slot[id as usize] {
            NONE => {
                let n = tree.node(id);
                DirAgg {
                    size: n.size,
                    files: 1,
                    dirs: 0,
                    newest: n.mtime_ns,
                    oldest: n.mtime_ns,
                }
            }
            s => self.dirs[s as usize],
        }
    }
}

/// Sums each subtree. Entries for which `skip` is true do not count, and
/// neither does anything below them (hidden entries are only in the footer).
///
/// Post: with no skipped entries, the root's `files` is the number of
/// non-directory nodes.
pub fn compute(tree: &Tree, skip: impl Fn(NodeId) -> bool) -> Aggs {
    let mut slot = vec![NONE; tree.len()];
    let mut dirs = Vec::new();
    for (id, node) in tree.nodes.iter().enumerate() {
        if node.is_dir() {
            slot[id] = dirs.len() as u32;
            dirs.push(DirAgg::EMPTY);
        }
    }
    let mut aggs = Aggs { slot, dirs };
    for id in (0..tree.len() as NodeId).rev() {
        let node = tree.node(id);
        if node.parent == NONE || skip(id) {
            continue;
        }
        let mut own = aggs.get(tree, id);
        if node.is_dir() {
            own.dirs += 1;
        }
        let p = aggs.slot[node.parent as usize] as usize;
        aggs.dirs[p].add(&own);
    }
    aggs
}
