//! Git status in process with `gix` (FR-57, design.md §Git status).
//!
//! teak reads the index, `HEAD`, and the refs. It compares stat data and
//! hashes the raw file bytes (through `io`) only when the stat data changed
//! or the entry is racy. It never applies filters, never asks an
//! fsmonitor, and never runs hooks or the `git` program (FR-76). Untracked
//! files come from teak's own walk; ignored files from its gitignore tags.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::tree::{Kind, NodeId, ROOT, Tree};

/// The status of one entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    Modified,
    Added,
    Deleted,
    Untracked,
    Ignored,
}

impl Code {
    pub fn letter(self) -> &'static str {
        match self {
            Code::Modified => "M",
            Code::Added => "A",
            Code::Deleted => "D",
            Code::Untracked => "?",
            Code::Ignored => "!",
        }
    }
}

/// The branch and its distance to the upstream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHead {
    /// The branch name, or the short commit id of a detached `HEAD`.
    pub branch: String,
    pub ahead: u64,
    pub behind: u64,
}

/// The status of the files in one tree.
#[derive(Clone, Debug, Default)]
pub struct GitStatus {
    /// Nodes with a status, by id.
    files: Vec<(NodeId, Code)>,
    /// For each deleted entry: the deepest directory of the tree that holds
    /// its path. By id.
    deleted: Vec<NodeId>,
}

impl GitStatus {
    pub fn of(&self, id: NodeId) -> Option<Code> {
        self.files
            .binary_search_by_key(&id, |(n, _)| *n)
            .ok()
            .map(|i| self.files[i].1)
    }

    /// Changed files in the subtree of `id` (with `id`): modified, added,
    /// deleted, and untracked. Ignored files do not count.
    pub fn changed_below(&self, tree: &Tree, id: NodeId) -> u64 {
        let end = tree.node(id).end;
        let lo = self.files.partition_point(|(n, _)| *n < id);
        let hi = self.files.partition_point(|(n, _)| *n < end);
        let files = self.files[lo..hi]
            .iter()
            .filter(|(_, c)| *c != Code::Ignored)
            .count();
        let lo = self.deleted.partition_point(|&n| n < id);
        let hi = self.deleted.partition_point(|&n| n < end);
        (files + hi - lo) as u64
    }

    /// Changed files in the whole tree.
    pub fn changed(&self, tree: &Tree) -> u64 {
        self.changed_below(tree, ROOT)
    }
}

pub struct GitRepo {
    repo: gix::Repository,
    /// The scan root relative to the work tree, `/`-joined; empty for the
    /// work tree itself.
    prefix: Vec<u8>,
}

/// Opens the repo that holds `root`, if there is one with a work tree.
///
/// The repo opens isolated: no system or user config, no `git` program to
/// find them, and `core.fsmonitor` off.
pub fn open(root: &Path) -> Option<GitRepo> {
    let real = std::fs::canonicalize(root).ok()?;
    let start = if real.is_dir() {
        real.clone()
    } else {
        real.parent()?.to_path_buf()
    };
    let top = start.ancestors().find(|p| p.join(".git").exists())?;
    let opts = gix::open::Options::isolated().config_overrides(["core.fsmonitor=false"]);
    let repo = gix::open_opts(top, opts).ok()?;
    let work: PathBuf = std::fs::canonicalize(repo.workdir()?).ok()?;
    let rel = real.strip_prefix(&work).ok()?;
    let prefix = rel.as_os_str().as_encoded_bytes().to_vec();
    Some(GitRepo { repo, prefix })
}

/// The most commits that ahead/behind counts.
const WALK_LIMIT: usize = 100_000;

