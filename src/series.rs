//! Series: runs of siblings with the same name shape, shown as one row
//! (PRD §7).
//!
//! A name splits into literal text and slots: UUIDs, dates (`2024-05-01`),
//! hex hashes (16 or more hex characters), and digit runs. The shape is the
//! name with each slot replaced by its kind. Files with the same shape (so
//! also the same extension) form a group; a group of `--series-min` or more
//! becomes one series row.

use std::collections::HashMap;

use crate::detect::LabelId;
use crate::tree::{Kind, NodeId, Tree};

/// One row candidate in a directory: a node, or a series of nodes.
#[derive(Clone, Debug)]
pub enum Unit {
    Node(NodeId),
    Series(Box<Series>),
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

/// One piece of the series pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    Lit(Vec<u8>),
    /// `[first–last]`.
    Range(Vec<u8>, Vec<u8>),
    Hash,
    Uuid,
}

#[derive(Clone, Debug)]
pub struct Series {
    pub parts: Vec<Part>,
    /// In name order.
    pub members: Vec<NodeId>,
    /// For one changing number or date: the size of its range, and how
    /// many values in it have no file. Both 0 otherwise.
    pub range: u64,
    pub missing: u64,
    pub total_size: u64,
    pub median_size: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Num,
    Date,
    Hash,
    Uuid,
}

#[derive(Clone, Copy, Debug)]
enum Tok {
    Lit(usize, usize),
    Slot(Slot, usize, usize),
}

fn is_hex(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

/// The length of a UUID at `i` (`8-4-4-4-12` hex digits), if one is there.
fn uuid_at(s: &[u8], i: usize) -> Option<usize> {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let mut j = i;
    for (k, len) in GROUPS.iter().enumerate() {
        if k > 0 {
            if s.get(j) != Some(&b'-') {
                return None;
            }
            j += 1;
        }
        if j + len > s.len() || !s[j..j + len].iter().all(|&b| is_hex(b)) {
            return None;
        }
        j += len;
    }
    let end_ok = s.get(j).is_none_or(|b| !b.is_ascii_alphanumeric());
    end_ok.then_some(j - i)
}

/// The length of a date at `i` (`YYYY-MM-DD`, also with `_` or `.`).
fn date_at(s: &[u8], i: usize) -> Option<usize> {
    let d = |k: usize| s.get(i + k).is_some_and(|b| b.is_ascii_digit());
    if !(0..4).all(d) {
        return None;
    }
    let sep = *s.get(i + 4)?;
    if !matches!(sep, b'-' | b'_' | b'.') || s.get(i + 7) != Some(&sep) {
        return None;
    }
    if !(d(5) && d(6) && d(8) && d(9)) || s.get(i + 10).is_some_and(|b| b.is_ascii_digit()) {
        return None;
    }
    let num = |a: usize| u32::from(s[i + a] - b'0') * 10 + u32::from(s[i + a + 1] - b'0');
    ((1..=12).contains(&num(5)) && (1..=31).contains(&num(8))).then_some(10)
}

/// Splits a name into literal text and slots.
fn tokens(s: &[u8]) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut lit = 0;
    let mut i = 0;
    let push_lit = |out: &mut Vec<Tok>, from: usize, to: usize| {
        if to > from {
            out.push(Tok::Lit(from, to));
        }
    };
    while i < s.len() {
        let boundary = i == 0 || !s[i - 1].is_ascii_alphanumeric();
        let slot = if boundary && let Some(len) = uuid_at(s, i) {
            Some((Slot::Uuid, len))
        } else if (i == 0 || !s[i - 1].is_ascii_digit())
            && let Some(len) = date_at(s, i)
        {
            Some((Slot::Date, len))
        } else if boundary && is_hex(s[i]) {
            let run = s[i..].iter().take_while(|&&b| is_hex(b)).count();
            let end_ok = s.get(i + run).is_none_or(|b| !b.is_ascii_alphanumeric());
            (run >= 16 && end_ok).then_some((Slot::Hash, run))
        } else {
            None
        };
        let slot = slot.or_else(|| {
            s[i].is_ascii_digit().then(|| {
                (
                    Slot::Num,
                    s[i..].iter().take_while(|b| b.is_ascii_digit()).count(),
                )
            })
        });
        match slot {
            Some((kind, len)) => {
                push_lit(&mut out, lit, i);
                out.push(Tok::Slot(kind, i, i + len));
                i += len;
                lit = i;
            }
            None => i += 1,
        }
    }
    push_lit(&mut out, lit, s.len());
    out
}

