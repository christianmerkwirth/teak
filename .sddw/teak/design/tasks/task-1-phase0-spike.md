# Task 1: Build the phase-0 spike and write the phase-0 report

## Trace
- **FR-IDs:** FR-71, FR-72, FR-73, FR-74, FR-75 (baseline measurements only; task-2 and task-3 meet the targets)
- **Depends on:** none
- **Design:** ../design.md

## Files
- `Cargo.toml` — create
- `.gitignore` — create
- `src/main.rs` — create
- `src/lib.rs` — create
- `src/io.rs` — create
- `examples/gen_tree.rs` — create
- `benches/walk.rs` — create
- `benches/magika.rs` — create
- `scripts/bench.sh` — create
- `tests/support/mod.rs` — create
- `tests/smoke.rs` — create
- `.github/workflows/ci.yml` — create
- `docs/phase0-report.md` — create

## Design References
- design.md §Architecture (Crate layout; `io` component)
- design.md §Design Decisions (One crate, library + binary)
- design.md §Design Decisions (Magika as an optional feature)
- design.md §Design Decisions (Test hooks behind a cargo feature)
- design.md §Design Decisions (Content reads: Magika reads; checks re-read)
- design.md §Design Decisions (Dependencies)

## Contracts (task-specific)

### CLI
- `teak --version` prints one line: `teak <semver> (magika: on, model <model-name>)`, or `teak <semver> (magika: off)` in a build without the `magika` feature. Exit code 0.
- Every other call prints `teak: not implemented yet` to stderr and exits with code 2. Task-2 replaces this.

### Internal Interfaces
- `teak::io::read_head(path: &Path, max: usize) -> io::Result<Vec<u8>>`: as in design.md §Interface Contracts.
  - Post: under `test-hooks`, adds the bytes to the read counter for `path` and sleeps `TEAK_TEST_READ_DELAY_MS` ms before it returns.
- `teak::io::hooks::stage(name: &'static str) -> StageTimer` (only `test-hooks`): a guard that adds its elapsed time to `stages_ms[name]` when it drops.
- `teak::io::hooks::write_report()` (only `test-hooks`): when `TEAK_TEST_REPORT` is set, writes `{"reads": {"<path>": <bytes>}, "stages_ms": {"<stage>": <ms>}}` to that file at exit.
- `examples/gen_tree.rs`: `cargo run --release --example gen_tree -- --files N --seed S --out DIR [--depth D] [--no-ext-share P]`.
  - Post: the same `N` and `S` give the same tree (paths, sizes, mtimes, and content bytes). The mix holds source files, images, text, data series, and files with no extension.
- `tests/support/mod.rs`:
  - `Fixture::new() -> Fixture` (a temp dir); `.file(path, bytes)`, `.dir(path)`, `.symlink(path, target)`, `.mode(path, mode)`, `.mtime(path, secs_before_scan)`, `.path() -> &Path`
  - `teak(fixture, args) -> Output`: runs the binary with `SOURCE_DATE_EPOCH` set to a fixed value. Task-2 adds the default flags `--format plain --width 120 --color never`.
  - `read_report(path) -> Report` (only `test-hooks`): parses the `TEAK_TEST_REPORT` file.
- `scripts/bench.sh <case>`: runs `hyperfine --warmup 3 --runs 10` for the cases `10k`, `100k`, `1m`, `magika100`, and `warmcache`. Writes JSON results to `target/bench/<case>.json`.

## Acceptance Criteria

**Shared test conditions.** Unless a scenario says otherwise:

- Tests run as a normal user, not as root.
- The output format is `plain`, with `--width 120` and `--color never`.
- "Fixture" means a tree that the test suite builds in a temp directory.
- `SOURCE_DATE_EPOCH` sets the scan time, so relative ages are fixed.
- "Reference machine" means the machine that the phase-0 report names.

Performance cases: For all of these: the fixture generator uses a fixed seed. Each case runs with
`hyperfine` (3 warmup runs, then 10 runs). The test checks the median on the
reference machine. Until the phase-0 report, a failed target is a warning, not a
failure.

### FR-71: 10k files

- GIVEN a generated tree with 10,000 files and a warm teak cache
- WHEN the test runs `teak`
- THEN the median time SHOULD be less than 150 ms

### FR-72: 100k files

- GIVEN a generated tree with 100,000 files and a warm OS cache
- WHEN the test runs `teak --detect full --no-cache`
- THEN the median time SHOULD be less than 1 s

### FR-73: 1M files

- GIVEN a generated tree with 1,000,000 files and a warm OS cache
- WHEN the test runs `teak --no-cache`
- THEN the median time SHOULD be less than 5 s
- AND the peak RSS SHOULD be less than 300 MB

### FR-74: Magika on visible rows

- GIVEN a tree in which about 100 files are visible
- WHEN the test measures only the detection stage (from the timing output of the test build)
- THEN it SHOULD take less than 300 ms

### FR-75: Warm type cache

- GIVEN the FR-72 tree and a teak cache from a previous run
- WHEN the test runs `teak --detect full`
- THEN the detection stage SHOULD take less than 5% of the total time
- AND the read counter for content detection SHOULD be 0

## Done Criteria
- [ ] `cargo build --release` works on Linux x86_64 with the `magika` feature, and `cargo build --release --no-default-features` works without it
- [ ] `.github/workflows/ci.yml` builds and runs `cargo test` on `ubuntu-latest` and `macos-latest`: default features, `--no-default-features`, and `--features test-hooks`
- [ ] `tests/smoke.rs`: `teak --version` prints `magika: on` in the default build and `magika: off` without the feature
- [ ] `tests/smoke.rs`: under `test-hooks`, `read_head` on a 10 KiB file with `max = 4096` records 4096 bytes in the report, and `TEAK_TEST_READ_DELAY_MS=10` adds at least 10 ms per read
- [ ] `tests/smoke.rs`: two `gen_tree` runs with the same seed give the same hash over all paths, sizes, mtimes, and content
- [ ] `benches/walk.rs` measures an `ignore` walk with `standard_filters(false)` on 10k, 100k, and 1M generated files
- [ ] `benches/magika.rs` measures Magika time per file on about 100 mixed files, with a cold and a warm session
- [ ] `cargo tree -e normal` on the default build shows no HTTP client or telemetry crate
- [ ] `docs/phase0-report.md` names the reference machine (CPU, RAM, disk, OS)
- [ ] The report records the ONNX Runtime link method (static or bundled) and the release binary size with and without `magika`, on Linux and macOS
- [ ] The report records Magika ms per file, walk time and peak RSS on 1M files, and the `scripts/bench.sh` results
- [ ] The report says whether the `magika` crate can read through a custom reader (so `io` counts its reads), or records the fallback (the hook adds the read sizes from the model config)
- [ ] The report gives a go or no-go for each of FR-71 to FR-75; each no-go has a written plan for the user to approve
