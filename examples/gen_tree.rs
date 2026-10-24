//! Seeded fixture generator for benchmarks and tests.
//!
//! ```text
//! cargo run --release --example gen_tree -- --files N --seed S --out DIR [--depth D] [--no-ext-share P]
//! ```
//!
//! The same `N` and `S` give the same tree: paths, sizes, mtimes, and content
//! bytes. The mix holds source files, text, data, images, other binaries, data
//! series (`part-00000.csv`, `IMG_0001.jpg`, ...), and files with no extension.
//! `DIR` should be empty or missing; the generator does not delete anything.
//!
//! The tests pull this file in as a module (`tests/support/mod.rs`), so `main`
//! is dead code there.
#![allow(dead_code)]

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

/// All mtimes are before this time (2025-06-15). It matches the test scan time.
pub const BASE_EPOCH: u64 = 1_750_000_000;

pub struct Opts {
    pub files: u64,
    pub seed: u64,
    pub out: PathBuf,
    /// Maximum directory depth below `out`.
    pub depth: u32,
    /// Share of files with no extension, from 0 to 1.
    pub no_ext_share: f64,
}

impl Default for Opts {
    fn default() -> Opts {
        Opts {
            files: 1000,
            seed: 1,
            out: PathBuf::from("gen-tree"),
            depth: 6,
            no_ext_share: 0.05,
        }
    }
}

#[derive(Debug, Default)]
pub struct Stats {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
}

