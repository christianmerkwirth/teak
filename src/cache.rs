//! The type cache (FR-54, design.md §Type cache format).
//!
//! One flat file, `types.bin`, in the user cache directory. teak loads it
//! whole at the start and writes it at the end (a temp file, then a
//! rename). The key `(dev, ino, size, mtime_ns)` never gives a type for a
//! changed file. A bad magic, version, model version, or label table, or a
//! short file, gives an empty cache and no error.
//!
//! Layout, little endian: `TEAKC`, `format_version u16`, the model version
//! (`u16` length and bytes), `labels u64` (a hash of the label table),
//! `templates u64` (a hash of the template table), `count u32`, then
//! `count` records of [`RECORD`] bytes, sorted by `(dev, ino)`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::chaff::ChaffClass;
use crate::chaff::templates::fnv1a64;
use crate::detect::{LABELS, LabelId, Source};
use crate::fnv::FnvMap;

const MAGIC: &[u8; 5] = b"TEAKC";
const FORMAT_VERSION: u16 = 1;
const FILE: &str = "types.bin";

/// dev, ino, size, mtime_ns, label, score, source, flags, chaff, template,
/// last_used, and one pad byte.
const RECORD: usize = 48;

/// The most entries that the file keeps; the oldest `last_used` go first.
pub const MAX_ENTRIES: usize = 200_000;

/// `last_used` is rewritten only when it is older than this (seconds), so a
/// run with only hits does not write the file.
const TOUCH_AGE: u32 = 24 * 3600;

const FLAG_FULL: u8 = 1;
const FLAG_SECRET: u8 = 2;
const FLAG_CHECKED: u8 = 4;

/// The stat data that a cached type is valid for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub dev: u64,
    pub ino: u64,
    pub size: u64,
    pub mtime_ns: i64,
}

impl Key {
    pub fn of(node: &crate::tree::Node) -> Key {
        Key {
            dev: node.dev,
            ino: node.ino,
            size: node.size,
            mtime_ns: node.mtime_ns,
        }
    }
}

/// A content-check result: the class (or none) and the template id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Check {
    pub class: Option<ChaffClass>,
    pub template: u16,
}

/// What the cache knows about one file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub label: LabelId,
    pub score: f32,
    pub source: Source,
    /// Typed in a `--detect full` run.
    pub full: bool,
    /// A private-key header in the content.
    pub secret: bool,
    /// The content checks ran (with this template table).
    pub check: Option<Check>,
}

#[derive(Clone, Copy)]
struct Entry {
    size: u64,
    mtime_ns: i64,
    hit: Hit,
    last_used: u32,
}

pub struct Cache {
    dir: PathBuf,
    model: String,
    templates: u64,
    entries: FnvMap<(u64, u64), Entry>,
    /// Seconds since the epoch, for `last_used`.
    now: u32,
    dirty: bool,
}

/// The cache directory: `$XDG_CACHE_HOME/teak` when it is set (on macOS
/// too, so tests can point it anywhere), else `~/Library/Caches/teak` on
/// macOS and `~/.cache/teak` elsewhere.
pub fn dir_from_env() -> Option<PathBuf> {
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(xdg) = var("XDG_CACHE_HOME").filter(|p| p.is_absolute()) {
        return Some(xdg.join("teak"));
    }
    let home = var("HOME")?;
    if cfg!(target_os = "macos") {
        Some(home.join("Library/Caches/teak"))
    } else {
        Some(home.join(".cache/teak"))
    }
}

/// A hash of the label names: the ids in the file mean the same labels.
fn labels_hash() -> u64 {
    let mut bytes = Vec::new();
    for l in LABELS {
        bytes.extend_from_slice(l.name.as_bytes());
        bytes.push(0);
    }
    fnv1a64(&bytes)
}

const SOURCES: [Source; 6] = [
    Source::Stat,
    Source::Name,
    Source::Magika,
    Source::Magic,
    Source::Ext,
    Source::None,
];

impl Cache {
    /// An empty cache that saves to `dir`.
    pub fn empty(dir: &Path, model_version: &str) -> Cache {
        Cache {
            dir: dir.to_path_buf(),
            model: model_version.to_string(),
            templates: 0,
            entries: FnvMap::default(),
            now: 0,
            dirty: false,
        }
    }

    /// Loads `dir/types.bin`.
    ///
    /// Post: a missing, corrupt, or foreign file gives an empty cache and no
    /// error.
    pub fn load(dir: &Path, model_version: &str) -> Cache {
        let mut cache = Cache::empty(dir, model_version);
        if let Ok(bytes) = fs::read(dir.join(FILE)) {
            cache.parse(&bytes);
        }
        cache
    }

