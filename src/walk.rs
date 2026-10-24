//! The stat-only parallel walk.
//!
//! The `ignore` walker runs with all its filters off, so every entry is seen
//! (gitignore rules are matched later, in `chaff`). Each walk thread keeps
//! its records in its own shard: the name bytes, the stat data, and the id of
//! the parent directory, not the full path (phase-0 Plan A2). After the walk,
//! [`build`] links the records into a [`Tree`] numbered in a fixed order.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use ignore::{WalkBuilder, WalkState};

use crate::tree::{Kind, NONE, Node, NodeId, ROOT, ScanError, Tree, flag, root_label};

pub struct WalkOpts {
    /// `--follow`: walk into symlinked directories.
    pub follow: bool,
    /// `-x`: do not enter other file systems.
    pub one_fs: bool,
    pub threads: usize,
}

/// Walks `root` and returns the tree. Reads no file content.
///
/// A missing root is an `Err`. Entries that cannot be read get the
/// `NO_ACCESS` flag and an entry in [`Tree::errors`].
pub fn scan(root: &Path, opts: &WalkOpts, scan_time: i64) -> Result<Tree, ScanError> {
    let meta = fs::symlink_metadata(root).map_err(|e| ScanError::new(root, &e))?;
    // A symlink given as the root is followed, like `ls` does.
    let walk_root = if meta.file_type().is_symlink() && root.is_dir() {
        root.join(".")
    } else {
        root.to_path_buf()
    };
    let ctx = Ctx {
        root_name: root.as_os_str().as_bytes(),
        dirs: DirMap::new(),
        next_dir: AtomicU32::new(0),
        root_dev: fs::metadata(&walk_root).map_or(meta.dev(), |m| m.dev()),
        opts,
        shards: Mutex::new(Vec::new()),
    };
    WalkBuilder::new(&walk_root)
        .standard_filters(false)
        .follow_links(opts.follow)
        .threads(opts.threads.max(1))
        .build_parallel()
        .run(|| {
            let mut visitor = Visitor {
                ctx: &ctx,
                shard: Shard::default(),
                parent_cache: (Vec::new(), NONE),
                devs: HashMap::new(),
            };
            Box::new(move |entry| visitor.visit(entry))
        });
    let shards = std::mem::take(&mut *ctx.shards.lock().unwrap_or_else(|e| e.into_inner()));
    let dir_count = ctx.next_dir.load(Ordering::Relaxed);
    let mut tree = build(shards, dir_count);
    tree.root_path = walk_root;
    tree.root_label = root_label(root);
    tree.scan_time = scan_time;
    Ok(tree)
}

/// Shared state of one walk.
struct Ctx<'a> {
    root_name: &'a [u8],
    /// Directory path bytes → directory id.
    dirs: DirMap,
    next_dir: AtomicU32,
    root_dev: u64,
    opts: &'a WalkOpts,
    shards: Mutex<Vec<Shard>>,
}

/// Records of one walk thread. Until [`build`], `Node::parent` holds the
/// parent's directory id and `Node::first_child` the node's own directory id.
#[derive(Default)]
struct Shard {
    nodes: Vec<Node>,
    names: Vec<u8>,
    links: Vec<(u32, Vec<u8>)>,
    errors: Vec<(ErrorAt, ScanError)>,
}

enum ErrorAt {
    /// A directory that could not be listed.
    Dir(u32),
    /// A node in this shard.
    Node(u32),
    /// No node (for example, the entry vanished).
    Nowhere,
}

struct Visitor<'a> {
    ctx: &'a Ctx<'a>,
    shard: Shard,
    /// The last parent lookup: siblings often come one after another.
    parent_cache: (Vec<u8>, u32),
    /// Device → on a pseudo file system.
    devs: HashMap<u64, bool>,
}

impl Drop for Visitor<'_> {
    fn drop(&mut self) {
        let shard = std::mem::take(&mut self.shard);
        self.ctx
            .shards
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(shard);
    }
}