/// splitmix64: small, fast, and stable across Rust versions.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x5DEE_CE66_D1CE_4E5B)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A log-uniform value in `[lo, hi]`.
    fn log_uniform(&mut self, lo: u64, hi: u64) -> u64 {
        let (a, b) = ((lo as f64).ln(), (hi as f64).ln());
        (a + (b - a) * self.unit()).exp() as u64
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len() as u64) as usize]
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Source(&'static str),
    Text(&'static str),
    Data(&'static str),
    Png,
    Jpeg,
    Binary(&'static str),
    NoExt,
}

/// One planned file. Content comes from `content_seed`, so files can be
/// written in parallel and still come out the same.
struct Plan {
    path: PathBuf,
    kind: Kind,
    size: u64,
    mtime: u64,
    content_seed: u64,
}

const DIR_WORDS: &[&str] = &[
    "src", "lib", "core", "api", "util", "docs", "assets", "data", "tests", "tools", "build",
    "config", "models", "scripts", "images", "logs", "web", "cmd", "internal", "pkg",
];
const STEMS: &[&str] = &[
    "main", "index", "parser", "config", "server", "client", "report", "notes", "photo", "table",
    "schema", "worker", "cache", "render", "event", "user", "order", "item", "helper", "types",
];
const SOURCE: &[&str] = &["rs", "py", "js", "ts", "go", "c", "h", "java", "sh"];
const TEXT: &[&str] = &["md", "txt", "rst"];
const DATA: &[&str] = &["json", "csv", "yaml", "toml", "xml"];
const BINARY: &[&str] = &["bin", "gz", "zip", "dat"];
const WORDS: &[&str] = &[
    "the", "tree", "file", "size", "data", "line", "value", "node", "type", "count", "order",
    "fast", "small", "large", "read", "write", "check", "build", "test", "model",
];

fn pick_kind(rng: &mut Rng, no_ext_share: f64) -> Kind {
    if rng.unit() < no_ext_share {
        return Kind::NoExt;
    }
    match rng.below(100) {
        0..=39 => Kind::Source(rng.pick(SOURCE)),
        40..=54 => Kind::Text(rng.pick(TEXT)),
        55..=69 => Kind::Data(rng.pick(DATA)),
        70..=79 => Kind::Png,
        80..=89 => Kind::Jpeg,
        _ => Kind::Binary(rng.pick(BINARY)),
    }
}

fn ext(kind: Kind) -> &'static str {
    match kind {
        Kind::Source(e) | Kind::Text(e) | Kind::Data(e) | Kind::Binary(e) => e,
        Kind::Png => "png",
        Kind::Jpeg => "jpg",
        Kind::NoExt => "",
    }
}

fn file_size(rng: &mut Rng) -> u64 {
    // About 1 in 2000 files is large; the rest are 32 B to 8 KiB.
    if rng.below(2000) == 0 {
        rng.log_uniform(64 * 1024, 2 * 1024 * 1024)
    } else {
        rng.log_uniform(32, 8192)
    }
}

/// From 1 minute to 3 years before `BASE_EPOCH`.
fn mtime(rng: &mut Rng) -> u64 {
    BASE_EPOCH - rng.log_uniform(60, 3 * 365 * 86_400)
}

/// Builds the directory list and the file plan. Sequential, so it is the same
/// for every run with the same options.
fn plan(opts: &Opts) -> (Vec<(PathBuf, u64)>, Vec<Plan>) {
    let mut rng = Rng::new(opts.seed);
    // Directories: (relative path, depth, mtime). Index 0 is the root.
    let n_dirs = (opts.files / 32).max(1) as usize;
    let mut dirs: Vec<(PathBuf, u32)> = vec![(PathBuf::new(), 0)];
    for i in 1..n_dirs {
        let parent = loop {
            let p = rng.below(dirs.len() as u64) as usize;
            if dirs[p].1 < opts.depth {
                break p;
            }
        };
        let name = format!("{}_{i}", rng.pick(DIR_WORDS));
        let path = dirs[parent].0.join(name);
        dirs.push((path, dirs[parent].1 + 1));
    }

    let mut files = Vec::with_capacity(opts.files as usize);
    let mut series_used = std::collections::HashSet::new();
    let mut index = 0u64;
    while index < opts.files {
        let dir = dirs[rng.below(dirs.len() as u64) as usize].0.clone();
        // About 15% of files sit in data series of 10 to 200 members.
        if rng.below(100) < 3 {
            let run = (10 + rng.below(191)).min(opts.files - index);
            let (prefix, digits, kind) = match rng.below(3) {
                0 => ("part-", 5, Kind::Data("csv")),
                1 => ("IMG_", 4, Kind::Jpeg),
                _ => ("frame_", 6, Kind::Png),
            };
            // A second series with the same prefix in one directory gets its
            // own prefix, so no two plans share a path.
            let prefix = if series_used.insert((dir.clone(), prefix)) {
                prefix.to_string()
            } else {
                format!("s{index}_{prefix}")
            };
            let size = file_size(&mut rng);
            let base = mtime(&mut rng);
            for k in 0..run {
                let name = format!("{prefix}{k:0digits$}.{}", ext(kind));
                files.push(Plan {
                    path: dir.join(name),
                    kind,
                    size: size + rng.below(size / 4 + 1),
                    mtime: base - k,
                    content_seed: rng.next(),
                });
            }
            index += run;
            continue;
        }
        let kind = pick_kind(&mut rng, opts.no_ext_share);
        let stem = rng.pick(STEMS);
        let name = match kind {
            Kind::NoExt => format!("{}_{index}", rng.pick(&["run", "NOTES", "blob", "LICENSE"])),
            _ => format!("{stem}_{index}.{}", ext(kind)),
        };
        files.push(Plan {
            path: dir.join(name),
            kind,
            size: file_size(&mut rng),
            mtime: mtime(&mut rng),
            content_seed: rng.next(),
        });
        index += 1;
    }
    let dirs = dirs
        .into_iter()
        .map(|(p, _)| (p, mtime(&mut rng)))
        .collect();
    (dirs, files)
}

fn text_lines(rng: &mut Rng, size: u64, mut line: impl FnMut(&mut Rng, &mut Vec<u8>)) -> Vec<u8> {
    let mut out = Vec::with_capacity(size as usize + 128);
    while (out.len() as u64) < size {
        line(rng, &mut out);
    }
    out
}

fn sentence(rng: &mut Rng, out: &mut Vec<u8>) {
    for i in 0..(4 + rng.below(10)) {
        if i > 0 {
            out.push(b' ');
        }
        out.extend_from_slice(rng.pick(WORDS).as_bytes());
    }
    out.extend_from_slice(b".\n");
}

fn random_bytes(rng: &mut Rng, out: &mut Vec<u8>, n: u64) {
    while (n as usize) > out.len() {
        out.extend_from_slice(&rng.next().to_le_bytes());
    }
    out.truncate(n as usize);
}

fn content(p: &Plan) -> Vec<u8> {
    let mut rng = Rng::new(p.content_seed);
    let size = p.size;
    match p.kind {
        Kind::Source(lang) => {
            let mut out = match lang {
                "py" => b"import os\nimport sys\n\n".to_vec(),
                "sh" => b"#!/bin/sh\nset -eu\n\n".to_vec(),
                "rs" => b"use std::io;\n\n".to_vec(),
                "go" => b"package main\n\nimport \"fmt\"\n\n".to_vec(),
                "java" => b"package app;\n\npublic class App {\n".to_vec(),
                _ => b"#include <stdio.h>\n\n".to_vec(),
            };
            let mut n = 0u64;
            while (out.len() as u64) < size {
                let w = rng.pick(WORDS);
                let line = match lang {
                    "py" => format!("def {w}_{n}(x):\n    return x * {}\n\n", rng.below(100)),
                    "sh" => format!("echo \"{w} {n}\"\n"),
                    "rs" => format!(
                        "pub fn {w}_{n}(x: u32) -> u32 {{\n    x * {}\n}}\n\n",
                        rng.below(100)
                    ),
                    "go" => format!(
                        "func {w}{n}(x int) int {{\n\tfmt.Println(x)\n\treturn x * {}\n}}\n\n",
                        rng.below(9)
                    ),
                    "js" | "ts" => format!(
                        "export function {w}{n}(x) {{\n  return x * {};\n}}\n\n",
                        rng.below(100)
                    ),
                    "java" => format!(
                        "    int {w}{n}(int x) {{ return x * {}; }}\n",
                        rng.below(100)
                    ),
                    _ => format!(
                        "int {w}_{n}(int x) {{\n    return x * {};\n}}\n\n",
                        rng.below(100)
                    ),
                };
                out.extend_from_slice(line.as_bytes());
                n += 1;
            }
            out
        }
        Kind::Text("md") => {
            let mut out = b"# Notes\n\n".to_vec();
            out.extend(text_lines(&mut rng, size, sentence));
            out
        }
        Kind::Text(_) => text_lines(&mut rng, size, sentence),
        Kind::Data("json") => {
            let mut out = b"[\n".to_vec();
            let mut n = 0;
            while (out.len() as u64) < size {
                let sep = if n > 0 { ",\n" } else { "" };
                let rec = format!(
                    "{sep}  {{\"id\": {n}, \"name\": \"{}\", \"value\": {}}}",
                    rng.pick(WORDS),
                    rng.below(1 << 20)
                );
                out.extend_from_slice(rec.as_bytes());
                n += 1;
            }
            out.extend_from_slice(b"\n]\n");
            out
        }
        Kind::Data("csv") => {
            let mut out = b"id,name,value,score\n".to_vec();
            let mut n = 0;
            while (out.len() as u64) < size {
                let row = format!(
                    "{n},{},{},{:.3}\n",
                    rng.pick(WORDS),
                    rng.below(1 << 20),
                    rng.unit()
                );
                out.extend_from_slice(row.as_bytes());
                n += 1;
            }
            out
        }
        Kind::Data("xml") => {
            let mut out = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<items>\n".to_vec();
            while (out.len() as u64) < size {
                out.extend_from_slice(
                    format!(
                        "  <item name=\"{}\">{}</item>\n",
                        rng.pick(WORDS),
                        rng.below(1000)
                    )
                    .as_bytes(),
                );
            }
            out.extend_from_slice(b"</items>\n");
            out
        }
        Kind::Data(fmt) => text_lines(&mut rng, size, |rng, out| {
            let (k, v) = (rng.pick(WORDS), rng.below(1000));
            let line = if fmt == "toml" {
                format!("{k}_{v} = {v}\n")
            } else {
                format!("{k}_{v}: {v}\n")
            };
            out.extend_from_slice(line.as_bytes());
        }),
        Kind::Png => {
            let mut out = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
            out.extend_from_slice(&(64 + rng.below(4000) as u32).to_be_bytes());
            out.extend_from_slice(&(64 + rng.below(4000) as u32).to_be_bytes());
            out.extend_from_slice(b"\x08\x06\0\0\0");
            random_bytes(&mut rng, &mut out, size.max(64));
            out
        }
        Kind::Jpeg => {
            let mut out = b"\xff\xd8\xff\xe0\0\x10JFIF\0\x01\x01\0\0\x01\0\x01\0\0".to_vec();
            random_bytes(&mut rng, &mut out, size.max(64));
            out.extend_from_slice(b"\xff\xd9");
            out
        }
        Kind::Binary(e) => {
            let mut out = match e {
                "gz" => b"\x1f\x8b\x08\0\0\0\0\0\0\x03".to_vec(),
                "zip" => b"PK\x03\x04\x14\0\0\0\x08\0".to_vec(),
                _ => Vec::new(),
            };
            random_bytes(&mut rng, &mut out, size);
            out
        }
        Kind::NoExt => match rng.below(3) {
            0 => {
                let mut out = b"#!/bin/sh\n".to_vec();
                out.extend(text_lines(&mut rng, size, |rng, out| {
                    out.extend_from_slice(format!("echo {}\n", rng.pick(WORDS)).as_bytes())
                }));
                out
            }
            1 => text_lines(&mut rng, size, sentence),
            _ => {
                let mut out = b"\x7fELF\x02\x01\x01\0".to_vec();
                random_bytes(&mut rng, &mut out, size.max(64));
                out
            }
        },
    }
}

fn set_mtime(path: &Path, secs: u64) -> io::Result<()> {
    File::open(path)?.set_modified(UNIX_EPOCH + Duration::from_secs(secs))
}

pub fn generate(opts: &Opts) -> io::Result<Stats> {
    let (dirs, files) = plan(opts);
    for (dir, _) in &dirs {
        fs::create_dir_all(opts.out.join(dir))?;
    }
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = files.len().div_ceil(threads).max(1);
    let bytes = std::thread::scope(|s| {
        let workers: Vec<_> = files
            .chunks(chunk)
            .map(|part| {
                s.spawn(move || -> io::Result<u64> {
                    let mut bytes = 0;
                    for p in part {
                        let path = opts.out.join(&p.path);
                        let data = content(p);
                        fs::write(&path, &data)?;
                        set_mtime(&path, p.mtime)?;
                        bytes += data.len() as u64;
                    }
                    Ok(bytes)
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().expect("writer thread"))
            .sum::<io::Result<u64>>()
    })?;
    // Set directory mtimes last, deepest first: writing a file changes the
    // mtime of its directory, and a child's mtime does not touch its parent.
    let mut order: Vec<_> = dirs.iter().collect();
    order.sort_by_key(|(p, _)| std::cmp::Reverse(p.components().count()));
    for (dir, secs) in order {
        set_mtime(&opts.out.join(dir), *secs)?;
    }
    Ok(Stats {
        files: files.len() as u64,
        dirs: dirs.len() as u64,
        bytes,
    })
}

fn parse_args() -> Result<Opts, String> {
    let mut opts = Opts::default();
    let mut out = None;
    let mut files = None;
    let mut seed = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--files" => files = Some(value()?.parse().map_err(|e| format!("--files: {e}"))?),
            "--seed" => seed = Some(value()?.parse().map_err(|e| format!("--seed: {e}"))?),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--depth" => opts.depth = value()?.parse().map_err(|e| format!("--depth: {e}"))?,
            "--no-ext-share" => {
                opts.no_ext_share = value()?
                    .parse()
                    .map_err(|e| format!("--no-ext-share: {e}"))?;
                if !(0.0..=1.0).contains(&opts.no_ext_share) {
                    return Err("--no-ext-share must be from 0 to 1".into());
                }
            }
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    opts.files = files.ok_or("--files is required")?;
    opts.seed = seed.ok_or("--seed is required")?;
    opts.out = out.ok_or("--out is required")?;
    Ok(opts)
}

fn main() {
    let opts = match parse_args() {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("gen_tree: {e}");
            eprintln!(
                "usage: gen_tree --files N --seed S --out DIR [--depth D] [--no-ext-share P]"
            );
            std::process::exit(2);
        }
    };
    match generate(&opts) {
        Ok(s) => println!(
            "{} files, {} dirs, {} bytes in {}",
            s.files,
            s.dirs,
            s.bytes,
            opts.out.display()
        ),
        Err(e) => {
            eprintln!("gen_tree: {e}");
            std::process::exit(1);
        }
    }
}