impl GitRepo {
    pub fn head(&self) -> Option<GitHead> {
        let head = self.repo.head().ok()?;
        let branch = match head.referent_name() {
            Some(name) => name.shorten().to_string(),
            None => {
                let id = head.id()?;
                id.to_hex_with_len(7).to_string()
            }
        };
        let (mut ahead, mut behind) = (0, 0);
        if let (Some(name), Some(local)) = (head.referent_name(), head.id())
            && let Some(Ok(up)) = self
                .repo
                .branch_remote_tracking_ref_name(name, gix::remote::Direction::Fetch)
            && let Ok(r) = self.repo.find_reference(up.as_ref())
            && let Ok(remote) = r.into_fully_peeled_id()
        {
            let (local, remote) = (local.detach(), remote.detach());
            let count = |from, hide| {
                self.repo
                    .rev_walk([from])
                    .with_hidden([hide])
                    .all()
                    .map_or(0, |w| w.take(WALK_LIMIT).filter(Result::is_ok).count())
                    as u64
            };
            ahead = count(local, remote);
            behind = count(remote, local);
        }
        Some(GitHead {
            branch,
            ahead,
            behind,
        })
    }

    /// The path of an index or tree entry relative to the scan root, or
    /// `None` when it is outside the root.
    fn below_root<'p>(&self, path: &'p [u8]) -> Option<&'p [u8]> {
        if self.prefix.is_empty() {
            return Some(path);
        }
        let rest = path.strip_prefix(self.prefix.as_slice())?;
        match rest.first() {
            None => Some(rest),
            Some(b'/') => Some(&rest[1..]),
            Some(_) => None,
        }
    }

    /// The status of the files in `tree`. `ignored` tells which nodes the
    /// gitignore rules match.
    pub fn status(&self, tree: &Tree, ignored: impl Fn(NodeId) -> bool) -> GitStatus {
        let mut files: Vec<(NodeId, Code)> = Vec::new();
        let mut deleted: Vec<NodeId> = Vec::new();
        let mut tracked = vec![false; tree.len()];
        let mut nodes = Resolver::new(tree);
        let head = self.head_blobs();
        let Ok(index) = self.repo.index_or_empty() else {
            return GitStatus::default();
        };
        let stamp = index.timestamp();
        let stamp = (stamp.unix_seconds(), stamp.nanoseconds());
        let mut in_index: std::collections::HashSet<&[u8]> = Default::default();
        for entry in index.entries() {
            let full: &[u8] = entry.path(&index);
            let Some(rel) = self.below_root(full) else {
                continue;
            };
            in_index.insert(full);
            if entry.mode == gix::index::entry::Mode::COMMIT {
                if let Ok(id) = nodes.find(rel) {
                    tracked[id as usize] = true;
                }
                continue;
            }
            let id = match nodes.find(rel) {
                Ok(id) => id,
                Err(dir) => {
                    deleted.push(dir);
                    continue;
                }
            };
            tracked[id as usize] = true;
            let staged = match head.as_ref().map(|h| h.get(full)) {
                None | Some(None) => Some(Code::Added),
                Some(Some(oid)) if *oid != entry.id => Some(Code::Modified),
                Some(Some(_)) => None,
            };
            let flags = entry.flags;
            let staged = if entry.stage_raw() != 0 {
                Some(Code::Modified)
            } else if flags.contains(gix::index::entry::Flags::INTENT_TO_ADD) {
                Some(Code::Added)
            } else {
                staged
            };
            let skip = flags.intersects(
                gix::index::entry::Flags::SKIP_WORKTREE | gix::index::entry::Flags::ASSUME_VALID,
            );
            let changed = !skip && self.worktree_changed(tree, id, entry, stamp);
            let code = match (staged, changed) {
                (Some(Code::Added), _) => Some(Code::Added),
                (Some(c), _) => Some(c),
                (None, true) => Some(Code::Modified),
                (None, false) => None,
            };
            if let Some(c) = code {
                files.push((id, c));
            }
        }
        // Deleted and staged for deletion: in HEAD, not in the index.
        if let Some(head) = &head {
            for path in head.keys() {
                if in_index.contains(path.as_slice()) {
                    continue;
                }
                if let Some(rel) = self.below_root(path)
                    && let Err(dir) = nodes.find(rel)
                {
                    deleted.push(dir);
                }
            }
        }
        // Untracked and ignored: files of the walk that the index does not
        // hold. `.git/` is not part of the work tree.
        let mut id: NodeId = 0;
        while (id as usize) < tree.len() {
            let n = tree.node(id);
            if n.is_dir() {
                if id != ROOT && tree.name(id) == b".git" {
                    id = n.end;
                    continue;
                }
                if tracked[id as usize] {
                    // A submodule.
                    id = n.end;
                    continue;
                }
            } else if !tracked[id as usize] {
                let code = if ignored(id) {
                    Code::Ignored
                } else {
                    Code::Untracked
                };
                files.push((id, code));
            }
            id += 1;
        }
        files.sort_by_key(|(n, _)| *n);
        files.dedup_by_key(|(n, _)| *n);
        deleted.sort_unstable();
        GitStatus { files, deleted }
    }

    /// Path → blob id for every file in the `HEAD` tree; `None` before the
    /// first commit.
    fn head_blobs(&self) -> Option<HashMap<Vec<u8>, gix::ObjectId>> {
        let tree = self.repo.head_tree().ok()?;
        let mut rec = gix::traverse::tree::Recorder::default();
        tree.traverse().breadthfirst(&mut rec).ok()?;
        Some(
            rec.records
                .into_iter()
                .filter(|e| !e.mode.is_tree())
                .map(|e| (e.filepath.to_vec(), e.oid))
                .collect(),
        )
    }

    /// The work-tree file differs from its index entry: another kind, the
    /// executable bit, the size, or (after a stat change, or when the entry
    /// is racy) the content.
    fn worktree_changed(
        &self,
        tree: &Tree,
        id: NodeId,
        entry: &gix::index::Entry,
        stamp: (i64, u32),
    ) -> bool {
        use gix::index::entry::Mode;
        let n = tree.node(id);
        let link = entry.mode == Mode::SYMLINK;
        match n.kind {
            Kind::Symlink if link => {}
            Kind::File if !link => {}
            _ => return true,
        }
        if !link && (entry.mode == Mode::FILE_EXECUTABLE) != (n.mode & 0o100 != 0) {
            return true;
        }
        if u64::from(entry.stat.size) != n.size & 0xFFFF_FFFF {
            return true;
        }
        let secs = n.mtime_ns.div_euclid(1_000_000_000);
        let nsecs = n.mtime_ns.rem_euclid(1_000_000_000) as u32;
        let m = entry.stat.mtime;
        let same_time = i64::from(m.secs) == secs && m.nsecs == nsecs;
        let racy = (i64::from(m.secs), m.nsecs) >= stamp;
        if same_time && !racy {
            return false;
        }
        let kind = self.repo.object_hash();
        let mut hasher = gix::hash::hasher(kind);
        if link {
            let Some(target) = tree.link_targets.get(&id) else {
                return true;
            };
            hasher.update(&gix::objs::encode::loose_header(
                gix::objs::Kind::Blob,
                target.len() as u64,
            ));
            hasher.update(target);
        } else {
            hasher.update(&gix::objs::encode::loose_header(
                gix::objs::Kind::Blob,
                n.size,
            ));
            if crate::io::read_each(&tree.path(id), |b| hasher.update(b)).is_err() {
                return true;
            }
        }
        hasher.try_finalize().map_or(true, |oid| oid != entry.id)
    }
}

/// Finds nodes by `/`-joined paths below the root, with a name map per
/// directory that is made on first use.
struct Resolver<'a> {
    tree: &'a Tree,
    dirs: HashMap<NodeId, HashMap<&'a [u8], NodeId>>,
}

impl<'a> Resolver<'a> {
    fn new(tree: &'a Tree) -> Resolver<'a> {
        Resolver {
            tree,
            dirs: HashMap::new(),
        }
    }

    /// The node at `rel`, or the deepest directory on the way.
    fn find(&mut self, rel: &[u8]) -> Result<NodeId, NodeId> {
        let tree = self.tree;
        let mut cur = ROOT;
        if rel.is_empty() {
            return Ok(ROOT);
        }
        for part in rel.split(|&b| b == b'/') {
            if !tree.node(cur).is_dir() {
                return Err(tree.node(cur).parent);
            }
            let names = self
                .dirs
                .entry(cur)
                .or_insert_with(|| tree.children(cur).map(|c| (tree.name(c), c)).collect());
            match names.get(part) {
                Some(&c) => cur = c,
                None => return Err(cur),
            }
        }
        Ok(cur)
    }
}
