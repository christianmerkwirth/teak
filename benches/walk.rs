//! Phase-0 walk benchmark (FR-71, FR-72, FR-73 baselines).
//!
//! A parallel `ignore` walk with `standard_filters(false)` and an lstat per
//! entry, on generated trees of 10k, 100k, and 1M files. Two variants:
//!
//! - `stat`: walk and lstat only, no storage.
//! - `arena`: walk, then build a compact arena like design.md §Data Models
//!   (`Node` + one name pool, children in name order).
//!
//! Peak RSS mode (criterion does not measure memory):
//!
//! ```text
//! TEAK_WALK_RSS=1m cargo bench --bench walk
//! ```
//!
//! Trees go to `target/bench-data/files-<N>-seed-42`. A child process makes
//! them, so the generator's memory does not count in this process.

#[path = "../examples/gen_tree.rs"]
mod gen_tree;

use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use criterion::{BenchmarkId, Criterion, criterion_group};
use ignore::{WalkBuilder, WalkState};

const SEED: u64 = 42;
const CASES: &[(&str, u64)] = &[("10k", 10_000), ("100k", 100_000), ("1m", 1_000_000)];

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("target/bench-data")
}

/// Returns the tree for `files`, and makes it first if needed.
fn ensure_tree(files: u64) -> PathBuf {
    let dir = data_dir().join(format!("files-{files}-seed-{SEED}"));
    let done = dir.with_extension("ok");
    if done.exists() {
        return dir;
    }
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove partial tree");
    }
    eprintln!("generating {} ...", dir.display());
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .env("TEAK_WALK_GEN", files.to_string())
        .env("TEAK_WALK_GEN_OUT", &dir)
        .status()
        .expect("spawn generator");
    assert!(status.success(), "generator failed");
    std::fs::write(&done, b"").unwrap();
    dir
}

fn threads() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get())
}

fn walker(root: &Path) -> ignore::WalkParallel {
    WalkBuilder::new(root)
        .standard_filters(false)
        .follow_links(false)
        .threads(threads())
        .build_parallel()
}

/// Walk and lstat every entry; store nothing. Returns the entry count.
fn walk_stat(root: &Path) -> u64 {
    let count = AtomicU64::new(0);
    walker(root).run(|| {
        Box::new(|entry| {
            if let Ok(e) = entry
                && e.metadata().is_ok()
            {
                count.fetch_add(1, Ordering::Relaxed);
            }
            WalkState::Continue
        })
    });
    count.into_inner()
}

/// One walk record before the arena exists.
struct Rec {
    path: Box<[u8]>,
    kind: u8,
    size: u64,
    mtime_ns: i64,
    mode: u32,
    uid: u32,
    gid: u32,
    dev: u64,
    ino: u64,
}

/// The arena node from design.md §Data Models.
#[allow(dead_code)]
struct Node {
    parent: u32,
    first_child: u32,
    next_sibling: u32,
    name_off: u32,
    name_len: u16,
    kind: u8,
    flags: u16,
    size: u64,
    mtime_ns: i64,
    mode: u32,
    uid: u32,
    gid: u32,
    dev: u64,
    ino: u64,
}

struct Tree {
    nodes: Vec<Node>,
    names: Vec<u8>,
}

const NONE: u32 = u32::MAX;

