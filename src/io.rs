//! The one gate for all file-content reads.
//!
//! Every byte that teak reads from a file goes through this module, so the
//! `test-hooks` feature can count reads per path and add a delay to each read.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Reads at most `max` bytes from the start of `path`.
///
/// The caller makes sure that `path` is a regular file and not on a pseudo
/// file system; this function does not stat it again.
pub fn read_head(path: &Path, max: usize) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut buf = Vec::new();
    file.take(max as u64).read_to_end(&mut buf)?;
    #[cfg(feature = "test-hooks")]
    {
        hooks::count_read(path, buf.len() as u64);
        hooks::delay();
    }
    Ok(buf)
}

/// Reads at most `len` bytes of `path` from `offset`.
///
/// The caller makes sure that `path` is a regular file and not on a pseudo
/// file system.
pub fn read_at(path: &Path, offset: u64, len: usize) -> io::Result<Vec<u8>> {
    use std::io::{Seek, SeekFrom};
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::new();
    file.take(len as u64).read_to_end(&mut buf)?;
    #[cfg(feature = "test-hooks")]
    {
        hooks::count_read(path, buf.len() as u64);
        hooks::delay();
    }
    Ok(buf)
}

/// Feeds all bytes of `path` to `f`, in blocks of 64 KiB.
///
/// The caller makes sure that `path` is a regular file and not on a pseudo
/// file system.
pub fn read_each(path: &Path, mut f: impl FnMut(&[u8])) -> io::Result<u64> {
    let mut file = File::open(path)?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        f(&buf[..n]);
        total += n as u64;
    }
    #[cfg(feature = "test-hooks")]
    {
        hooks::count_read(path, total);
        hooks::delay();
    }
    Ok(total)
}

/// Reads at most `len` bytes of `path` from `offset`, for a preview: after
/// a type-cache hit, or for an image header or a zip directory. The hooks
/// count these reads apart from detection reads (`preview_reads`).
///
/// The caller makes sure that `path` is a regular file and not on a pseudo
/// file system.
pub fn read_preview(path: &Path, offset: u64, len: usize) -> io::Result<Vec<u8>> {
    use std::io::{Seek, SeekFrom};
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::new();
    file.take(len as u64).read_to_end(&mut buf)?;
    #[cfg(feature = "test-hooks")]
    hooks::count_preview_read(path, buf.len() as u64);
    Ok(buf)
}

/// Reads a rule file (`.gitignore`, `.gitattributes`) of at most `max`
/// bytes. Rule files steer the walk; they are not content, so the hooks do
/// not count them (FR-52 counts content reads only).
///
/// The caller makes sure that `path` is a regular file and not on a pseudo
/// file system.
pub fn read_rule_file(path: &Path, max: usize) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut buf = Vec::new();
    file.take(max as u64).read_to_end(&mut buf)?;
    Ok(buf)
}

/// A file that Magika reads through its `SyncInput` trait.
///
/// Magika picks the offsets (the first and the last block of the file); each
/// `read_at` goes through this gate, so the hooks see the bytes. The read delay
/// applies once per file, when it opens. The block at offset 0 is kept, so
/// the secret check needs no second read.
#[cfg(feature = "magika")]
pub struct MagikaInput {
    file: File,
    len: u64,
    head: Vec<u8>,
    #[cfg(feature = "test-hooks")]
    path: std::path::PathBuf,
}

#[cfg(feature = "magika")]
impl MagikaInput {
    pub fn open(path: &Path) -> io::Result<MagikaInput> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        #[cfg(feature = "test-hooks")]
        hooks::delay();
        Ok(MagikaInput {
            file,
            len,
            head: Vec::new(),
            #[cfg(feature = "test-hooks")]
            path: path.to_path_buf(),
        })
    }

    /// The bytes that Magika read at offset 0.
    pub fn into_head(self) -> Vec<u8> {
        self.head
    }
}

#[cfg(feature = "magika")]
impl magika::SyncInput for MagikaInput {
    fn length(&self) -> magika::Result<u64> {
        Ok(self.len)
    }