/// The shape key: literal bytes, and a NUL plus a kind letter per slot.
fn shape(s: &[u8], toks: &[Tok]) -> Vec<u8> {
    let mut key = Vec::with_capacity(s.len());
    for t in toks {
        match *t {
            Tok::Lit(a, b) => key.extend_from_slice(&s[a..b]),
            Tok::Slot(kind, _, _) => {
                key.push(0);
                key.push(match kind {
                    Slot::Num => b'n',
                    Slot::Date => b'd',
                    Slot::Hash => b'h',
                    Slot::Uuid => b'u',
                });
            }
        }
    }
    key
}

/// Splits the children of `dir` into units, in name order.
///
/// Post: every child is in exactly one unit; units are in name order.
pub fn group(tree: &Tree, _dir: NodeId, children: &[NodeId], min: usize) -> Vec<Unit> {
    let mut keys: HashMap<Vec<u8>, usize> = HashMap::new();
    let mut groups: Vec<Vec<NodeId>> = Vec::new();
    let mut group_of: Vec<Option<usize>> = Vec::with_capacity(children.len());
    for &c in children {
        if tree.node(c).kind != Kind::File {
            group_of.push(None);
            continue;
        }
        let name = tree.name(c);
        let toks = tokens(name);
        if !toks.iter().any(|t| matches!(t, Tok::Slot(..))) {
            group_of.push(None);
            continue;
        }
        let key = shape(name, &toks);
        let g = *keys.entry(key).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[g].push(c);
        group_of.push(Some(g));
    }
    let min = min.max(2);
    let mut built: Vec<Option<Box<Series>>> = groups
        .iter()
        .map(|m| (m.len() >= min).then(|| Box::new(build(tree, m))))
        .collect();
    let mut out = Vec::new();
    for (&c, g) in children.iter().zip(group_of) {
        match g {
            Some(g) if groups[g].len() >= min => {
                // The series takes the place of its first member.
                if let Some(s) = built[g].take() {
                    out.push(Unit::Series(s));
                }
            }
            _ => out.push(Unit::Node(c)),
        }
    }
    out
}

/// The series of a group (members in name order).
fn build(tree: &Tree, members: &[NodeId]) -> Series {
    let names: Vec<&[u8]> = members.iter().map(|&m| tree.name(m)).collect();
    let toks: Vec<Vec<Tok>> = names.iter().map(|n| tokens(n)).collect();
    let slot_vals = |m: usize| -> Vec<(Slot, &[u8])> {
        toks[m]
            .iter()
            .filter_map(|t| match *t {
                Tok::Slot(kind, a, b) => Some((kind, &names[m][a..b])),
                Tok::Lit(..) => None,
            })
            .collect()
    };
    let first = slot_vals(0);
    let varying: Vec<usize> = (0..first.len())
        .filter(|&j| (1..members.len()).any(|m| slot_vals(m)[j].1 != first[j].1))
        .collect();
    // Members in value order, for the range ends.
    let key_of = |m: usize| -> Vec<u8> {
        let vals = slot_vals(m);
        let mut k = Vec::new();
        for &j in &varying {
            let (kind, v) = vals[j];
            if kind == Slot::Num {
                // Numbers compare by value: pad to a fixed width.
                let digits = v.iter().skip_while(|&&b| b == b'0').count();
                k.extend(std::iter::repeat_n(b'0', 40usize.saturating_sub(digits)));
                k.extend_from_slice(&v[v.len() - digits..]);
            } else {
                k.extend_from_slice(v);
            }
            k.push(0);
        }
        k
    };
    let lo = (0..members.len()).min_by_key(|&m| key_of(m)).unwrap_or(0);
    let hi = (0..members.len()).max_by_key(|&m| key_of(m)).unwrap_or(0);

    let mut parts = Vec::new();
    let mut slot = 0;
    let mut span: Option<(usize, usize)> = None;
    let push_lit = |parts: &mut Vec<Part>, bytes: &[u8]| match parts.last_mut() {
        Some(Part::Lit(l)) => l.extend_from_slice(bytes),
        _ => parts.push(Part::Lit(bytes.to_vec())),
    };
    let first_var = varying.first().copied();
    let last_var = varying.last().copied();
    for t in &toks[0] {
        match *t {
            Tok::Lit(a, b) => {
                if span.is_none() {
                    push_lit(&mut parts, &names[0][a..b]);
                }
            }
            Tok::Slot(kind, a, _) => {
                let j = slot;
                slot += 1;
                if !varying.contains(&j) {
                    if span.is_none() {
                        push_lit(&mut parts, first[j].1);
                    }
                    continue;
                }
                if varying.len() == 1 {
                    parts.push(match kind {
                        Slot::Hash => Part::Hash,
                        Slot::Uuid => Part::Uuid,
                        _ => Part::Range(slot_vals(lo)[j].1.to_vec(), slot_vals(hi)[j].1.to_vec()),
                    });
                    continue;
                }
                // Several slots change: one range over the whole changing span.
                if Some(j) == first_var {
                    span = Some((a, 0));
                }
                if Some(j) == last_var {
                    let piece = |m: usize| -> Vec<u8> {
                        let slots: Vec<(usize, usize)> = toks[m]
                            .iter()
                            .filter_map(|t| match *t {
                                Tok::Slot(_, a, b) => Some((a, b)),
                                Tok::Lit(..) => None,
                            })
                            .collect();
                        let from = slots[first_var.unwrap_or(0)].0;
                        let to = slots[j].1;
                        names[m][from..to].to_vec()
                    };
                    parts.push(Part::Range(piece(lo), piece(hi)));
                    span = None;
                }
            }
        }
    }

    let (range, missing) = match varying.as_slice() {
        [j] => gaps(members.len(), |m| slot_vals(m)[*j]),
        _ => (0, 0),
    };
    let mut sizes: Vec<u64> = members.iter().map(|&m| tree.node(m).size).collect();
    sizes.sort_unstable();
    let n = sizes.len();
    let median_size = if n % 2 == 1 {
        sizes[n / 2]
    } else {
        (sizes[n / 2 - 1] + sizes[n / 2]) / 2
    };
    Series {
        parts,
        members: members.to_vec(),
        range,
        missing,
        total_size: sizes.iter().sum(),
        median_size,
    }
}

