//! The greedy budget planner (PRD §9.3, design.md §Budget accounting).
//!
//! The root starts open. Each step either opens a closed directory (it
//! shows its best child, plus an overflow row and a chaff line when needed)
//! or reveals the next best child of an open directory. The queue takes the
//! step with the best gain ÷ cost that still fits. Revealing the last child
//! costs nothing: it takes the place of the overflow row.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap, HashMap};

use crate::aggregate::Aggs;
use crate::chaff::{Action, ChaffTag, Source};
use crate::detect::{LabelId, label};
use crate::series::{self, Uniform, Unit};
use crate::tree::{NodeId, ROOT, Tree, flag};
use crate::view::{ChaffItem, ClassCounts, Row, RowKind, View};

pub struct SelectInputs<'a> {
    pub tree: &'a Tree,
    /// Totals without hidden entries.
    pub aggs: &'a Aggs,
    pub tags: &'a [ChaffTag],
    pub scores: &'a [f32],
    /// Name-based labels, for the `type` sort.
    pub labels: &'a [LabelId],
}

/// `--sort`: the display order inside a directory. It never changes which
/// rows show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    /// Directories first, then by name.
    Default,
    Salience,
    Name,
    Size,
    Age,
    Type,
}

pub struct SelectOpts {
    /// `-L`: no row deeper than this.
    pub depth: Option<usize>,
    pub series_min: usize,
    pub sort: SortKey,
}

/// Builds the view for a budget of at least 3 lines.
///
/// Post: `rows.len() <= budget`; every node is in exactly one row or in a
/// footer count; ties break by name.
pub fn plan(inp: &SelectInputs, budget: usize, opts: &SelectOpts) -> View {
    debug_assert!(budget >= 3);
    let mut p = Planner {
        inp,
        opts,
        states: HashMap::new(),
        heap: BinaryHeap::new(),
        left: 0,
    };
    let mut rows = vec![Row {
        depth: 0,
        last: false,
        kind: RowKind::Header,
    }];
    let rows_budget = budget.saturating_sub(2);
    if !inp.tree.node(ROOT).is_dir() {
        rows.push(Row {
            depth: 1,
            last: true,
            kind: RowKind::Entry {
                node: ROOT,
                joined: Vec::new(),
                open: false,
            },
        });
    } else if p.open_root(rows_budget) {
        p.run();
        p.emit(ROOT, 1, &mut rows);
    } else {
        rows.push(Row {
            depth: 1,
            last: true,
            kind: RowKind::Summary {
                node: ROOT,
                joined: Vec::new(),
                uniform: None,
            },
        });
    }
    let footer = footer(inp.tree, inp.tags, &rows, budget);
    rows.push(footer);
    debug_assert!(rows.len() <= budget, "{} rows for {budget}", rows.len());
    View { rows, budget }
}

/// A row candidate in a directory.
#[derive(Clone)]
struct Cand {
    unit: Unit,
    score: f32,
    /// For a directory: the joined chain below it (FR-15).
    joined: Vec<NodeId>,
}

impl Cand {
    /// The directory that opens for this candidate, if it is one.
    fn tail(&self, tree: &Tree) -> Option<NodeId> {
        match &self.unit {
            Unit::Node(id) if tree.node(*id).is_dir() => {
                Some(self.joined.last().copied().unwrap_or(*id))
            }
            _ => None,
        }
    }
}

struct DirState {
    /// Best first; ties by name.
    cands: Vec<Cand>,
    /// A uniform directory shows as one row, also when it would open.
    uniform: Option<Uniform>,
    chaff: Vec<NodeId>,
    shown: usize,
    open: bool,
    /// The depth of this directory's row (0 for the root).
    depth: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Reveal(NodeId),
    Open(NodeId),
}

struct Item {
    ratio: f32,
    key: NodeId,
    step: Step,
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Item {}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher ratio first; then the smaller id, which is name order.
        self.ratio
            .total_cmp(&other.ratio)
            .then_with(|| other.key.cmp(&self.key))
    }
}