impl Visitor<'_> {
    fn visit(&mut self, entry: Result<ignore::DirEntry, ignore::Error>) -> WalkState {
        match entry {
            Ok(dent) => self.visit_ok(&dent),
            Err(err) => {
                self.visit_err(&err, None);
                WalkState::Continue
            }
        }
    }

    fn visit_ok(&mut self, dent: &ignore::DirEntry) -> WalkState {
        let path = dent.path();
        let depth = dent.depth();
        let meta = match dent.metadata() {
            Ok(m) => m,
            Err(err) => {
                self.visit_err(&err, Some(path));
                return WalkState::Continue;
            }
        };
        let kind = kind_of(&meta.file_type());
        let mut flags = 0;
        if depth == 0 {
            flags |= flag::ROOT;
        }
        if dent.path_is_symlink() && kind != Kind::Symlink {
            flags |= flag::FOLLOWED;
        }
        let mut state = WalkState::Continue;
        if kind == Kind::Dir && depth > 0 && self.ctx.opts.one_fs && meta.dev() != self.ctx.root_dev
        {
            flags |= flag::MOUNT;
            state = WalkState::Skip;
        }
        let idx = self.push(path, depth, kind, flags, &meta);
        if kind == Kind::Symlink || flags & flag::FOLLOWED != 0 {
            self.link_target(idx, path, kind == Kind::Symlink);
        }
        state
    }

    fn visit_err(&mut self, err: &ignore::Error, path: Option<&Path>) {
        match failure(err, path) {
            Failure::Loop(child) => {
                if let Ok(meta) = fs::symlink_metadata(&child) {
                    let idx = self.push(&child, 1, Kind::Symlink, flag::LOOP, &meta);
                    self.link_target(idx, &child, false);
                }
            }
            Failure::Io(Some(path), ioerr) => {
                let err = ScanError::new(&path, ioerr);
                if let Some(dir) = self.ctx.dirs.get(path.as_os_str().as_bytes()) {
                    self.shard.errors.push((ErrorAt::Dir(dir), err));
                    return;
                }
                match fs::symlink_metadata(&path) {
                    Ok(meta) if meta.file_type().is_symlink() => {
                        // `--follow` on a broken link: not an error.
                        let idx = self.push(&path, 1, Kind::Symlink, flag::BROKEN, &meta);
                        self.link_target(idx, &path, false);
                    }
                    Ok(meta) => {
                        let kind = kind_of(&meta.file_type());
                        let idx = self.push(&path, 1, kind, flag::NO_ACCESS, &meta);
                        self.shard.errors.push((ErrorAt::Node(idx), err));
                    }
                    Err(_) => self.shard.errors.push((ErrorAt::Nowhere, err)),
                }
            }
            Failure::Io(None, ioerr) => self.shard.errors.push((
                ErrorAt::Nowhere,
                ScanError {
                    path: PathBuf::new(),
                    message: crate::tree::io_message(ioerr),
                },
            )),
            Failure::Other(message) => self.shard.errors.push((
                ErrorAt::Nowhere,
                ScanError {
                    path: path.map(Path::to_path_buf).unwrap_or_default(),
                    message,
                },
            )),
        }
    }

    /// Adds one record and returns its index in this shard.
    fn push(
        &mut self,
        path: &Path,
        depth: usize,
        kind: Kind,
        mut flags: u16,
        meta: &fs::Metadata,
    ) -> u32 {
        let parent = if depth == 0 {
            NONE
        } else {
            self.parent_id(path)
        };
        let own_dir = if kind == Kind::Dir {
            let id = self.ctx.next_dir.fetch_add(1, Ordering::Relaxed);
            self.ctx.dirs.insert(path.as_os_str().as_bytes(), id);
            id
        } else {
            NONE
        };
        let dev = meta.dev();
        let pseudo = *self.devs.entry(dev).or_insert_with(|| {
            let probe = if kind == Kind::Symlink {
                path.parent().unwrap_or(path)
            } else {
                path
            };
            is_pseudo_fs(probe)
        });
        if pseudo {
            flags |= flag::PSEUDO_FS;
        }
        if kind != Kind::Dir && meta.nlink() > 1 {
            flags |= flag::MULTI_LINK;
        }
        let name = if depth == 0 {
            self.ctx.root_name
        } else {
            path.file_name().map_or(&b""[..], |n| n.as_bytes())
        };
        let name_len = name.len().min(u16::MAX as usize);
        let shard = &mut self.shard;
        shard.nodes.push(Node {
            parent,
            first_child: own_dir,
            next_sibling: NONE,
            end: NONE,
            name_off: shard.names.len() as u32,
            name_len: name_len as u16,
            kind,
            flags,
            size: if kind == Kind::Dir { 0 } else { meta.len() },
            mtime_ns: meta.mtime() * 1_000_000_000 + meta.mtime_nsec(),
            mode: meta.mode(),
            uid: meta.uid(),
            gid: meta.gid(),
            dev,
            ino: meta.ino(),
        });
        shard.names.extend_from_slice(&name[..name_len]);
        (shard.nodes.len() - 1) as u32
    }

    fn parent_id(&mut self, path: &Path) -> u32 {
        let parent = path.parent().map_or(&b""[..], |p| p.as_os_str().as_bytes());
        if self.parent_cache.0 == parent {
            return self.parent_cache.1;
        }
        let id = self.ctx.dirs.get(parent).unwrap_or(NONE);
        self.parent_cache = (parent.to_vec(), id);
        id
    }

    fn link_target(&mut self, idx: u32, path: &Path, check_broken: bool) {
        if let Ok(target) = fs::read_link(path) {
            self.shard
                .links
                .push((idx, target.into_os_string().into_vec()));
        }
        // stat follows the link but opens nothing.
        if check_broken && fs::metadata(path).is_err() {
            self.shard.nodes[idx as usize].flags |= flag::BROKEN;
        }
    }
}