    fn read_at(&mut self, buffer: &mut [u8], offset: u64) -> magika::Result<()> {
        use std::os::unix::fs::FileExt;
        self.file.read_exact_at(buffer, offset)?;
        if offset == 0 && self.head.is_empty() {
            self.head = buffer.to_vec();
        }
        #[cfg(feature = "test-hooks")]
        hooks::count_read(&self.path, buffer.len() as u64);
        Ok(())
    }
}

/// Test signals. Only in builds with the `test-hooks` feature.
///
/// - `TEAK_TEST_READ_DELAY_MS`: sleep this long on each content read.
/// - `TEAK_TEST_NO_CONTENT_CHECKS=1`: no content checks.
/// - `TEAK_TEST_REPORT`: [`write_report`] writes the read counters and stage
///   times to this file as JSON.
#[cfg(feature = "test-hooks")]
pub mod hooks {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    static READS: Mutex<BTreeMap<PathBuf, u64>> = Mutex::new(BTreeMap::new());
    static STAGES: Mutex<BTreeMap<&'static str, f64>> = Mutex::new(BTreeMap::new());
    static COUNTS: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());
    static PREVIEW_READS: Mutex<BTreeMap<PathBuf, u64>> = Mutex::new(BTreeMap::new());

    /// Adds one to `counts[name]`, for example `magika` per Magika call.
    pub fn count(name: &'static str) {
        let mut counts = COUNTS.lock().unwrap_or_else(|e| e.into_inner());
        *counts.entry(name).or_default() += 1;
    }

    pub(crate) fn count_read(path: &Path, bytes: u64) {
        let mut reads = READS.lock().unwrap_or_else(|e| e.into_inner());
        *reads.entry(path.to_path_buf()).or_default() += bytes;
    }

    pub(crate) fn count_preview_read(path: &Path, bytes: u64) {
        let mut reads = PREVIEW_READS.lock().unwrap_or_else(|e| e.into_inner());
        *reads.entry(path.to_path_buf()).or_default() += bytes;
    }

    /// `TEAK_TEST_NO_CONTENT_CHECKS=1`: the content checks do not run.
    pub fn content_checks_off() -> bool {
        std::env::var("TEAK_TEST_NO_CONTENT_CHECKS").is_ok_and(|v| v == "1")
    }

    /// Reads the env var on each call, so tests can change it at run time.
    pub(crate) fn delay() {
        let ms = std::env::var("TEAK_TEST_READ_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok());
        if let Some(ms) = ms.filter(|&ms| ms > 0) {
            std::thread::sleep(Duration::from_millis(ms));
        }
    }

    /// Adds its elapsed time to `stages_ms[name]` when it drops.
    pub struct StageTimer {
        name: &'static str,
        start: Instant,
    }

    impl Drop for StageTimer {
        fn drop(&mut self) {
            let ms = self.start.elapsed().as_secs_f64() * 1000.0;
            let mut stages = STAGES.lock().unwrap_or_else(|e| e.into_inner());
            *stages.entry(self.name).or_default() += ms;
        }
    }

    pub fn stage(name: &'static str) -> StageTimer {
        StageTimer {
            name,
            start: Instant::now(),
        }
    }

    /// Writes `{"reads": {...}, "stages_ms": {...}, "counts": {...},
    /// "preview_reads": {...}}` to `TEAK_TEST_REPORT`, if set.
    pub fn write_report() {
        let Some(path) = std::env::var_os("TEAK_TEST_REPORT") else {
            return;
        };
        let reads: serde_json::Map<_, _> = READS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(p, n)| (p.to_string_lossy().into_owned(), (*n).into()))
            .collect();
        let stages: serde_json::Map<_, _> = STAGES
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(s, ms)| (s.to_string(), (*ms).into()))
            .collect();
        let counts: serde_json::Map<_, _> = COUNTS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(s, n)| (s.to_string(), (*n).into()))
            .collect();
        let previews: serde_json::Map<_, _> = PREVIEW_READS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(p, n)| (p.to_string_lossy().into_owned(), (*n).into()))
            .collect();
        let report = serde_json::json!({
            "reads": reads,
            "stages_ms": stages,
            "counts": counts,
            "preview_reads": previews,
        });
        if let Err(e) = std::fs::write(&path, report.to_string()) {
            eprintln!(
                "teak: cannot write test report {}: {e}",
                Path::new(&path).display()
            );
        }
    }
}