    fn parse(&mut self, bytes: &[u8]) -> Option<()> {
        let mut r = Reader(bytes);
        if r.take(5)? != MAGIC || r.u16()? != FORMAT_VERSION {
            return None;
        }
        let len = r.u16()? as usize;
        if r.take(len)? != self.model.as_bytes() || r.u64()? != labels_hash() {
            return None;
        }
        let templates = r.u64()?;
        let count = r.u32()? as usize;
        if r.0.len() != count * RECORD {
            return None;
        }
        let mut entries = FnvMap::with_capacity_and_hasher(count, Default::default());
        for _ in 0..count {
            let (dev, ino, size) = (r.u64()?, r.u64()?, r.u64()?);
            let mtime_ns = r.u64()? as i64;
            let label = r.u16()?;
            let score = f32::from_bits(r.u32()?);
            let source = *SOURCES.get(r.u8()? as usize)?;
            let flags = r.u8()?;
            let chaff = r.u8()?;
            let template = r.u16()?;
            let last_used = r.u32()?;
            r.take(1)?;
            if label as usize >= LABELS.len() {
                return None;
            }
            let class = match chaff {
                0 => None,
                n => Some(*ChaffClass::ALL.get(n as usize - 1)?),
            };
            let hit = Hit {
                label,
                score,
                source,
                full: flags & FLAG_FULL != 0,
                secret: flags & FLAG_SECRET != 0,
                check: (flags & FLAG_CHECKED != 0).then_some(Check { class, template }),
            };
            entries.insert(
                (dev, ino),
                Entry {
                    size,
                    mtime_ns,
                    hit,
                    last_used,
                },
            );
        }
        self.templates = templates;
        self.entries = entries;
        Some(())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Sets the run's template table and time. Check results from another
    /// table are dropped; the types stay.
    pub fn begin(&mut self, templates: u64, now: i64) {
        self.now = now.clamp(0, u32::MAX as i64) as u32;
        if templates != self.templates {
            for e in self.entries.values_mut() {
                e.hit.check = None;
            }
            self.templates = templates;
            self.dirty = true;
        }
    }

    /// The entry for `key`, if the file has not changed.
    pub fn get(&mut self, key: Key) -> Option<Hit> {
        let now = self.now;
        let e = self.entries.get_mut(&(key.dev, key.ino))?;
        if e.size != key.size || e.mtime_ns != key.mtime_ns {
            return None;
        }
        if now.saturating_sub(e.last_used) > TOUCH_AGE {
            e.last_used = now;
            self.dirty = true;
        }
        Some(e.hit)
    }

    pub fn put(&mut self, key: Key, hit: Hit) {
        self.entries.insert(
            (key.dev, key.ino),
            Entry {
                size: key.size,
                mtime_ns: key.mtime_ns,
                hit,
                last_used: self.now,
            },
        );
        self.dirty = true;
    }

    /// Adds a check result to the entry for `key`.
    pub fn set_check(&mut self, key: Key, check: Check) {
        if let Some(e) = self.entries.get_mut(&(key.dev, key.ino))
            && e.size == key.size
            && e.mtime_ns == key.mtime_ns
        {
            e.hit.check = Some(check);
            self.dirty = true;
        }
    }

    /// The directory the cache saves to.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Writes the file when something changed: a temp file in the cache
    /// directory, then a rename. Keeps the newest [`MAX_ENTRIES`].
    pub fn save(self) -> io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let mut list: Vec<((u64, u64), Entry)> = self.entries.into_iter().collect();
        if list.len() > MAX_ENTRIES {
            list.sort_unstable_by(|a, b| b.1.last_used.cmp(&a.1.last_used).then(a.0.cmp(&b.0)));
            list.truncate(MAX_ENTRIES);
        }
        list.sort_unstable_by_key(|(k, _)| *k);
        let mut out = Vec::with_capacity(64 + list.len() * RECORD);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        out.extend_from_slice(&(self.model.len() as u16).to_le_bytes());
        out.extend_from_slice(self.model.as_bytes());
        out.extend_from_slice(&labels_hash().to_le_bytes());
        out.extend_from_slice(&self.templates.to_le_bytes());
        out.extend_from_slice(&(list.len() as u32).to_le_bytes());
        for ((dev, ino), e) in &list {
            let h = &e.hit;
            let source = SOURCES.iter().position(|s| *s == h.source).unwrap_or(5) as u8;
            let mut flags = 0;
            if h.full {
                flags |= FLAG_FULL;
            }
            if h.secret {
                flags |= FLAG_SECRET;
            }
            if h.check.is_some() {
                flags |= FLAG_CHECKED;
            }
            let chaff = h.check.and_then(|c| c.class).map_or(0, |c| {
                ChaffClass::ALL.iter().position(|x| *x == c).unwrap_or(0) as u8 + 1
            });
            out.extend_from_slice(&dev.to_le_bytes());
            out.extend_from_slice(&ino.to_le_bytes());
            out.extend_from_slice(&e.size.to_le_bytes());
            out.extend_from_slice(&e.mtime_ns.to_le_bytes());
            out.extend_from_slice(&h.label.to_le_bytes());
            out.extend_from_slice(&h.score.to_bits().to_le_bytes());
            out.extend_from_slice(&[source, flags, chaff]);
            out.extend_from_slice(&h.check.map_or(0, |c| c.template).to_le_bytes());
            out.extend_from_slice(&e.last_used.to_le_bytes());
            out.push(0);
        }
        fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join(format!(".{FILE}.{}", std::process::id()));
        let written = fs::File::create(&tmp)
            .and_then(|mut f| f.write_all(&out))
            .and_then(|()| fs::rename(&tmp, self.dir.join(FILE)));
        if written.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        written
    }
}

/// Reads little-endian numbers from the front of a byte slice.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Some(head)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
}
