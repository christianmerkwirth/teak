# Task 5 Completion: Add P4 agents and release — Markdown, JSON, config, completions, man page, release pipeline

## Summary
Added P4 in three TDD slices: the `md` and `json` formats with `--json-full` and `schema/teak.schema.json`; config files (user and project) with chaff rules and salience weights; and the release: completions and the man page from clap, cargo-dist archives for four targets, a Homebrew tap, and CI install checks. Intel macOS builds without Magika through `build.rs` (Plan B, approved).

## Commits
- `f4736af` test(teak): add failing Markdown, JSON, and stable-output tests (FR-12, FR-21, FR-30, FR-63, FR-64, FR-65, FR-77)
- `17c4055` feat(teak): add the md and json formats and the JSON schema (FR-12, FR-21, FR-30, FR-63, FR-64, FR-65, FR-77)
- `671cb4b` test(teak): add failing config-file and weight tests (FR-66, FR-67)
- `a1450dd` feat(teak): add config files and salience weights (FR-31, FR-66, FR-67)
- `76b644e` test(teak): add failing completion, man page, and release-config tests (FR-68, FR-69)
- `81972c8` feat(teak): add completions, the man page, and the release pipeline (FR-68, FR-69)

## Deviations
- **Plan B (approved by the user)** — no prebuilt ONNX Runtime for `x86_64-apple-darwin` → `build.rs` sets `cfg(magika)` only when the feature is on and the target is not Intel macOS; `magika`/`ort` moved to a target-specific dependency table; all `cfg(feature = "magika")` became `cfg(magika)`. `cargo install teak` works on Intel macOS with no flag and prints `magika: off`.
- **Rule 3: Blocking** — cargo-dist can only add files that exist in the repo → the completions and `teak.1` are committed in `assets/` (not in the task's file list); `tests/release.rs` fails when they are stale.
- **Rule 3: Blocking** — a release made with `GITHUB_TOKEN` does not trigger other workflows → the post-release checks (`--version` per target, `brew install`) are a dist `post-announce-jobs` reusable workflow, `.github/workflows/release-check.yml` (new file). `ci.yml` (not in the file list) gets the `install` job: `cargo install --path .` and `cargo publish --dry-run` on Linux x86_64/aarch64 and macOS arm64/Intel, plus clippy without `test-hooks`.
- **Rule 3: Blocking** — `cargo install cargo-dist` failed: `aws-lc-sys` rejects this machine's gcc (memcmp bug 95189) → installed the official prebuilt `dist` 0.32.0 into `~/.cargo/bin` after a SHA-256 check.
- **Rule 1: Bug** — `drop(_t)` on the no-op layout timer failed `cargo clippy` without `test-hooks` (CI only ran clippy with it) → the timer is block-scoped.
- **Rule 2: Missing Critical** — tests read the user's real config → `tests/support` gives every run an empty `XDG_CONFIG_HOME`.
- **Interface (internal)** — `cli::parse_args -> Args::{Version, Run(Box<CliArgs>)}`, `cli::command()`; `parse_from` stays (defaults only). Clap values with defaults are `Option`s; the defaults show as `[default: …]` in the help text. `Opts::{json_full, weights}`; `ChaffOpts::rules`; `chaff::rules::{UserRule, UserRules}`; `config::{load, load_files, parse, resolve, user_file, project_file, ConfigLayers, Layer, ConfigError}`; `render::md::{write, write_stats}`, `render::json::{write, document, SCHEMA_VERSION}`; `layout::{mix, series_counts, type_groups, git_cell, root_totals, files_text}` are shared functions now. `serde_json` is a normal dependency (`preserve_order`); `toml` is new.
- **Spec choices (for verify)**
  - `md`: the header and footer rows of the view are the lines around the fence, so the fence holds at most `budget − 2` lines. Hidden classes follow the folded ones: `Folded: … . Hidden: … . Run …`; with none, `Folded: none.` The root name in the heading has Markdown escapes. `md` and `json` never have colors.
  - JSON: paths have a `/` after a directory and `./` for the root; times are epoch seconds; `scan_time` is an integer. Open directories carry the flag `open` and their total `files`. Extra keys beyond the contract: `mtime`, `target` (symlinks), `items` (chaff rows, with each item's class, so FR-30 can be checked), and the row kinds `mismatch` and `more` (the `--mismatches` view). Group names are `audio/video` and `other`, as in the text output.
  - Several roots in `json` give an array of documents. `--why` stays plain text in every format. `--stats --format json` has empty `rows`; `--stats --format md` has no fence.
  - Config: `templates` paths are relative to the config file and add to `--templates`. Within a file, `[chaff]` applies show, then fold, then hide; CLI overrides come after both files. Project rules come before user rules. A bad value names the file and the key (no line: `toml::Table` has no spans); a syntax error names the file and the line.
  - The `.teak.toml` and user-file reads go through `io::read_rule_file`: config is not content, so FR-52's read counter does not count it.
- **Placeholders** — `OWNER` in `repository`, `homepage`, the tap (`OWNER/homebrew-tap`), the README, and the schema `$id`. `release-check.yml` uses `github.repository_owner` for the tap.

## Difficulties
- Three RED tests had wrong expectations: the fake `.git/` gives the `git repo` kind (task-4 rule); a uniform `img/` folds its series (the series moved to the root, as in task 4); the file mode depends on the umask (the test sets 0644) → fixed in the tests, noted in the feat commit.
- The FR-67 scenario ("`a/` opens before `z/`") holds with the default weights too, because opening `a/` costs one line → the test also checks the `--why` scores: with `size = 0, recency = 1`, `z/` drops from 0.31 to below 0.1 and `a/` is above 0.5 (0.79). The first bound, 0.8, came from a manual run with a 0 s age.
- A full Intel macOS cross-check needs an Apple C toolchain (zstd-sys) → checked the dependency graph instead: `cargo tree --target x86_64-apple-darwin` has no `magika`/`ort`; `aarch64-apple-darwin` keeps them. The added rust-std target was removed again.

## Notes
- Tests pass with default features, `--features test-hooks`, and `--no-default-features`; clippy is clean in all three; `PROPTEST_CASES=300` passes for FR-12/FR-21 in text, `md`, and `json`.
- `cargo publish --dry-run` passes (64 files; tests, corpus, and benches stay out). `cargo install --locked --path .` works on Linux. `dist plan` lists four archives, each with the binary, the three completions, `teak.1`, README, and LICENSE.
- Not run here (no GitHub remote): the release workflow, macOS CI, `brew install`, and the aarch64 Linux build. These are FR-68 done criteria that verify must check after the first push and tag.
- `LICENSE` is untracked. dist puts it into the archives, so it must be committed before the first release. `include` in Cargo.toml leaves it out of the crate.
- After a CLI change, run `cargo run --example gen_assets -- assets` (the release test fails until then).
- Publish (2026-10-25): the `OWNER` placeholders became `christianmerkwirth` (tap `christianmerkwirth/homebrew-tap`), and the repo went public as `christianmerkwirth/teak`.
- Release prep (2026-10-25): the user dropped Homebrew for v0.1.0 (FR-68's `brew install` criterion is out). The dist config has only the shell installer; `release-check.yml` runs the installer script on the four OSes instead of `brew install`. crates.io publishing waits too.
- CI fixes (2026-10-25): the type cache guard now compares the real cache path (macOS `/var` → `/private/var` wrote the cache inside the tree); the thread-count property test uses `--detect fast` (the 2 s detection cap is a wall-clock limit); the test job runs `cargo fetch` first.