fn kind_of(ft: &fs::FileType) -> Kind {
    if ft.is_dir() {
        Kind::Dir
    } else if ft.is_file() {
        Kind::File
    } else if ft.is_symlink() {
        Kind::Symlink
    } else if ft.is_fifo() {
        Kind::Fifo
    } else if ft.is_socket() {
        Kind::Socket
    } else if ft.is_block_device() || ft.is_char_device() {
        Kind::Device
    } else {
        Kind::File
    }
}

enum Failure<'e> {
    Loop(PathBuf),
    Io(Option<PathBuf>, &'e io::Error),
    Other(String),
}

fn failure<'e>(err: &'e ignore::Error, path: Option<&Path>) -> Failure<'e> {
    use ignore::Error as E;
    match err {
        E::WithPath { path, err } => failure(err, Some(path)),
        E::WithDepth { err, .. } | E::WithLineNumber { err, .. } => failure(err, path),
        E::Loop { child, .. } => Failure::Loop(child.clone()),
        E::Io(e) => Failure::Io(path.map(Path::to_path_buf), e),
        E::Partial(errs) if !errs.is_empty() => failure(&errs[0], path),
        other => Failure::Other(other.to_string()),
    }
}

/// True for `/proc`, `/sys`, and the other kernel file systems whose sizes
/// are not real and whose reads can block.
#[cfg(target_os = "linux")]
fn is_pseudo_fs(path: &Path) -> bool {
    const MAGIC: [u64; 10] = [
        0x9fa0,      // proc
        0x6265_6572, // sysfs
        0x1cd1,      // devpts
        0x0027_e0eb, // cgroup
        0x6367_7270, // cgroup2
        0x6462_6720, // debugfs
        0x7472_6163, // tracefs
        0x7363_6673, // securityfs
        0xcafe_4a11, // bpf
        0x6265_6570, // configfs
    ];
    rustix::fs::statfs(path).is_ok_and(|s| MAGIC.contains(&(s.f_type as u64)))
}