/// The gain of a directory that holds only chaff: the chaff line shows a
/// little, but less than a normal row.
const CHAFF_ONLY_GAIN: f32 = 0.01;

/// The part of a directory's own score that opening it replaces.
const DIR_SHARE: f32 = 0.5;

struct Planner<'a> {
    inp: &'a SelectInputs<'a>,
    opts: &'a SelectOpts,
    states: HashMap<NodeId, DirState>,
    heap: BinaryHeap<Item>,
    /// Row lines not yet spent.
    left: usize,
}

fn ratio(gain: f32, cost: usize) -> f32 {
    if cost == 0 {
        f32::INFINITY
    } else {
        gain / cost as f32
    }
}

impl Planner<'_> {
    /// The children of `dir` that get rows, and those that fold into its
    /// chaff line. Hidden children are in neither.
    fn visible(&self, dir: NodeId) -> (Vec<NodeId>, Vec<NodeId>) {
        let mut rows = Vec::new();
        let mut chaff = Vec::new();
        for c in self.inp.tree.children(dir) {
            match self.inp.tags[c as usize].action {
                Action::Hide => {}
                Action::Fold => chaff.push(c),
                Action::Show | Action::Dim => rows.push(c),
            }
        }
        (rows, chaff)
    }

    /// The chain of single-child directories below `id`.
    fn chain(&self, id: NodeId) -> Vec<NodeId> {
        let tree = self.inp.tree;
        let mut out = Vec::new();
        let mut cur = id;
        while !tree.node(cur).has(flag::NO_ACCESS | flag::MOUNT) {
            let (rows, chaff) = self.visible(cur);
            if chaff.is_empty() && rows.len() == 1 && tree.node(rows[0]).is_dir() {
                cur = rows[0];
                out.push(cur);
            } else {
                break;
            }
        }
        out
    }

    fn unit_score(&self, unit: &Unit) -> f32 {
        match unit {
            Unit::Node(id) => self.inp.scores[*id as usize],
            Unit::Series(s) => s
                .members
                .iter()
                .map(|&m| self.inp.scores[m as usize])
                .fold(0.0, f32::max),
        }
    }

    /// Computes the state of `dir` once.
    fn ensure(&mut self, dir: NodeId, depth: usize) {
        if self.states.contains_key(&dir) {
            return;
        }
        let (rows, chaff) = self.visible(dir);
        let units = series::group(self.inp.tree, dir, &rows, self.opts.series_min);
        let uniform = if dir == ROOT {
            None
        } else {
            series::uniform(self.inp.tree, &units, self.inp.labels)
        };
        let mut cands: Vec<Cand> = units
            .into_iter()
            .map(|unit| {
                let joined = match &unit {
                    Unit::Node(id) if self.inp.tree.node(*id).is_dir() => self.chain(*id),
                    _ => Vec::new(),
                };
                Cand {
                    score: self.unit_score(&unit),
                    unit,
                    joined,
                }
            })
            .collect();
        cands.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.unit.first().cmp(&b.unit.first()))
        });
        self.states.insert(
            dir,
            DirState {
                cands,
                uniform,
                chaff,
                shown: 0,
                open: false,
                depth,
            },
        );
    }

    /// Opens the root if its first lines fit.
    fn open_root(&mut self, rows_budget: usize) -> bool {
        if self.opts.depth == Some(0) {
            return false;
        }
        self.ensure(ROOT, 0);
        let st = self.states.get_mut(&ROOT).unwrap();
        let initial = usize::from(!st.cands.is_empty()) + usize::from(!st.chaff.is_empty());
        if initial > rows_budget {
            return false;
        }
        st.open = true;
        self.left = rows_budget - initial;
        self.push_reveal(ROOT);
        true
    }

    /// The cost of revealing the next child of an open directory: one line,
    /// or none for the last child (it replaces the overflow row).
    fn reveal_cost(st: &DirState) -> usize {
        usize::from(st.shown + 1 < st.cands.len())
    }

    /// The cost and gain of opening `dir` (its first child, an overflow row
    /// if more children wait, and the chaff line). The overflow row is worth
    /// its best child. Half the score of the directory row comes off, since
    /// that row already sums up its children: a directory with one small
    /// file gains little by opening.
    fn open_cost(st: &DirState, dir_score: f32) -> (usize, f32) {
        let k = st.cands.len();
        let cost = usize::from(k >= 1) + usize::from(k > 1) + usize::from(!st.chaff.is_empty());
        let shown = match st.cands.as_slice() {
            [] => CHAFF_ONLY_GAIN,
            [first] => first.score,
            [first, second, ..] => first.score + second.score,
        };
        (cost, (shown - DIR_SHARE * dir_score).max(0.0))
    }

    fn push_reveal(&mut self, dir: NodeId) {
        let st = &self.states[&dir];
        if let Some(next) = st.cands.get(st.shown) {
            self.heap.push(Item {
                ratio: ratio(next.score, Self::reveal_cost(st)),
                key: next.unit.first(),
                step: Step::Reveal(dir),
            });
        }
    }

    /// Queues the opening of `cand`, a child row at `row_depth`.
    fn push_open(&mut self, cand: &Cand, row_depth: usize) {
        let Some(tail) = cand.tail(self.inp.tree) else {
            return;
        };
        if self.opts.depth.is_some_and(|limit| row_depth + 1 > limit) {
            return;
        }
        self.ensure(tail, row_depth);
        if self.states[&tail].uniform.is_some() {
            return;
        }
        let (cost, gain) = Self::open_cost(&self.states[&tail], cand.score);
        if cost == 0 {
            return;
        }
        self.heap.push(Item {
            ratio: ratio(gain, cost),
            key: tail,
            step: Step::Open(tail),
        });
    }

    fn run(&mut self) {
        while let Some(item) = self.heap.pop() {
            match item.step {
                Step::Reveal(dir) => {
                    let st = &self.states[&dir];
                    let cost = Self::reveal_cost(st);
                    if cost > self.left {
                        continue;
                    }
                    self.left -= cost;
                    let st = self.states.get_mut(&dir).unwrap();
                    let cand = st.cands[st.shown].clone();
                    st.shown += 1;
                    let depth = st.depth;
                    self.push_reveal(dir);
                    self.push_open(&cand, depth + 1);
                }
                Step::Open(dir) => {
                    let (cost, _) = Self::open_cost(&self.states[&dir], 0.0);
                    if cost > self.left {
                        continue;
                    }
                    self.left -= cost;
                    let st = self.states.get_mut(&dir).unwrap();
                    st.open = true;
                    st.shown = st.cands.len().min(1);
                    let first = st.cands.first().cloned();
                    let depth = st.depth;
                    self.push_reveal(dir);
                    if let Some(first) = first {
                        self.push_open(&first, depth + 1);
                    }
                }
            }
        }
    }

    fn is_open(&self, dir: NodeId) -> bool {
        self.states.get(&dir).is_some_and(|s| s.open)
    }

    /// The display order of the shown children (`--sort`).
    fn display_cmp(&self, a: &Cand, b: &Cand) -> Ordering {
        let tree = self.inp.tree;
        let (ia, ib) = (a.unit.first(), b.unit.first());
        let by_name = || tree.name(ia).cmp(tree.name(ib));
        let dirs_first = || {
            let (da, db) = (tree.node(ia).is_dir(), tree.node(ib).is_dir());
            db.cmp(&da)
        };
        let size = |c: &Cand| self.unit_size(c);
        let newest = |c: &Cand| self.unit_newest(c);
        match self.opts.sort {
            SortKey::Default => dirs_first().then_with(by_name),
            SortKey::Name => by_name(),
            SortKey::Salience => b.score.total_cmp(&a.score).then_with(by_name),
            SortKey::Size => size(b).cmp(&size(a)).then_with(by_name),
            SortKey::Age => newest(b).cmp(&newest(a)).then_with(by_name),
            SortKey::Type => dirs_first()
                .then_with(|| {
                    let la = label(self.inp.labels[ia as usize]).name;
                    let lb = label(self.inp.labels[ib as usize]).name;
                    la.cmp(lb)
                })
                .then_with(by_name),
        }
    }

    fn unit_size(&self, c: &Cand) -> u64 {
        let tree = self.inp.tree;
        match &c.unit {
            Unit::Node(id) => self.inp.aggs.get(tree, *id).size,
            Unit::Series(s) => s.members.iter().map(|&m| tree.node(m).size).sum(),
        }
    }

    fn unit_newest(&self, c: &Cand) -> i64 {
        let tree = self.inp.tree;
        match &c.unit {
            Unit::Node(id) => {
                let a = self.inp.aggs.get(tree, *id);
                if a.newest == i64::MIN {
                    tree.node(*id).mtime_ns
                } else {
                    a.newest
                }
            }
            Unit::Series(s) => s
                .members
                .iter()
                .map(|&m| tree.node(m).mtime_ns)
                .max()
                .unwrap_or(i64::MIN),
        }
    }

    /// The chaff row of `dir`, if it has chaff.
    fn chaff_row(&self, dir: NodeId, depth: usize) -> Option<Row> {
        let st = &self.states[&dir];
        if st.chaff.is_empty() {
            return None;
        }
        let tree = self.inp.tree;
        let dir_total = self.inp.aggs.get(tree, dir).size;
        let mut items: Vec<ChaffItem> = st
            .chaff
            .iter()
            .map(|&c| {
                let a = self.inp.aggs.get(tree, c);
                ChaffItem {
                    node: c,
                    size: a.size,
                    files: a.files,
                    large: a.size >= 1 << 20 || (dir_total > 0 && a.size * 10 >= dir_total),
                }
            })
            .collect();
        // Large items by size (high first), then small items by name.
        items.sort_by(|a, b| {
            b.large
                .cmp(&a.large)
                .then_with(|| {
                    if a.large {
                        b.size.cmp(&a.size)
                    } else {
                        Ordering::Equal
                    }
                })
                .then_with(|| a.node.cmp(&b.node))
        });
        let chaff_size: u64 = items.iter().map(|i| i.size).sum();
        let first = chaff_size > dir_total.saturating_sub(chaff_size);
        Some(Row {
            depth: depth as u16,
            last: false,
            kind: RowKind::Chaff {
                parent: dir,
                items,
                first,
            },
        })
    }

    /// Emits the child rows of the open directory `dir` at `depth`.
    fn emit(&self, dir: NodeId, depth: usize, rows: &mut Vec<Row>) {
        let st = &self.states[&dir];
        let tree = self.inp.tree;
        let mut shown: Vec<&Cand> = st.cands[..st.shown].iter().collect();
        shown.sort_by(|a, b| self.display_cmp(a, b));
        let mut chaff = self.chaff_row(dir, depth);
        let mut last_index = None;
        if let Some(row) = chaff.take_if(|r| matches!(r.kind, RowKind::Chaff { first: true, .. })) {
            last_index = Some(rows.len());
            rows.push(row);
        }
        for cand in shown {
            last_index = Some(rows.len());
            match &cand.unit {
                Unit::Node(id) => {
                    let id = *id;
                    if tree.node(id).is_dir() {
                        let tail = cand.tail(tree).unwrap_or(id);
                        let open = self.is_open(tail);
                        let kind = if open {
                            RowKind::Entry {
                                node: id,
                                joined: cand.joined.clone(),
                                open: true,
                            }
                        } else {
                            RowKind::Summary {
                                node: id,
                                joined: cand.joined.clone(),
                                uniform: self.states.get(&tail).and_then(|s| s.uniform),
                            }
                        };
                        rows.push(Row {
                            depth: depth as u16,
                            last: false,
                            kind,
                        });
                        if open {
                            self.emit(tail, depth + 1, rows);
                        }
                    } else {
                        rows.push(Row {
                            depth: depth as u16,
                            last: false,
                            kind: RowKind::Entry {
                                node: id,
                                joined: Vec::new(),
                                open: false,
                            },
                        });
                    }
                }
                Unit::Series(s) => rows.push(Row {
                    depth: depth as u16,
                    last: false,
                    kind: RowKind::Series {
                        parent: dir,
                        series: s.clone(),
                    },
                }),
            }
        }
        if st.shown < st.cands.len() {
            last_index = Some(rows.len());
            rows.push(self.overflow(dir, depth, &st.cands[st.shown..]));
        }
        if let Some(row) = chaff {
            last_index = Some(rows.len());
            rows.push(row);
        }
        if let Some(i) = last_index {
            rows[i].last = true;
        }
    }

    fn overflow(&self, dir: NodeId, depth: usize, rest: &[Cand]) -> Row {
        let tree = self.inp.tree;
        let (mut dirs, mut files, mut size, mut total_files) = (0, 0, 0, 0);
        let mut newest = i64::MIN;
        let mut nodes = Vec::new();
        for cand in rest {
            match &cand.unit {
                Unit::Node(id) => {
                    let a = self.inp.aggs.get(tree, *id);
                    if tree.node(*id).is_dir() {
                        dirs += 1;
                    } else {
                        files += 1;
                    }
                    size += a.size;
                    total_files += a.files;
                    newest = newest.max(a.newest);
                    nodes.push(*id);
                }
                Unit::Series(s) => {
                    for &m in &s.members {
                        let n = tree.node(m);
                        files += 1;
                        size += n.size;
                        total_files += 1;
                        newest = newest.max(n.mtime_ns);
                        nodes.push(m);
                    }
                }
            }
        }
        Row {
            depth: depth as u16,
            last: false,
            kind: RowKind::Overflow {
                parent: dir,
                dirs,
                files,
                size,
                total_files,
                newest,
                nodes,
            },
        }
    }
}