/// Collects records per thread, then links each entry to its parent through a
/// map of directory paths, and sorts each child list by name bytes.
fn walk_arena(root: &Path) -> Tree {
    let shards: Mutex<Vec<Vec<Rec>>> = Mutex::new(Vec::new());
    struct Shard<'a> {
        recs: Vec<Rec>,
        out: &'a Mutex<Vec<Vec<Rec>>>,
    }
    impl Drop for Shard<'_> {
        fn drop(&mut self) {
            self.out
                .lock()
                .unwrap()
                .push(std::mem::take(&mut self.recs));
        }
    }
    walker(root).run(|| {
        let mut shard = Shard {
            recs: Vec::new(),
            out: &shards,
        };
        Box::new(move |entry| {
            let Ok(e) = entry else {
                return WalkState::Continue;
            };
            let Ok(m) = e.metadata() else {
                return WalkState::Continue;
            };
            let ft = m.file_type();
            let kind = if ft.is_dir() {
                0
            } else if ft.is_file() {
                1
            } else if ft.is_symlink() {
                2
            } else {
                3
            };
            let rel = e.path().strip_prefix(root).expect("entry below root");
            shard.recs.push(Rec {
                path: rel.as_os_str().as_bytes().into(),
                kind,
                size: m.len(),
                mtime_ns: m.mtime() * 1_000_000_000 + m.mtime_nsec(),
                mode: m.mode(),
                uid: m.uid(),
                gid: m.gid(),
                dev: m.dev(),
                ino: m.ino(),
            });
            WalkState::Continue
        })
    });
    // Read the shards in place; a flat copy would double the record memory.
    let shards = shards.into_inner().unwrap();
    let recs = || shards.iter().flatten();
    let total = shards.iter().map(Vec::len).sum();
    let root = root.as_os_str().as_bytes();

    // Node ids follow walk order; only the child lists are sorted. Paths are
    // relative to the root, and the root itself has the empty path.
    let dirs: HashMap<&[u8], u32> = recs()
        .enumerate()
        .filter(|(_, r)| r.kind == 0)
        .map(|(i, r)| (&r.path[..], i as u32))
        .collect();
    let mut tree = Tree {
        nodes: Vec::with_capacity(total),
        names: Vec::new(),
    };
    for rec in recs() {
        let (parent, name) = if rec.path.is_empty() {
            (NONE, root)
        } else {
            match rec.path.iter().rposition(|&c| c == b'/') {
                Some(slash) => (dirs[&rec.path[..slash]], &rec.path[slash + 1..]),
                None => (dirs[&b""[..]], &rec.path[..]),
            }
        };
        tree.nodes.push(Node {
            parent,
            first_child: NONE,
            next_sibling: NONE,
            name_off: tree.names.len() as u32,
            name_len: name.len() as u16,
            kind: rec.kind,
            flags: 0,
            size: rec.size,
            mtime_ns: rec.mtime_ns,
            mode: rec.mode,
            uid: rec.uid,
            gid: rec.gid,
            dev: rec.dev,
            ino: rec.ino,
        });
        tree.names.extend_from_slice(name);
    }
    drop(dirs);
    drop(shards);

    // Child lists in CSR form: count, prefix sums, fill, sort each slice.
    let n = tree.nodes.len();
    let mut start = vec![0u32; n + 1];
    for node in &tree.nodes {
        if node.parent != NONE {
            start[node.parent as usize + 1] += 1;
        }
    }
    for i in 0..n {
        start[i + 1] += start[i];
    }
    let mut fill = start.clone();
    let mut kids = vec![0u32; start[n] as usize];
    for (id, node) in tree.nodes.iter().enumerate() {
        if node.parent != NONE {
            let slot = &mut fill[node.parent as usize];
            kids[*slot as usize] = id as u32;
            *slot += 1;
        }
    }
    fn name(t: &Tree, id: u32) -> &[u8] {
        let node = &t.nodes[id as usize];
        &t.names[node.name_off as usize..][..node.name_len as usize]
    }
    for p in 0..n {
        let list = &mut kids[start[p] as usize..start[p + 1] as usize];
        list.sort_unstable_by(|&a, &b| name(&tree, a).cmp(name(&tree, b)));
        if let Some(&first) = list.first() {
            tree.nodes[p].first_child = first;
        }
        for w in list.windows(2) {
            tree.nodes[w[0] as usize].next_sibling = w[1];
        }
    }
    tree
}

fn peak_rss_mb() -> f64 {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: getrusage fills the struct we pass.
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    let raw = usage.ru_maxrss as f64;
    // Linux reports KiB, macOS reports bytes.
    if cfg!(target_os = "macos") {
        raw / 1048576.0
    } else {
        raw / 1024.0
    }
}

fn bench_walk(c: &mut Criterion) {
    let mut group = c.benchmark_group("walk");
    group.sample_size(10);
    for &(name, files) in CASES {
        let root = ensure_tree(files);
        group.measurement_time(Duration::from_secs(if files >= 1_000_000 {
            40
        } else {
            10
        }));
        group.bench_with_input(BenchmarkId::new("stat", name), &root, |b, root| {
            b.iter(|| walk_stat(root))
        });
        group.bench_with_input(BenchmarkId::new("arena", name), &root, |b, root| {
            b.iter(|| walk_arena(root).nodes.len())
        });
    }
    group.finish();
}

criterion_group!(benches, bench_walk);

fn main() {
    if let Ok(files) = std::env::var("TEAK_WALK_GEN") {
        let opts = gen_tree::Opts {
            files: files.parse().expect("TEAK_WALK_GEN"),
            seed: SEED,
            out: std::env::var_os("TEAK_WALK_GEN_OUT")
                .expect("TEAK_WALK_GEN_OUT")
                .into(),
            ..gen_tree::Opts::default()
        };
        let stats = gen_tree::generate(&opts).expect("generate tree");
        eprintln!(
            "{} files, {} dirs, {} bytes",
            stats.files, stats.dirs, stats.bytes
        );
        return;
    }
    if let Ok(case) = std::env::var("TEAK_WALK_RSS") {
        let &(_, files) = CASES
            .iter()
            .find(|(n, _)| *n == case)
            .expect("TEAK_WALK_RSS: 10k, 100k, or 1m");
        let root = ensure_tree(files);
        let entries = walk_stat(&root); // also warms the OS cache
        let base = peak_rss_mb();
        let start = Instant::now();
        let tree = walk_arena(&root);
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(tree.nodes.len() as u64, entries, "arena lost entries");
        assert_eq!(
            tree.nodes.iter().filter(|n| n.parent == NONE).count(),
            1,
            "one root"
        );
        println!(
            "walk_arena {case}: {} nodes, {} name bytes, {ms:.0} ms, peak RSS {:.1} MB (before walk {base:.1} MB)",
            tree.nodes.len(),
            tree.names.len(),
            peak_rss_mb()
        );
        return;
    }
    benches();
    Criterion::default().configure_from_args().final_summary();
}
