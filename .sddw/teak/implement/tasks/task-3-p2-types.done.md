# Task 3 Completion: Add P2 types — Magika, sampling, mismatches, type cache, and content chaff

## Summary
Added P2 type detection in four TDD slices, then tuned for FR-75. The slices: Magika on visible files with the mismatch mark and `--mismatches`; content checks with the 13 built-in templates and `--templates`, `--templates-only`, and `--dump-templates`; sampled type mixes of folded directories with the global cap and the Plan A1 Magika sub-cap; and the type cache with `--no-cache` (Plan A3). The detection corpus and all benchmarks pass on the reference machine.

## Commits
- `58e06e6` test(teak): add failing detection tests and the corpus (FR-49, FR-52, FR-53, FR-55)
- `d56f595` feat(teak): type visible files with Magika, mark and list mismatches (FR-49, FR-52, FR-53, FR-55)
- `9d347f9` test(teak): add failing content-chaff and template tests (FR-30, FR-33, FR-38, FR-39, FR-40, FR-41, FR-42)
- `594e996` feat(teak): add content checks and boilerplate templates (FR-30, FR-33, FR-38, FR-39, FR-40, FR-41, FR-42)
- `3a9afc0` test(teak): add failing sampling and cap tests (FR-50, FR-51)
- `2430c09` feat(teak): sample folded directories and cap content detection (FR-50, FR-51)
- `3fef072` test(teak): add failing type-cache tests (FR-54)
- `bd6a3d9` feat(teak): add the type cache and --no-cache (FR-54)
- `6844d8e` fix(teak): zero reads and a fast detect stage with a warm cache (FR-54, FR-75)
- `13a2083` chore(teak): update the bench notes for the type cache (FR-72, FR-73, FR-74, FR-75)

## Deviations
- **Rule 1: Bug** — Magika labels zero-padded PNG fixtures as `iso` → magic bytes that agree with the extension on a binary format win over the model.
- **Rule 1: Bug** — Magika labels a run of `x` (text) as `iso` → a binary Magika label on a head that looks like text falls back to the fast path.
- **Rule 1: Bug** — fast detection called every `#!` script `shell` (the `infer` text matcher) → the shebang interpreter decides.
- **Rule 1: Bug** — binary files, and visible files with a name class, never get a check result, so they missed the cache on every run (found in `bench.sh warmcache`) → only text files that the checks look at need a cached check result.
- **Rule 1: Bug (tests)** — the marker fixture `m0.txt`…`m5.txt` formed a series → `support::word` names. `support::sized` wrote one long line, which FR-39 calls minified → it now writes 64-byte lines.
- **Rule 2: Missing Critical** — files that are boilerplate by name (`CODE_OF_CONDUCT.md`, `.gitignore`) fold before detection, so no template could ever label them → detection also types the boilerplate files of visible chaff lines; the template check adds the label and keeps the class. FR-42 still holds.
- **Rule 2: Missing Critical** — the cache header also has a label-table hash and a template-table hash, and a record has source, mode, and flag bytes (48 bytes) → a changed label table drops the file; a changed template table drops only the check results.
- **Interface (internal, not architectural)** — `teak::build(root, opts, &mut Context, budget, scan_time)`; `Context::new(opts, env) -> Result<_, UsageError>` holds the detector, templates, and cache; `Env::cache_dir`; `detect::run(view, tree, tags, labels, det, cap, checks_on) -> Detected`; `Classifier::new(tree, opts, templates)`; `classify_content(tree, id, head, size)`; `sample::plan(tree, folded, k, skip) -> Plan` (bucket ids); new `select::footer`, `select::apply_content`, `select::mismatch_view`; new `RowKind::{Mismatch, More, Note}`; `Scan::{estimated, template_labels}`; `src/fnv.rs` (an FNV hasher; not in the design's crate layout).
- **Spec choices (for verify)** — `⚠` comes only from a sure Magika label (score ≥ 0.90), so fast mode never shows it. `detect::same_type` adds families: `.txt` accepts any text, js/ts, c/cpp, json/jsonl, yaml/json, ini/toml, and zip containers. An unsure Magika label (< 0.90) loses to a known extension. `--mismatches` uses Magika for every file within the global cap. `XDG_CACHE_HOME` counts on macOS too. A cache hit counts toward the 5,000-file cap, so warm and cold runs take the same samples. Mixes with an estimate show a `≈ ` prefix, also on uniform rows. Content checks run only for text files.
- **Requirements change** — FR-51 got the approved Plan A1 sub-cap line and a "Magika sub-cap" scenario (in `requirements.md`, which is not under git).
- **Spec gap (for verify)** — FR-49's "`--why script` names `magika`" needs `--why` (task-4). The test checks `Source::Magika` through the library.

## Difficulties
- `perf` is blocked here (`perf_event_paranoid` = 4) → temporary sub-stage timers. The warm detect stage went from 15.8 ms to 2.5 ms: hits count toward the cap, bucket ids instead of hash maps, no sort, a name-rule quick reject, FNV maps, and fewer clock reads.
- Magika gives odd labels on synthetic content (padded binaries, uniform text, short Rust files at a 0.44 score) → the fallbacks above. The corpus uses realistic files, all checked with a probe first.
- Tests must not share a type cache (inode reuse with fixed mtimes) → each test run gets its own `XDG_CACHE_HOME` below `CARGO_TARGET_TMPDIR`.

## Notes
- Benchmarks (reference machine, hyperfine medians): 10k 13.8 ms (FR-71); 100k `--detect full --no-cache` 657 ms (FR-72); 1M `--no-cache` 1.10 s and 172.7 MB peak RSS (FR-73); magika100 detect stage 195 ms (FR-74); warmcache detect stage 2.49 ms of 57.0 ms (4.4%) with 0 content reads (FR-75).
- Tests pass with default features, `--features test-hooks`, and `--no-default-features`; clippy is clean in all three. macOS CI has not run (the repo has no GitHub remote).
- `src/detect/secret.rs` shows `no preview (secret)` on teak's own repo: its source holds the private-key header pattern. This is the FR-77 content rule from task 2; verify can decide whether it needs a fix.
- Template texts: licenses from choosealicense.com, `.gitignore` files from github/gitignore, and the Contributor Covenant 2.1. A `.gitignore` matches only when it is the unchanged GitHub template.
- For task-4: `--why` can use `TypeInfo::source` and `Scan::estimated`. Previews should read through `io` and stay off with `--detect off`. The cache record has no preview data.