#[cfg(target_os = "macos")]
fn is_pseudo_fs(path: &Path) -> bool {
    rustix::fs::statfs(path).is_ok_and(|s| {
        let name: Vec<u8> = s
            .f_fstypename
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as u8)
            .collect();
        name == b"devfs"
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn is_pseudo_fs(_path: &Path) -> bool {
    false
}

/// Directory path → id, in shards so walk threads seldom wait.
struct DirMap {
    shards: Vec<Mutex<HashMap<Box<[u8]>, u32>>>,
}

impl DirMap {
    const SHARDS: usize = 64;

    fn new() -> DirMap {
        DirMap {
            shards: (0..Self::SHARDS).map(|_| Mutex::default()).collect(),
        }
    }

    fn shard(&self, key: &[u8]) -> &Mutex<HashMap<Box<[u8]>, u32>> {
        // FNV-1a: cheap, and good enough to spread the keys.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &b in key {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
        &self.shards[(h % Self::SHARDS as u64) as usize]
    }

    fn insert(&self, key: &[u8], id: u32) {
        self.shard(key)
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.into(), id);
    }

    fn get(&self, key: &[u8]) -> Option<u32> {
        self.shard(key)
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(key)
            .copied()
    }
}

/// Links the shard records into a tree in depth-first pre-order, with
/// children in name-byte order, so the ids do not depend on the walk order.
fn build(shards: Vec<Shard>, dir_count: u32) -> Tree {
    let total: usize = shards.iter().map(|s| s.nodes.len()).sum();
    let name_total: usize = shards.iter().map(|s| s.names.len()).sum();
    let mut nodes: Vec<Node> = Vec::with_capacity(total);
    let mut names: Vec<u8> = Vec::with_capacity(name_total);
    let mut links: Vec<(u32, Vec<u8>)> = Vec::new();
    let mut errors: Vec<(ErrorAt, ScanError)> = Vec::new();
    for shard in shards {
        let base = nodes.len() as u32;
        let name_base = names.len() as u32;
        nodes.extend(shard.nodes.into_iter().map(|mut n| {
            n.name_off += name_base;
            n
        }));
        names.extend_from_slice(&shard.names);
        links.extend(shard.links.into_iter().map(|(i, t)| (base + i, t)));
        errors.extend(shard.errors.into_iter().map(|(at, e)| {
            let at = match at {
                ErrorAt::Node(i) => ErrorAt::Node(base + i),
                other => other,
            };
            (at, e)
        }));
    }
    let n = nodes.len();

    // Directory id → record index; then parents become record indices.
    let mut dir_node = vec![NONE; dir_count as usize];
    for (i, node) in nodes.iter().enumerate() {
        if node.kind == Kind::Dir && node.first_child != NONE {
            dir_node[node.first_child as usize] = i as u32;
        }
    }
    let mut root = NONE;
    for (i, node) in nodes.iter_mut().enumerate() {
        node.first_child = NONE;
        if node.parent == NONE {
            if node.flags & flag::ROOT != 0 {
                root = i as u32;
            }
        } else {
            node.parent = dir_node[node.parent as usize];
        }
    }

    // Child lists in CSR form, each sorted by name bytes.
    let name_of = |nodes: &[Node], i: u32| -> &[u8] {
        let node = &nodes[i as usize];
        &names[node.name_off as usize..][..node.name_len as usize]
    };
    let mut start = vec![0u32; n + 1];
    for node in &nodes {
        if node.parent != NONE {
            start[node.parent as usize + 1] += 1;
        }
    }
    for i in 0..n {
        start[i + 1] += start[i];
    }
    let mut fill = start.clone();
    let mut kids = vec![0u32; start[n] as usize];
    for (i, node) in nodes.iter().enumerate() {
        if node.parent != NONE {
            let slot = &mut fill[node.parent as usize];
            kids[*slot as usize] = i as u32;
            *slot += 1;
        }
    }
    drop(fill);
    for p in 0..n {
        kids[start[p] as usize..start[p + 1] as usize]
            .sort_unstable_by(|&a, &b| name_of(&nodes, a).cmp(name_of(&nodes, b)));
    }

    // Depth-first pre-order from the root. Records with no path to the root
    // (a parent that vanished during the walk) are dropped.
    let mut order: Vec<u32> = Vec::with_capacity(n);
    if root != NONE {
        let mut stack = vec![root];
        while let Some(i) = stack.pop() {
            order.push(i);
            let list = &kids[start[i as usize] as usize..start[i as usize + 1] as usize];
            stack.extend(list.iter().rev());
        }
    }
    drop(kids);
    drop(start);
    let mut new_of_old = vec![NONE; n];
    for (new, &old) in order.iter().enumerate() {
        new_of_old[old as usize] = new as u32;
    }

    // Position `p` takes the record at `order[p]`. With no dropped records
    // this is a permutation, done in place along its cycles.
    let kept = order.len();
    if kept == n {
        let mut done = vec![false; n];
        for start in 0..n {
            if done[start] {
                continue;
            }
            let first = nodes[start];
            let mut p = start;
            loop {
                done[p] = true;
                let src = order[p] as usize;
                if src == start {
                    nodes[p] = first;
                    break;
                }
                nodes[p] = nodes[src];
                p = src;
            }
        }
    } else {
        nodes = order.iter().map(|&o| nodes[o as usize]).collect();
    }
    drop(order);

    // Parents, child links, and subtree ends in the new ids.
    let mut last_child = vec![NONE; kept];
    for id in 0..kept {
        let node = &mut nodes[id];
        node.next_sibling = NONE;
        node.first_child = NONE;
        node.end = 0;
        if node.parent != NONE {
            node.parent = new_of_old[node.parent as usize];
        }
        let parent = node.parent;
        if parent != NONE {
            let last = last_child[parent as usize];
            if last == NONE {
                nodes[parent as usize].first_child = id as u32;
            } else {
                nodes[last as usize].next_sibling = id as u32;
            }
            last_child[parent as usize] = id as u32;
        }
    }
    drop(last_child);
    for id in (0..kept).rev() {
        let end = nodes[id].end.max(id as u32 + 1);
        nodes[id].end = end;
        let parent = nodes[id].parent;
        if parent != NONE {
            let p = &mut nodes[parent as usize];
            p.end = p.end.max(end);
        }
    }

    // Hard links: the first in id order keeps the size.
    let mut seen: HashSet<(u64, u64)> = HashSet::new();
    for node in nodes.iter_mut() {
        if node.flags & flag::MULTI_LINK != 0 {
            node.flags &= !flag::MULTI_LINK;
            if !seen.insert((node.dev, node.ino)) {
                node.size = 0;
            }
        }
    }

    let link_targets = links
        .into_iter()
        .filter_map(|(old, t)| {
            let new = new_of_old[old as usize];
            (new != NONE).then_some((new, t))
        })
        .collect();

    let mut placed: Vec<(NodeId, ScanError)> = Vec::new();
    let mut loose: Vec<ScanError> = Vec::new();
    for (at, err) in errors {
        let old = match at {
            ErrorAt::Dir(d) => dir_node.get(d as usize).copied().unwrap_or(NONE),
            ErrorAt::Node(i) => i,
            ErrorAt::Nowhere => NONE,
        };
        let new = if old == NONE {
            NONE
        } else {
            new_of_old[old as usize]
        };
        if new == NONE {
            loose.push(err);
        } else {
            nodes[new as usize].flags |= flag::NO_ACCESS;
            placed.push((new, err));
        }
    }
    placed.sort_by_key(|(id, _)| *id);
    let mut all_errors: Vec<ScanError> = placed.into_iter().map(|(_, e)| e).collect();
    loose.sort_by(|a, b| a.path.cmp(&b.path));
    all_errors.extend(loose);

    debug_assert!(nodes.is_empty() || nodes[ROOT as usize].flags & flag::ROOT != 0);
    Tree {
        nodes,
        names,
        link_targets,
        root_path: PathBuf::new(),
        root_label: String::new(),
        scan_time: 0,
        errors: all_errors,
    }
}
