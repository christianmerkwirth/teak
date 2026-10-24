//! Series: runs of siblings with the same name shape, shown as one row.

use crate::tree::{NodeId, Tree};

/// One row candidate in a directory: a node, or a series of nodes.
#[derive(Clone, Debug)]
pub enum Unit {
    Node(NodeId),
    Series(Box<Series>),
}

#[derive(Clone, Debug)]
pub struct Series {
    pub members: Vec<NodeId>,
}

impl Unit {
    /// The first node of the unit, in name order.
    pub fn first(&self) -> NodeId {
        match self {
            Unit::Node(id) => *id,
            Unit::Series(s) => s.members[0],
        }
    }
}

/// Splits the children of `dir` into units, in name order.
///
/// Post: every child is in exactly one unit.
pub fn group(_tree: &Tree, _dir: NodeId, children: &[NodeId], _min: usize) -> Vec<Unit> {
    children.iter().map(|&id| Unit::Node(id)).collect()
}