/// The footer for `rows` (the header and the rows before the footer): the
/// lines used, the files, and the folded and hidden counts by class.
pub fn footer(tree: &Tree, tags: &[ChaffTag], rows: &[Row], budget: usize) -> Row {
    let mut folded: BTreeMap<&'static str, u64> = BTreeMap::new();
    for row in rows {
        if let RowKind::Chaff { items, .. } = &row.kind {
            for item in items {
                let tag = &tags[item.node as usize];
                *folded.entry(tag.group_name()).or_default() += item.files;
            }
        }
    }
    let mut hidden: BTreeMap<&'static str, u64> = BTreeMap::new();
    for (id, tag) in tags.iter().enumerate() {
        if tag.action == Action::Hide && tag.source != Source::Inherited {
            *hidden.entry(tag.group_name()).or_default() += tree.file_count(id as NodeId);
        }
    }
    Row {
        depth: 0,
        last: false,
        kind: RowKind::Footer {
            used: rows.len() + 1,
            budget,
            total_files: tree.file_count(ROOT),
            folded: sorted_counts(folded),
            hidden: sorted_counts(hidden),
        },
    }
}

/// The `--mismatches` view: the header, one row per mismatched file (in the
/// given order) while the budget lasts, then `… N more`, then the footer.
/// With no mismatches, one `no mismatches` row.
pub fn mismatch_view(tree: &Tree, tags: &[ChaffTag], files: &[NodeId], budget: usize) -> View {
    let row = |kind| Row {
        depth: 0,
        last: false,
        kind,
    };
    let mut rows = vec![row(RowKind::Header)];
    let room = budget.saturating_sub(2);
    if files.is_empty() {
        rows.push(row(RowKind::Note("no mismatches")));
    } else if files.len() <= room {
        rows.extend(files.iter().map(|&node| row(RowKind::Mismatch { node })));
    } else {
        let shown = room.saturating_sub(1);
        rows.extend(
            files[..shown]
                .iter()
                .map(|&node| row(RowKind::Mismatch { node })),
        );
        rows.push(row(RowKind::More {
            count: (files.len() - shown) as u64,
        }));
    }
    let footer = footer(tree, tags, &rows, budget);
    rows.push(footer);
    View { rows, budget }
}

/// High counts first, then by name; empty groups are left out.
fn sorted_counts(map: BTreeMap<&'static str, u64>) -> ClassCounts {
    let mut v: ClassCounts = map.into_iter().filter(|(_, n)| *n > 0).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    v
}
