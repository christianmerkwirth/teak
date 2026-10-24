//! Phase-0 Magika benchmark (FR-74 baseline).
//!
//! Magika time on 100 mixed generated files, read through `teak::io`:
//!
//! - `session_new`: build a session only (model load).
//! - `cold_100`: build a session, then identify 100 files one by one.
//! - `warm_100`: identify 100 files with a session that already exists.
//! - `warm_batch_100`: extract features for 100 files, then run one batch.
//! - `warm_par4_100`: 4 sessions with 1 intra-op thread each, 25 files each.

#[path = "../examples/gen_tree.rs"]
mod gen_tree;

use std::path::{Path, PathBuf};
use std::time::Duration;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use magika::{FeaturesOrRuled, Session};
use teak::io::MagikaInput;

const FILES: u64 = 100;

fn files() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/bench-data/magika-100-seed-42");
    if !dir.with_extension("ok").exists() {
        let _ = std::fs::remove_dir_all(&dir);
        let opts = gen_tree::Opts {
            files: FILES,
            seed: 42,
            out: dir.clone(),
            ..gen_tree::Opts::default()
        };
        gen_tree::generate(&opts).expect("generate files");
        std::fs::write(dir.with_extension("ok"), b"").unwrap();
    }
    let mut out = Vec::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p)
            } else {
                out.push(p)
            }
        }
    }
    out.sort();
    assert_eq!(out.len() as u64, FILES);
    out
}

fn identify_all(session: &mut Session, files: &[PathBuf]) -> usize {
    let mut labels = 0;
    for f in files {
        let ty = session
            .identify_content_sync(MagikaInput::open(f).unwrap())
            .unwrap();
        labels += ty.info().label.len();
    }
    labels
}

fn identify_batch(session: &mut Session, files: &[PathBuf]) -> usize {
    let mut features = Vec::new();
    for f in files {
        if let FeaturesOrRuled::Features(x) =
            FeaturesOrRuled::extract_sync(MagikaInput::open(f).unwrap()).unwrap()
        {
            features.push(x);
        }
    }
    session
        .identify_features_batch_sync(&features)
        .unwrap()
        .len()
}

fn bench_magika(c: &mut Criterion) {
    let files = files();
    let mut group = c.benchmark_group("magika");
    group
        .sample_size(20)
        .measurement_time(Duration::from_secs(10));
    group.bench_function("session_new", |b| b.iter(|| Session::new().unwrap()));
    group.throughput(Throughput::Elements(FILES));
    group.bench_function("cold_100", |b| {
        b.iter(|| identify_all(&mut Session::new().unwrap(), &files))
    });
    let mut session = Session::new().unwrap();
    group.bench_function("warm_100", |b| {
        b.iter(|| identify_all(&mut session, &files))
    });
    group.bench_function("warm_batch_100", |b| {
        b.iter(|| identify_batch(&mut session, &files))
    });
    let mut sessions: Vec<Session> = (0..4)
        .map(|_| Session::builder().with_intra_threads(1).build().unwrap())
        .collect();
    group.bench_function("warm_par4_100", |b| {
        b.iter(|| {
            std::thread::scope(|s| {
                let parts = files.chunks(files.len().div_ceil(4));
                let workers: Vec<_> = sessions
                    .iter_mut()
                    .zip(parts)
                    .map(|(se, part)| s.spawn(move || identify_all(se, part)))
                    .collect();
                workers
                    .into_iter()
                    .map(|w| w.join().unwrap())
                    .sum::<usize>()
            })
        })
    });
    group.finish();
}

criterion_group!(benches, bench_magika);
criterion_main!(benches);
