//! The rows of one view. Every output format renders a `View`, so the line
//! budget holds in one place.

use crate::aggregate::Aggs;
use crate::series::{Series, Uniform};
use crate::tree::{NodeId, Tree};

/// Footer counts: (group name, files), high counts first.
pub type ClassCounts = Vec<(&'static str, u64)>;

pub struct View {
    pub rows: Vec<Row>,
    pub budget: usize,
}

/// One output line.
pub struct Row {
    /// 0 for the header, the footer, and a root summary.
    pub depth: u16,
    /// The last row among its siblings (for the tree glyphs).
    pub last: bool,
    pub kind: RowKind,
}

pub enum RowKind {
    Header,
    /// A file, a special file, a symlink, or an open directory. `joined`
    /// holds the directories of a joined chain below `node`.
    Entry {
        node: NodeId,
        joined: Vec<NodeId>,
        open: bool,
    },
    /// A closed directory. A uniform directory (FR-47) carries its type
    /// and median size.
    Summary {
        node: NodeId,
        joined: Vec<NodeId>,
        uniform: Option<Uniform>,
    },
    /// The children of `parent` that do not fit.
    Overflow {
        parent: NodeId,
        dirs: u64,
        files: u64,
        size: u64,
        /// Files in the row, with those inside the directories.
        total_files: u64,
        newest: i64,
        /// The nodes in the row (series members one by one).
        nodes: Vec<NodeId>,
    },
    Series {
        parent: NodeId,
        series: Box<Series>,
    },
    Chaff {
        parent: NodeId,
        items: Vec<ChaffItem>,
        /// The chaff is larger than the rest of the directory.
        first: bool,
    },
    Footer {
        used: usize,
        budget: usize,
        total_files: u64,
        folded: ClassCounts,
        hidden: ClassCounts,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct ChaffItem {
    pub node: NodeId,
    pub size: u64,
    pub files: u64,
    /// 1 MiB or more, or 10% or more of the directory.
    pub large: bool,
}

impl Row {
    /// The files this row stands for (FR-21). An open directory stands for
    /// none: its rows below do.
    pub fn files(&self, tree: &Tree, aggs: &Aggs) -> u64 {
        match &self.kind {
            RowKind::Header | RowKind::Footer { .. } => 0,
            RowKind::Entry { node, open, .. } => {
                if *open || tree.node(*node).is_dir() {
                    0
                } else {
                    1
                }
            }
            RowKind::Summary { node, .. } => aggs.get(tree, *node).files,
            RowKind::Overflow { total_files, .. } => *total_files,
            RowKind::Series { series, .. } => series.members.len() as u64,
            RowKind::Chaff { items, .. } => items.iter().map(|i| i.files).sum(),
        }
    }
}

impl View {
    /// The hidden file count from the footer.
    pub fn hidden_files(&self) -> u64 {
        self.rows
            .iter()
            .find_map(|r| match &r.kind {
                RowKind::Footer { hidden, .. } => Some(hidden.iter().map(|(_, n)| n).sum()),
                _ => None,
            })
            .unwrap_or(0)
    }
}
