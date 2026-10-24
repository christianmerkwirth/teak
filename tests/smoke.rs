//! Phase-0 smoke tests: version line, the `io` gate and its hooks, and the
//! fixture generator.

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use support::{Fixture, gen_tree, teak};

#[test]
fn version_prints_one_line_with_magika_state() {
    let fx = Fixture::new();
    let out = teak(&fx, &["--version"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let want = if cfg!(feature = "magika") {
        format!(
            "teak {} (magika: on, model {})\n",
            env!("CARGO_PKG_VERSION"),
            model_name()
        )
    } else {
        format!("teak {} (magika: off)\n", env!("CARGO_PKG_VERSION"))
    };
    assert_eq!(stdout, want);
}

#[cfg(feature = "magika")]
fn model_name() -> &'static str {
    magika::MODEL_NAME
}

#[cfg(not(feature = "magika"))]
fn model_name() -> &'static str {
    unreachable!()
}

fn ten_kib() -> Vec<u8> {
    (0..10 * 1024).map(|i| (i % 251) as u8).collect()
}

#[test]
fn read_head_reads_at_most_max_bytes() {
    let fx = Fixture::new();
    let bytes = ten_kib();
    fx.file("big.bin", &bytes).file("small.txt", "hello");
    let head = teak::io::read_head(&fx.path().join("big.bin"), 4096).unwrap();
    assert_eq!(head, bytes[..4096]);
    let head = teak::io::read_head(&fx.path().join("small.txt"), 4096).unwrap();
    assert_eq!(head, b"hello");
}

#[test]
fn read_head_on_missing_file_is_an_error() {
    let fx = Fixture::new();
    assert!(teak::io::read_head(&fx.path().join("nope"), 16).is_err());
}

#[cfg(feature = "magika")]
#[test]
fn magika_identifies_through_the_io_gate() {
    let fx = Fixture::new();
    let src = "import os\nimport sys\n\n\ndef main(argv):\n    for name in os.listdir(argv[1]):\n        \
               print(name, os.path.getsize(os.path.join(argv[1], name)))\n    return 0\n\n\n\
               if __name__ == \"__main__\":\n    sys.exit(main(sys.argv))\n";
    fx.file("list.py", src);
    let input = teak::io::MagikaInput::open(&fx.path().join("list.py")).unwrap();
    let mut session = magika::Session::new().unwrap();
    let ty = session.identify_content_sync(input).unwrap();
    assert_eq!(ty.info().label, "python");
}

/// Hook tests share process-wide env vars, so they run one at a time.
#[cfg(feature = "test-hooks")]
mod hooks {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use super::*;
    use support::read_report;

    static ENV: Mutex<()> = Mutex::new(());

    fn with_env<T>(vars: &[(&str, &str)], f: impl FnOnce() -> T) -> T {
        let _guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: the ENV mutex keeps all env access in this binary serial.
        unsafe {
            for (k, v) in vars {
                std::env::set_var(k, v);
            }
        }
        let result = f();
        unsafe {
            for (k, _) in vars {
                std::env::remove_var(k);
            }
        }
        result
    }

    #[test]
    fn report_records_bytes_read_per_path() {
        let fx = Fixture::new();
        fx.file("big.bin", ten_kib());
        let file = fx.path().join("big.bin");
        let report = fx.path().join("report.json");
        let parsed = with_env(&[("TEAK_TEST_REPORT", report.to_str().unwrap())], || {
            teak::io::read_head(&file, 4096).unwrap();
            teak::io::hooks::write_report();
            read_report(&report)
        });
        assert_eq!(parsed.reads.get(file.to_str().unwrap()), Some(&4096));
    }

    #[test]
    fn read_delay_applies_to_each_read() {
        let fx = Fixture::new();
        fx.file("a.txt", "a").file("b.txt", "b").file("c.txt", "c");
        with_env(&[("TEAK_TEST_READ_DELAY_MS", "10")], || {
            for name in ["a.txt", "b.txt", "c.txt"] {
                let start = Instant::now();
                teak::io::read_head(&fx.path().join(name), 16).unwrap();
                assert!(
                    start.elapsed() >= Duration::from_millis(10),
                    "{name} took {:?}",
                    start.elapsed()
                );
            }
        });
    }

