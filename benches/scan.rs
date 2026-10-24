//! The P1 pipeline on generated trees (FR-71 baseline): walk, chaff tags,
//! totals, salience, selection, fast detection on the visible rows, and
//! layout, through the library.
//!
//! Trees come from `benches/walk.rs` (`target/bench-data/files-<N>-seed-42`);
//! run `cargo bench --bench walk` once to make them.

use std::path::{Path, PathBuf};
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

const CASES: &[(&str, u64)] = &[("10k", 10_000), ("100k", 100_000)];

fn tree(files: u64) -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("target/bench-data/files-{files}-seed-42"));
    dir.with_extension("ok").exists().then_some(dir)
}

fn scan_and_layout(root: &Path) -> usize {
    let opts = match teak::cli::parse_from(["teak", "--detect", "fast"]).unwrap() {
        teak::cli::Parsed::Run(o) => o,
        teak::cli::Parsed::Version => unreachable!(),
    };
    let env = teak::Env::fixed(1_750_000_000);
    let set = teak::settings(&opts, &env);
    let mut cx = teak::Context::new(&opts);
    let scan = teak::build(root, &opts, &mut cx, set.budget, env.scan_time).unwrap();
    teak::layout::fit(&scan, &teak::layout_opts(&opts, &set)).len()
}

fn bench_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("scan");
    group
        .sample_size(10)
        .measurement_time(Duration::from_secs(10));
    for &(name, files) in CASES {
        let Some(root) = tree(files) else {
            eprintln!("skipping {name}: run `cargo bench --bench walk` to make the tree");
            continue;
        };
        group.bench_with_input(BenchmarkId::new("pipeline", name), &root, |b, root| {
            b.iter(|| scan_and_layout(root))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_scan);
criterion_main!(benches);
