# Task 4 Completion: Add P3 context — previews, git status, focus, why, stats, project kinds, long view, duplicates

## Summary
Added P3 in four TDD slices: git status with `gix`, project kinds, and the P3 header; per-type previews; `--focus`, `--why`, and `--stats`; and `-l` with `--dupes`. The FR-12 property now runs with `-l` and `--focus`, and a new snapshot covers every row kind with git and preview cells.

## Commits
- `2bdd9d1` test(teak): add failing git status, project kind, and P3 header tests (FR-19, FR-57, FR-59, FR-76)
- `22406c5` feat(teak): add git status, project kinds, and the P3 header (FR-19, FR-57, FR-59, FR-76)
- `327efd0` test(teak): add failing preview tests (FR-56, FR-77)
- `2c367a9` feat(teak): add per-type previews (FR-56, FR-77)
- `7b10e57` test(teak): add failing focus, --why, and --stats tests (FR-18, FR-60, FR-61)
- `6f12dec` feat(teak): add --focus, --why, and --stats (FR-18, FR-60, FR-61)
- `ff87666` test(teak): add failing long-view, duplicate, and FR-12 flag tests (FR-12, FR-24, FR-43)
- `d30aa81` feat(teak): add the long view and --dupes (FR-12, FR-24, FR-43)

## Deviations
- **Rule 1: Bug** — gitignored chaff (`target/` as `build`) counted as untracked, because the first matching source wins and the tag is `build`, not `ignored` → `Classifier::classify_names_and_ignored` gives gitignore bits apart from the chaff class. The bits load also with `-a` and `--no-gitignore`.
- **Rule 1: Bug** — a long `--preview full` text dropped the whole preview column → the column plan counts a preview as at most 40 columns; the line cut ends it.
- **Rule 1: Bug** — after the focus path reorders a directory, a queued reveal could point at the wrong child and index past the end → stale reveal items go back with the ratio of the real next child.
- **Rule 3: Blocking** — `Opts` outgrew clippy's `large_enum_variant` limit → `Parsed::Run(Box<Opts>)`; `tests/support` and `benches/scan.rs` follow.
- **Design change (approved in the plan)** — the zip entry count comes from the end-of-directory record (and the zip64 record) that teak reads itself; the `zip` crate is not used.
- **Data model (approved in the plan)** — copy → original is `Scan::dupes`, not a `ChaffTag::dup_of` field, so tags stay small on 1M-node trees.
- **Choice (approved)** — on a type-cache hit, the preview reads the head itself (`io::read_preview`); the test report counts these reads under `preview_reads`, apart from `reads`. FR-54 and FR-75 "zero content-detection reads" still hold.
- **Interface (internal, not architectural)** — `build()` returns `BuildError::{Scan, Usage}`; new `Scan` fields `kinds`, `git`, `git_head`, `previews`, `why`, `dupes`; `Detected::{heads, cache_hits}`, `Detector::take_head`; `io::{read_at, read_each, read_preview}`; `Classifier::{classify_names_and_ignored, why}`; `ScoreInputs::{git, focus}`, `Terms::focus`, `salience::terms`; `SelectOpts::focus`; `LayoutOpts::long`; `Glyphs::{ahead, behind}`; `layout::{stats, series_name, mode_string, user_name, group_name}`; `Tree::find_bytes`; `TypeGroup::name`; `explain::{resolve, find, why}`; `git::{open, GitRepo, GitStatus, GitHead, Code}`; `kind::{detect, head_has_exif}`; `dupes::find`; `preview::of`.
- **Spec choices (for verify)**
  - Git status covers the whole tree, not only the visible directories: salience needs the git term before selection. The stat compare is cheap; hashing (through `io`) runs only after a stat change or for a racy entry.
  - `D` (deleted) has no row; it counts in its directory's `M<n>` and in the header. A staged change shows `M`, a staged new file `A`.
  - Only the repo that holds the scan root counts; repos nested inside the root get no status.
  - `git repo` (the `.git/` kind) shows only when git is on and teak cannot read the head. With `--no-git` the header has no git data (FR-57 "off").
  - The focus boost is 1 on the path and for the focus's direct children, then halves per step. A focus inside a folded chaff directory does not force it open.
  - `photos` looks at the root's own files only, and reads at most 16 image heads (4 KiB each). Not with `--detect off`.
  - The symlink target stays in the flags column (P1), so `--preview off` still shows `⇢ target`.
  - "Other binary: the Magika description" has no preview: teak keeps no Magika descriptions.
  - `--ascii` turns `→ × ≈ …` in previews into `-> x ~ ...`.
  - `--stats` counts all files that are not hidden (folded chaff too), with name labels and the sample estimates.
  - `--dupes` runs with `--detect off` too (it is an explicit flag). Copies inside folded or hidden directories are not checked.
- **Spec gap (for verify)** — FR-49's "`--why script` names `magika`": `--why` prints class, action, salience, and decision (the task contract); it does not print the type source.

## Difficulties
- The new git snapshot counted `.git/` files, and that count depends on the git version (template hooks) → the fixture uses `git init --template=`.
- A uniform directory folded the image series, so the snapshot had no series row → the series moved to the root.
- The RED run of the props test left a `tests/props.proptest-regressions` seed (`-l` was an unknown flag then) → deleted; it was not a real failure.
- `gix` with only `index`, `revision`, and `sha1` still pulls `gix-command` and `gix-transport` (no HTTP client). `Options::isolated()` keeps gix from running `git` to find config; the strace test confirms one `execve`.

## Notes
- Benchmarks (reference machine, hyperfine): 10k 13.7 ms (FR-71, was 13.8 ms); warmcache total 63.4 ms (was 57.0 ms), detect stage 2.40 ms (3.8%), 0 content reads (FR-75). The bench trees live in `target/` inside teak's repo, so git status runs for them (all ignored). Most of the +6 ms is that pass and the preview re-reads. FR-72–FR-74 were not run again.
- `PROPTEST_CASES=300` passes for FR-12/FR-21 with `-l` and `--focus`.
- Tests pass with default features, `--features test-hooks`, and `--no-default-features`; clippy is clean; `cargo tree -e normal` has no network crate. macOS CI has not run (the repo has no GitHub remote).
- Tests need `git` on the PATH (fixtures only; teak itself never runs it).
- On teak's own repo, the changed count (25) matches `git status -uall`.
