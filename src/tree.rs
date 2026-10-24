//! The arena of scanned entries.
//!
//! Nodes are numbered in depth-first pre-order, with children in name-byte
//! order. So the ids do not depend on the walk order, and the subtree of a
//! node is the id range `id..node.end`.

use std::collections::HashMap;
use std::fmt;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub type NodeId = u32;

/// No node.
pub const NONE: NodeId = u32::MAX;

/// The root of every tree.
pub const ROOT: NodeId = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Dir,
    File,
    Symlink,
    Fifo,
    Socket,
    Device,
}

/// Bits of [`Node::flags`].
pub mod flag {
    /// `⊘`: teak cannot read the entry or list the directory.
    pub const NO_ACCESS: u16 = 1 << 0;
    /// `↻`: a symlink that points back into its own ancestors (`--follow`).
    pub const LOOP: u16 = 1 << 1;
    /// A symlink whose target does not exist.
    pub const BROKEN: u16 = 1 << 2;
    /// `⊞`: a mount point that `-x` did not enter.
    pub const MOUNT: u16 = 1 << 3;
    /// On a pseudo file system such as `/proc`: never read content.
    pub const PSEUDO_FS: u16 = 1 << 4;
    /// A likely secret: never show content.
    pub const SECRET: u16 = 1 << 7;
    /// The scan root.
    pub const ROOT: u16 = 1 << 8;
    /// A symlink that `--follow` followed; the kind is the target's kind.
    pub const FOLLOWED: u16 = 1 << 9;
    /// Walk-time only: a non-directory with more than one hard link.
    pub(crate) const MULTI_LINK: u16 = 1 << 15;
}

#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub parent: NodeId,
    pub first_child: NodeId,
    pub next_sibling: NodeId,
    /// One past the last id of this subtree.
    pub end: NodeId,
    pub(crate) name_off: u32,
    pub(crate) name_len: u16,
    pub kind: Kind,
    pub flags: u16,
    /// Bytes; 0 for a repeated hard link.
    pub size: u64,
    pub mtime_ns: i64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub dev: u64,
    pub ino: u64,
}

impl Node {
    pub fn is_dir(&self) -> bool {
        self.kind == Kind::Dir
    }

    pub fn has(&self, f: u16) -> bool {
        self.flags & f != 0
    }
}

/// One error from the scan, for one line on stderr.
#[derive(Clone, Debug)]
pub struct ScanError {
    pub path: PathBuf,
    pub message: String,
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot read '{}': {}", self.path.display(), self.message)
    }
}

impl ScanError {
    pub fn new(path: &Path, err: &std::io::Error) -> ScanError {
        ScanError {
            path: path.to_path_buf(),
            message: io_message(err),
        }
    }
}

/// The OS error text without the `(os error N)` tail.
pub fn io_message(err: &std::io::Error) -> String {
    let text = err.to_string();
    match text.find(" (os error") {
        Some(i) => text[..i].to_string(),
        None => text,
    }
}

pub struct Tree {
    pub nodes: Vec<Node>,
    pub(crate) names: Vec<u8>,
    /// Symlink targets, as `readlink` gives them.
    pub link_targets: HashMap<NodeId, Vec<u8>>,
    /// The root path to open entries with.
    pub root_path: PathBuf,
    /// The root name for the header (see [`root_label`]).
    pub root_label: String,
    /// Seconds since the epoch: `SOURCE_DATE_EPOCH` or now.
    pub scan_time: i64,
    /// Entries that teak could not read, in id order.
    pub errors: Vec<ScanError>,
}

impl Tree {
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    /// The raw name bytes. The root has the path as given.
    pub fn name(&self, id: NodeId) -> &[u8] {
        let n = &self.nodes[id as usize];
        &self.names[n.name_off as usize..][..n.name_len as usize]
    }

    pub fn children(&self, id: NodeId) -> Children<'_> {
        Children {
            tree: self,
            next: self.nodes[id as usize].first_child,
        }
    }

    /// The ids of the subtree below `id`, without `id`.
    pub fn descendants(&self, id: NodeId) -> std::ops::Range<NodeId> {
        id + 1..self.nodes[id as usize].end
    }

    /// The path relative to the root, joined with `/`. Empty for the root.
    pub fn rel_path(&self, id: NodeId) -> Vec<u8> {
        let mut parts = Vec::new();
        let mut cur = id;
        while cur != ROOT && cur != NONE {
            parts.push(self.name(cur));
            cur = self.nodes[cur as usize].parent;
        }
        let mut out = Vec::new();
        for (i, part) in parts.iter().rev().enumerate() {
            if i > 0 {
                out.push(b'/');
            }
            out.extend_from_slice(part);
        }
        out
    }

    /// The path to open the entry with.
    pub fn path(&self, id: NodeId) -> PathBuf {
        if id == ROOT {
            return self.root_path.clone();
        }
        self.root_path
            .join(std::ffi::OsStr::from_bytes(&self.rel_path(id)))
    }

    /// The depth below the root (the root is 0).
    pub fn depth(&self, id: NodeId) -> usize {
        let mut d = 0;
        let mut cur = id;
        while cur != ROOT {
            cur = self.nodes[cur as usize].parent;
            d += 1;
        }
        d
    }

    /// A dotfile below the root.
    pub fn is_dotfile(&self, id: NodeId) -> bool {
        id != ROOT && self.name(id).first() == Some(&b'.')
    }

    /// The number of non-directory nodes in the subtree of `id` (with `id`).
    pub fn file_count(&self, id: NodeId) -> u64 {
        (id..self.nodes[id as usize].end)
            .filter(|&i| !self.nodes[i as usize].is_dir())
            .count() as u64
    }
}

pub struct Children<'a> {
    tree: &'a Tree,
    next: NodeId,
}

impl Iterator for Children<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        if self.next == NONE {
            return None;
        }
        let id = self.next;
        self.next = self.tree.nodes[id as usize].next_sibling;
        Some(id)
    }
}

/// The root name for the header: the path as given, but `.` and `..` (and
/// paths that end in them) become the base name of the real directory.
pub fn root_label(path: &Path) -> String {
    let given = path.to_string_lossy().into_owned();
    let last = path.components().next_back();
    let dotted = matches!(
        last,
        Some(std::path::Component::CurDir | std::path::Component::ParentDir)
    ) || given.ends_with("/.");
    if !dotted {
        return given;
    }
    match std::fs::canonicalize(path) {
        Ok(real) => match real.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            None => "/".to_string(),
        },
        Err(_) => given,
    }
}
