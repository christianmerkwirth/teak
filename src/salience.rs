//! Salience: how much a row tells the user, from 0 to 1 (PRD §9.2).
//!
//! Only stat data, names, chaff tags, and git status go in: content
//! detection runs after selection.

use std::collections::HashMap;

use crate::aggregate::Aggs;
use crate::chaff::{Action, ChaffClass, ChaffTag};
use crate::detect::ext::ext_of;
use crate::git::{Code, GitStatus};
use crate::tree::{Kind, NodeId, ROOT, Tree, flag};

/// Term weights. P4 makes them config keys (FR-67).
#[derive(Clone, Copy, Debug)]
pub struct Weights {
    pub size: f32,
    pub recency: f32,
    pub git: f32,
    pub rarity: f32,
    pub role: f32,
    pub anomaly: f32,
}

impl Default for Weights {
    fn default() -> Weights {
        Weights {
            size: 0.30,
            recency: 0.20,
            git: 0.15,
            rarity: 0.10,
            role: 0.10,
            anomaly: 0.15,
        }
    }
}

/// The score factor for dimmed and gitignored entries.
const CHAFF_FACTOR: f32 = 0.2;

pub struct ScoreInputs<'a> {
    pub tree: &'a Tree,
    /// Totals without hidden entries.
    pub aggs: &'a Aggs,
    pub tags: &'a [ChaffTag],
    pub git: Option<&'a GitStatus>,
}

/// The term values of one node, each in `[0, 1]`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Terms {
    pub size: f32,
    pub recency: f32,
    pub git: f32,
    pub rarity: f32,
    pub role: f32,
    pub anomaly: f32,
    pub factor: f32,
}

impl Terms {
    pub fn score(&self, w: &Weights) -> f32 {
        let sum = w.size * self.size
            + w.recency * self.recency
            + w.git * self.git
            + w.rarity * self.rarity
            + w.role * self.role
            + w.anomaly * self.anomaly;
        sum.clamp(0.0, 1.0) * self.factor
    }
}

/// One score per node.
pub struct Scores {
    pub score: Vec<f32>,
}

/// Scores every node.
pub fn score(inp: &ScoreInputs, w: &Weights) -> Scores {
    let ctx = Context::new(inp);
    let score = (0..inp.tree.len() as NodeId)
        .map(|id| ctx.terms(inp, id).score(w))
        .collect();
    Scores { score }
}

/// Tree-wide facts that the terms compare against.
struct Context {
    total: u64,
    scan_time: i64,
    /// Per node: the rarity term.
    rarity: Vec<f32>,
}

/// Ages at or above this get no recency.
const OLD_SECS: f64 = 10.0 * 365.0 * 86_400.0;

impl Context {
    fn new(inp: &ScoreInputs) -> Context {
        Context {
            total: inp.aggs.get(inp.tree, ROOT).size,
            scan_time: inp.tree.scan_time,
            rarity: rarity(inp.tree),
        }
    }

    fn terms(&self, inp: &ScoreInputs, id: NodeId) -> Terms {
        let tree = inp.tree;
        let node = tree.node(id);
        let agg = inp.aggs.get(tree, id);
        let size = if self.total == 0 {
            0.0
        } else {
            let share = agg.size as f64 / self.total as f64;
            ((1.0 + 999.0 * share).ln() / 1000f64.ln()) as f32
        };
        let newest = if agg.newest == i64::MIN {
            node.mtime_ns
        } else {
            agg.newest
        };
        // Log scale of the age at scan time: 1 for now, 0 for 10 years or
        // more. Seconds apart (one checkout) give almost the same value.
        let age_s = (self.scan_time as f64 - newest as f64 / 1e9).max(0.0);
        let recency = (1.0 - (1.0 + age_s).ln() / (1.0 + OLD_SECS).ln()).clamp(0.0, 1.0) as f32;
        let rarity = self.rarity[id as usize];
        let tag = &inp.tags[id as usize];
        let factor = if tag.action == Action::Dim || tag.class == Some(ChaffClass::Ignored) {
            CHAFF_FACTOR
        } else {
            1.0
        };
        Terms {
            size,
            recency,
            git: git_term(inp, id),
            rarity,
            role: role(tree.name(id), node.kind),
            anomaly: anomaly(tree, id, self.total),
            factor,
        }
    }
}

/// 1 for a modified, added, or untracked file. A directory scores by the
/// share of its files that changed: 0.5 for one, up to 1 for all.
fn git_term(inp: &ScoreInputs, id: NodeId) -> f32 {
    let Some(git) = inp.git else {
        return 0.0;
    };
    let tree = inp.tree;
    if !tree.node(id).is_dir() {
        return match git.of(id) {
            Some(Code::Ignored) | None => 0.0,
            Some(_) => 1.0,
        };
    }
    let changed = git.changed_below(tree, id);
    if changed == 0 {
        return 0.0;
    }
    let files = inp.aggs.get(tree, id).files.max(changed);
    0.5 + 0.5 * (changed as f32 / files as f32)
}

/// Rarity of each file among its sibling files: `1 - share of its
/// extension`, when one extension holds at least half of 3 or more files
/// (one `.py` among many `.png`). In a mixed directory, no file is rare.
fn rarity(tree: &Tree) -> Vec<f32> {
    let mut out = vec![0.0; tree.len()];
    let mut counts: HashMap<&[u8], u32> = HashMap::new();
    for (dir, node) in tree.nodes.iter().enumerate() {
        if !node.is_dir() {
            continue;
        }
        counts.clear();
        let mut files = 0u32;
        for c in tree.children(dir as NodeId) {
            if !tree.node(c).is_dir() {
                files += 1;
                *counts
                    .entry(ext_of(tree.name(c)).unwrap_or(b""))
                    .or_default() += 1;
            }
        }
        let top = counts.values().copied().max().unwrap_or(0);
        if files < 3 || top * 2 < files {
            continue;
        }
        for c in tree.children(dir as NodeId) {
            if !tree.node(c).is_dir() {
                let n = counts[ext_of(tree.name(c)).unwrap_or(b"")];
                out[c as usize] = 1.0 - n as f32 / files as f32;
            }
        }
    }
    out
}

/// Entry points and key files.
fn role(name: &[u8], kind: Kind) -> f32 {
    if kind == Kind::Dir {
        return 0.0;
    }
    const EXACT: &[&[u8]] = &[
        b"Makefile",
        b"Dockerfile",
        b"Cargo.toml",
        b"package.json",
        b"pyproject.toml",
        b"setup.py",
        b"go.mod",
        b"pom.xml",
        b"build.gradle",
        b"CMakeLists.txt",
        b"Gemfile",
        b"justfile",
    ];
    if EXACT.contains(&name) || name.starts_with(b"README") {
        return 1.0;
    }
    let stem = name.split(|&b| b == b'.').next().unwrap_or(name);
    if matches!(stem, b"main" | b"index" | b"app" | b"lib" | b"mod") {
        return 0.6;
    }
    0.0
}

/// No access, odd permissions, broken links, and huge single files.
fn anomaly(tree: &Tree, id: NodeId, total: u64) -> f32 {
    let node = tree.node(id);
    if node.has(flag::NO_ACCESS) {
        return 1.0;
    }
    if node.kind == Kind::File && total > 1 << 20 && node.size * 2 > total {
        return 1.0;
    }
    let odd_mode = node.kind != Kind::Symlink && node.mode & 0o6002 != 0;
    if odd_mode || node.has(flag::BROKEN) || node.has(flag::LOOP) {
        return 0.5;
    }
    0.0
}
