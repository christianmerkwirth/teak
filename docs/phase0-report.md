# teak — phase-0 report

Date: 2026-10-24 · Task: `.sddw/teak/design/tasks/task-1-phase0-spike.md`

Phase 0 tests the risky parts of the plan before P1 starts: the ONNX Runtime
build, the binary size, the Magika speed, and the walk speed and memory on 1M
files. This report gives a go or no-go for FR-71 to FR-75. After you approve
it, those FRs become SHALL.

## Summary

| FR | Target | Phase-0 basis (median) | Verdict |
|---|---|---|---|
| FR-71 | 10k files, warm cache: < 150 ms | walk + arena 6.4 ms, start-up 1.4 ms | **Go** |
| FR-72 | 100k files, `--detect full --no-cache`: < 1 s | walk + arena 34 ms, Magika 1.75 ms per file | **No-go as written** (FR-51's 2 s cap); go with Plan A1 |
| FR-73 | 1M files, `--no-cache`: < 5 s, RSS < 300 MB | walk + arena 358 ms, peak RSS 242 MB | **Go, with a risk** (RSS headroom 20%) |
| FR-74 | Magika on about 100 visible files: < 300 ms | 186 ms cold, 175 ms warm | **Go** |
| FR-75 | warm cache: detect < 5% of total, 0 reads | design property; not measurable yet | **Go, with a condition** (cache load outside the detect stage) |

Another finding: **pyke has no prebuilt ONNX Runtime for Intel macOS**
(`x86_64-apple-darwin`). This is a no-go for the `magika` feature on that one
target. See [Plan B](#plan-b-intel-macos).

Four items need your approval: plans A1, A2, A3, and B below. Plan A1
changes the requirements (a Magika sub-cap inside FR-51).

## Reference machine

| Part | Value |
|---|---|
| CPU | Intel Core i5-12600K (12th gen; 6 P-cores + 4 E-cores, 16 threads) |
| RAM | 62 GiB |
| Disk | XPG GAMMIX S70, 1 TB NVMe SSD, ext4 |
| OS | Ubuntu 24.04.5 LTS, Linux 6.8.0-147-generic, x86_64 |
| Toolchain | rustc 1.98.1, cargo 1.98.1 |
| Tools | hyperfine 2.0.0, criterion 0.7 |

All times are the median from criterion (10 samples for the walk, 20 for
Magika) or hyperfine (3 warmup runs, 10 runs). The OS cache was warm.

## Build

### ONNX Runtime link method: static

- `magika` 1.1.0 pins `ort = 2.0.0-rc.12` (ONNX Runtime 1.24.2) with no default
  features. teak turns on `ort/download-binaries` and `ort/tls-rustls`.
- At build time, the `ort-sys` build script downloads `libonnxruntime.a` from
  `cdn.pyke.io`, checks its SHA-256 hash, and links it statically. It keeps a
  copy in `~/.cache/ort.pyke.io/`.
- `ldd target/release/teak` shows only `libstdc++`, `libgcc_s`, `libm`, and `libc`.
  No `libonnxruntime.so` is needed at run time.
- The Magika model (`standard_v3_3`, 3.2 MB of ONNX) is built into the binary by
  the `magika` crate. Nothing downloads at run time (FR-10).
- `tls-rustls` was chosen over `tls-native`, so the build needs no OpenSSL
  headers.

Prebuilt targets in `ort-sys` rc.12 (`build/download/dist.txt`):

| Target | Prebuilt |
|---|---|
| `x86_64-unknown-linux-gnu` | yes |
| `aarch64-unknown-linux-gnu` | yes |
| `aarch64-apple-darwin` | yes |
| `x86_64-apple-darwin` | **no** |

### Release binary size

Release profile: `lto = "thin"`, `codegen-units = 1`, `strip = true`.

| Platform | `magika` (default) | `--no-default-features` |
|---|---|---|
| Linux x86_64 (reference machine) | 19,876,424 B (19.0 MiB) | 343,384 B (0.33 MiB) |
| macOS arm64 | from CI — pending | from CI — pending |
| Linux aarch64 | not measured | not measured |

The CI `checks` job writes the sizes for `ubuntu-latest` and `macos-latest`
(arm64) to the job summary. The repo has no GitHub remote yet. Fill in the
macOS row after the first CI run.

The `magika` build is about 19 MiB. Most of it is ONNX Runtime. The model is
3.2 MB. Start-up costs 1.4 ms with `magika` and 0.5 ms without it
(`teak --version`, hyperfine).

### No network crates (FR-10)

`cargo tree -e normal` on the default build lists 24 crates. None is an HTTP
client or a telemetry crate. `ureq` is in `Cargo.lock`, but only as a
build dependency of `ort-sys` (the download at build time). The CI job checks
this on each push.

## Magika

### Can Magika read through a custom reader? Yes

`magika` 1.1.0 has a public `SyncInput` trait (`length`, `read_at`) and
`Session::identify_content_sync(impl SyncInput)`. `teak::io::MagikaInput`
implements it, so every Magika read goes through the `io` gate and the
`test-hooks` counters see the real bytes. **We do not need the fallback**
(adding the read sizes from the model config).

For each file, Magika reads two blocks of `min(4096, size)` bytes: one at the
start and one at the end (`block_size = 4096` in the model config). Files below
8 bytes, and files that are only white space, get a rule result with no model
call. The test `hooks::magika_reads_are_counted` checks the count.

### Time per file

`benches/magika.rs`, 100 mixed files from `gen_tree` (seed 42):

| Case | Median | Per file |
|---|---|---|
| `session_new` (model load only) | 9.8 ms | — |
| `cold_100` (new session, then 100 files) | 186 ms | 1.86 ms |
| `warm_100` (session exists, 100 files one by one) | 175 ms | 1.75 ms |
| `warm_batch_100` (features, then one batch of 100) | 233 ms | 2.33 ms |
| `warm_par4_100` (4 sessions × 1 intra-op thread, 25 files each) | 173 ms | 1.73 ms |

Findings:

- Use one session and identify one file at a time. A batch is 33% slower.
- Four parallel sessions give no gain. Throughput stops at about 575 files per
  second. We do not know the cause yet (ONNX Runtime threads or a shared lock).
  Task-3 can look into it, but the targets do not need it.
- Make the session lazily: a run where every type comes from the cache does
  not pay the 9.8 ms model load.

## Walk

`benches/walk.rs`: an `ignore::WalkBuilder` walk with `standard_filters(false)`,
`follow_links(false)`, 16 threads, and one `lstat` per entry. Trees from
`gen_tree` (seed 42, depth 6, 5% of files with no extension):

| Tree | Entries | Data on disk | `stat` (walk only) | `arena` (walk + tree) | Peak RSS (`arena`) |
|---|---|---|---|---|---|
| 10k | 10,312 | — | 4.8 ms | 6.4 ms | 8.9 MB |
| 100k | 103,125 | 176 MB | 19.4 ms | 33.8 ms | 29.5 MB |
| 1M | 1,031,250 | 1.85 GB (4.8 GB used) | 164 ms | 358 ms | **242 MB** |

`arena` builds the tree as in design.md §Data Models: one `Node` per entry, all
names in one byte pool, and children sorted by name bytes. Peak RSS comes from
`TEAK_WALK_RSS=<case> cargo bench --bench walk` (`getrusage`, two runs; both
gave 242 MB). The tree is made by a child process, so it does not count.

What changed during the spike:

1. The first arena sorted all paths by component. It took 2.43 s on 1M files.
   Directory lookup with a hash map, plus a sort of each child list, takes
   358 ms (−85%).
2. Full absolute paths and a flat copy of the per-thread records gave a peak
   RSS of 372 MB, which is above the target. Paths relative to the root, and
   reading the per-thread records in place, give 242 MB.

Memory at 1M entries (estimate): walk records about 150 MB (a relative path of
about 50 bytes, plus 72 bytes of fields, plus the allocator overhead), `Node`
72 bytes × 1M = 72 MB, name pool 15 MB. The peak is during the arena build,
while the records and the nodes are both in memory.

## `scripts/bench.sh`

All five cases run, and each writes `target/bench/<case>.json`. The phase-0
binary is a stub that exits 2 for every scan, so the numbers show only that the
harness works and the start-up floor:

| Case | Command | Median | Note |
|---|---|---|---|
| `10k` | `teak <tree>` | 1.4 ms | stub, exit 2 |
| `100k` | `teak --detect full --no-cache <tree>` | 1.3 ms | stub, exit 2 |
| `1m` | `teak --no-cache <tree>` | 1.3 ms | stub, exit 2; peak RSS 7.1 MB |
| `magika100` | test-hooks build, `--detect full --no-cache` | 1.4 ms | no `detect` stage yet |
| `warmcache` | test-hooks build, `--detect full` | 1.3 ms | no `detect` stage yet; 0 bytes read |

Run with `BENCH_IGNORE_FAILURE=1 scripts/bench.sh all`. Task-2 and task-3
run it again without that variable, against the real pipeline.

## Go / no-go

### FR-71: 10k files, warm cache, < 150 ms — Go

Walk + arena 6.4 ms + start-up 1.4 ms ≈ 8 ms. That leaves about 140 ms for
aggregate, chaff, salience, select, cache load, and render. With a warm cache,
the visible rows need no Magika call. The session must be lazy (see Magika
findings); otherwise each run pays 9.8 ms.

### FR-72: 100k files, `--detect full --no-cache`, < 1 s — No-go as written

Walk + arena 34 ms + session 10 ms. Magika then costs 1.75 ms per file, and
that is the main cost. At 1 s, there is room for about 500 Magika calls, but
CI and laptops are slower than the reference machine.

The conflict is with FR-51. Its global cap is 5,000 files or 2 s. FR-50 samples
up to 3 files per extension in each folded directory, so a 100k tree with about
3,000 directories asks for thousands of samples. With Magika on each one,
detection runs until the 2 s time cap, and FR-72 fails.

**Plan A1 (please approve):** a Magika sub-cap inside the FR-51 cap.

- With `--detect full`, Magika types the visible rows first, then samples,
  up to **300 files** per run (about 525 ms on the reference machine, so 2×
  headroom).
- After that, the remaining samples use the `fast` path (magic bytes with
  `infer` on the head, then the extension). Phase 0 did not measure this path.
  It needs one small read per file and no model call, so we expect tens of
  microseconds per file with a warm OS cache. Task-3 measures it.
- The FR-51 cap (5,000 files or 2 s) stays as it is, for all content detection.
- requirements.md gets one new line under FR-51 or FR-53 for the sub-cap, and
  an acceptance test: with `--detect full` on 1,000 files that have no
  extension, at most 300 types come from Magika.

Rejected: lowering the FR-51 time cap to below 1 s. It would also cut `fast`
detection, which has no speed problem.

### FR-73: 1M files, `--no-cache`, < 5 s and RSS < 300 MB — Go, with a risk

Time: walk + arena 358 ms, far below 5 s. Memory: 242 MB for the walk and
tree only. Aggregates, chaff tags, and scores come on top, about 24 to 40 bytes
per node. So the full pipeline can come near 300 MB.

**Plan A2 (please approve):** task-2 keeps these spike changes, and adds two
more:

1. Each walk thread keeps only the name bytes and the parent directory's
   path id, not the full relative path. This saves about 60 MB at 1M.
2. Free the walk records before aggregate starts, and size per-node vectors
   (`ChaffTag`, scores) with indices, not `String`s. A `ChaffTag` gets a label
   id, not an `Option<String>`.

Task-2 checks the full pipeline with `scripts/bench.sh 1m` (peak RSS from
`/usr/bin/time`). If the RSS is still above 280 MB, it stops and reports to you
before it goes on.

### FR-74: Magika on about 100 visible files, < 300 ms — Go

Cold: 186 ms (session build included). Warm: 175 ms. The margin is 114 ms
(38%). Task-3 times the `detect` stage with `io::hooks::stage("detect")` and
runs `scripts/bench.sh magika100`.

### FR-75: warm cache, detect < 5% of total, 0 content reads — Go, with a condition

We cannot measure this until the cache exists (task-3). Zero reads follow
from the design: a cache entry stores the type label and the content-check
result (design.md §Cache stores check results). But the 5% rule is tight. The
total for 100k files will be about 60 to 100 ms, so 5% is 3 to 5 ms. Loading
`types.bin` for 100k entries (about 4.6 MB) can take that long by itself.

**Plan A3 (please approve):** task-3 loads `types.bin` on its own thread while
the walk runs, and times it as the `cache_load` stage, not as `detect`. Then
`detect` holds only the lookups for visible and sampled rows (hash lookups,
well below 1 ms). The FR-75 test checks the `detect` stage only.

## Plan B: Intel macOS

pyke has no prebuilt ONNX Runtime for `x86_64-apple-darwin`, so the default
build (with `magika`) fails on Intel Macs. The requirements list macOS x86_64
as a target.

**Plan B (please approve), recommended option first:**

1. **Ship Intel macOS without `magika`.** The release workflow (task-5) builds
   `x86_64-apple-darwin` with `--no-default-features`. `teak --version` prints
   `magika: off`, and `--detect full` acts as `fast` (FR-55 already covers
   this). For `cargo install` on an Intel Mac, a `build.rs` check or the README
   tells the user to add `--no-default-features`. Cost: small. Intel Macs get
   magic-byte and extension detection only.
2. Build ONNX Runtime from source for that target in CI, and point `ort` at it
   with `ORT_LIB_LOCATION`. Cost: a slow CI job (about 1 hour cold) and more
   upkeep.
3. Drop Intel macOS from v1. Cost: a change to the requirements.

## Open points

- macOS arm64 binary sizes: they come from the first CI run (the repo needs a
  GitHub remote).
- Linux aarch64: not built or measured. The prebuilt exists, and task-5 adds it
  to the release matrix.
- `gen_tree` trees are synthetic: the files are small and the names are evenly
  mixed. A check on one real large tree (for example a monorepo with
  `node_modules`) in task-2 would make the numbers more trustworthy.
- Node ids in the spike arena follow walk order, which changes between runs.
  Only the child lists are sorted. Task-2 must number the nodes in a fixed order
  (for example a preorder pass), because FR-65 needs the same output for every
  thread count.

## How to reproduce

```sh
cargo bench --bench walk                     # walk times (makes the trees on first run)
TEAK_WALK_RSS=1m cargo bench --bench walk    # peak RSS on 1M files
cargo bench --bench magika                   # Magika times
BENCH_IGNORE_FAILURE=1 scripts/bench.sh all  # hyperfine harness (stub binary)
cargo build --release && ls -l target/release/teak
cargo build --release --no-default-features --target-dir target/no-magika
```