/// For one changing number or date slot: (range size, missing values).
fn gaps<'a>(n: usize, value: impl Fn(usize) -> (Slot, &'a [u8])) -> (u64, u64) {
    let mut vals: Vec<i64> = Vec::with_capacity(n);
    for m in 0..n {
        let (kind, v) = value(m);
        let parsed = match kind {
            Slot::Num if v.len() <= 15 => std::str::from_utf8(v).ok().and_then(|s| s.parse().ok()),
            Slot::Date => day_number(v),
            _ => None,
        };
        match parsed {
            Some(x) => vals.push(x),
            None => return (0, 0),
        }
    }
    vals.sort_unstable();
    vals.dedup();
    let (Some(&lo), Some(&hi)) = (vals.first(), vals.last()) else {
        return (0, 0);
    };
    let range = (hi - lo + 1) as u64;
    if range > 1_000_000_000 {
        return (0, 0);
    }
    (range, range - vals.len() as u64)
}

/// Days since 1970-01-01 for `YYYY?MM?DD`.
fn day_number(v: &[u8]) -> Option<i64> {
    let num = |a: usize, b: usize| std::str::from_utf8(&v[a..b]).ok()?.parse::<i64>().ok();
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    // Days from civil (H. Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// A directory where more than 90% of the files are one series with one
/// type (FR-47).
#[derive(Clone, Copy, Debug)]
pub struct Uniform {
    pub label: LabelId,
    pub median_size: u64,
}

/// The uniform summary of a directory, from its units. Only directories
/// with no subdirectory rows count.
pub fn uniform(tree: &Tree, units: &[Unit], labels: &[LabelId]) -> Option<Uniform> {
    let mut files = 0usize;
    let mut best: Option<&Series> = None;
    for u in units {
        match u {
            Unit::Node(id) if tree.node(*id).is_dir() => return None,
            Unit::Node(_) => files += 1,
            Unit::Series(s) => {
                files += s.members.len();
                if best.is_none_or(|b| s.members.len() > b.members.len()) {
                    best = Some(s);
                }
            }
        }
    }
    let s = best?;
    let label = labels[s.members[0] as usize];
    let one_type = s.members.iter().all(|&m| labels[m as usize] == label);
    (one_type && s.members.len() * 10 > files * 9).then_some(Uniform {
        label,
        median_size: s.median_size,
    })
}