    #[test]
    fn stage_timer_adds_its_time_to_the_report() {
        let fx = Fixture::new();
        let report = fx.path().join("report.json");
        let parsed = with_env(&[("TEAK_TEST_REPORT", report.to_str().unwrap())], || {
            {
                let _t = teak::io::hooks::stage("phase0-test");
                std::thread::sleep(Duration::from_millis(5));
            }
            teak::io::hooks::write_report();
            read_report(&report)
        });
        assert!(
            parsed.stages_ms["phase0-test"] >= 5.0,
            "{:?}",
            parsed.stages_ms
        );
    }

    #[cfg(feature = "magika")]
    #[test]
    fn magika_reads_are_counted() {
        let fx = Fixture::new();
        fx.file("big.bin", ten_kib());
        let file = fx.path().join("big.bin");
        let report = fx.path().join("report.json");
        let parsed = with_env(&[("TEAK_TEST_REPORT", report.to_str().unwrap())], || {
            let input = teak::io::MagikaInput::open(&file).unwrap();
            magika::Session::new()
                .unwrap()
                .identify_content_sync(input)
                .unwrap();
            teak::io::hooks::write_report();
            read_report(&report)
        });
        let bytes = parsed.reads[file.to_str().unwrap()];
        assert!(bytes > 0 && bytes < 10 * 1024, "magika read {bytes} bytes");
    }
}

/// Hashes every path, size, mtime, and content byte under `root`.
fn tree_hash(root: &Path) -> u64 {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, (u64, i64, Vec<u8>)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).unwrap();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let mtime = meta
                .modified()
                .unwrap()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            if meta.is_dir() {
                out.insert(rel + "/", (0, mtime, Vec::new()));
                walk(&path, root, out);
            } else {
                out.insert(rel, (meta.len(), mtime, fs::read(&path).unwrap()));
            }
        }
    }
    let mut all = BTreeMap::new();
    walk(root, root, &mut all);
    // FNV-1a over the sorted records.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for (path, (size, mtime, content)) in &all {
        eat(path.as_bytes());
        eat(&size.to_le_bytes());
        eat(&mtime.to_le_bytes());
        eat(content);
    }
    h
}

fn generate(files: u64, seed: u64, out: &Path) {
    gen_tree::generate(&gen_tree::Opts {
        files,
        seed,
        out: out.to_path_buf(),
        ..gen_tree::Opts::default()
    })
    .unwrap();
}

#[test]
fn gen_tree_is_deterministic_per_seed() {
    let (a, b, c) = (Fixture::new(), Fixture::new(), Fixture::new());
    generate(400, 7, a.path());
    generate(400, 7, b.path());
    generate(400, 8, c.path());
    assert_eq!(tree_hash(a.path()), tree_hash(b.path()));
    assert_ne!(tree_hash(a.path()), tree_hash(c.path()));
}

#[test]
fn gen_tree_makes_the_requested_mix() {
    let fx = Fixture::new();
    let stats = gen_tree::generate(&gen_tree::Opts {
        files: 2000,
        seed: 1,
        out: fx.path().to_path_buf(),
        ..gen_tree::Opts::default()
    })
    .unwrap();
    assert_eq!(stats.files, 2000);
    let mut exts = BTreeMap::<String, u64>::new();
    let mut series = 0;
    let mut stack = vec![fx.path().to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            series += u64::from(name.starts_with("part-"));
            let ext = path
                .extension()
                .map_or(String::new(), |e| e.to_string_lossy().into_owned());
            *exts.entry(ext).or_default() += 1;
        }
    }
    assert_eq!(exts.values().sum::<u64>(), 2000);
    for want in ["", "rs", "py", "png", "jpg", "md", "csv"] {
        assert!(
            exts.get(want).copied().unwrap_or(0) > 0,
            "no .{want} files in {exts:?}"
        );
    }
    assert!(series > 0, "no data series");
}
