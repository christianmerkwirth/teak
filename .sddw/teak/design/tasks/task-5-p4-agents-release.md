# Task 5: Add P4 agents and release — Markdown, JSON, config, completions, man page, release pipeline

## Trace
- **FR-IDs:** FR-63, FR-64, FR-65, FR-66, FR-67, FR-68, FR-69
- **Depends on:** task-4
- **Design:** ../design.md

## Files
- `Cargo.toml` — modify (crates.io metadata; `[workspace.metadata.dist]` for cargo-dist; `toml` dependency)
- `src/cli.rs` — modify
- `src/lib.rs` — modify
- `src/config.rs` — create
- `src/salience.rs` — modify (weights from config)
- `src/chaff/rules.rs` — modify (user chaff rules from config)
- `src/render/mod.rs` — modify
- `src/render/md.rs` — create
- `src/render/json.rs` — create
- `schema/teak.schema.json` — create
- `examples/gen_assets.rs` — create (writes completions and `teak.1` from the clap definitions)
- `.github/workflows/release.yml` — create (cargo-dist)
- `CHANGELOG.md` — create
- `README.md` — create
- `tests/formats.rs` — create
- `tests/stable.rs` — create
- `tests/config.rs` — create
- `tests/release.rs` — create
- `tests/props.rs` — update (interface change: FR-12 in `md` and `json`; FR-21 through JSON)
- `tests/chaff.rs` — update (interface change: FR-30 class check through `--format json`)
- `tests/safety.rs` — update (interface change: FR-77 in `md`, `json`, and `--json-full`)

## Design References
- design.md §Architecture (Components: `render`, `config`; Data Flow, config layers)
- design.md §Data Models (Row, View; JSON output)
- design.md §Interface Contracts (CLI; `render::write`; `config::load`)
- design.md §Design Decisions (Deterministic tree)
- design.md §Design Decisions (Salience defaults)
- design.md §Design Decisions (Secret guard)
- design.md §Design Decisions (Dependencies)

## Contracts (task-specific)

### CLI flags added in this task
- `--format md|json` (next to `tty` and `plain`), `--json-full`.

### Markdown format
- Line 1: `# <root>/ — <size>, <n> files`. Line 2: kind, git, changed count, and `scanned <YYYY-MM-DD HH:MM>` (scan time, UTC).
- The tree: the `plain` rows in ASCII, in a fenced `text` block. The fence is one backtick longer than the longest backtick run in the tree, and at least 3.
- Then the type-group table (`| Group | Files | Size |`) and the line `Folded: <class> <n> · … . Run `teak -a` to show all.`
- The budget applies to the lines inside the fenced block.

### JSON format
- `{"schema_version": 1, "root", "scan_time", "header", "rows": [...], "footer", "types": [...]}`. Each row has `kind` (`entry`, `overflow`, `series`, `summary`, `chaff`), `path`, `depth`, `size`, `files`, `dirs`, `type`, `type_mix`, `estimated`, `class`, `flags`, and the optional `git`, `preview`, `mismatch`, `dup_of`.
- `--json-full` adds `entries: [...]` with one object per node (`path`, `kind`, `size`, `mtime`, `mode`, `class`, `type` when known, `visible_in`).
- Paths that are not UTF-8 use the same `\xHH` escape as text output. Object keys are in a fixed order; arrays follow the display order.
- The budget applies to the number of objects in `rows`.

### Config
- Files: `~/.config/teak/config.toml` (or `$XDG_CONFIG_HOME/teak/config.toml`) and `.teak.toml` in the root or its nearest parent. Order: defaults ← user file ← project file ← flags.
- Keys: `lines`, `depth`, `sort`, `series_min`, `detect`, `preview`, `color`, `width`, `time`, `format`, `templates = [..]`, `[chaff] show/fold/hide = [..]`, `[[chaff.rules]] pattern, class`, `[weights] size, recency, git, rarity, role, anomaly`.
- A TOML syntax error → name the file and the line, exit 2. An unknown key → one warning on stderr, then continue.

### Internal Interfaces
- `config::load(root: &Path) -> Result<ConfigLayers, ConfigError>`; `ConfigLayers::merge(self, cli: &CliArgs) -> Opts`.
  - Post: a flag the user did not type does not override a config value (clap values are `Option`s).
- `render::md::write(view, style, out)`; `render::json::write(view, tree, full: bool, out)`.
- `examples/gen_assets.rs`: `cargo run --example gen_assets -- <out-dir>` writes `teak.bash`, `_teak`, `teak.fish`, and `teak.1`.

## Acceptance Criteria

**Shared test conditions.** Unless a scenario says otherwise:

- Tests run as a normal user, not as root.
- The output format is `plain`, with `--width 120` and `--color never`.
- "Fixture" means a tree that the test suite builds in a temp directory.
- `SOURCE_DATE_EPOCH` sets the scan time, so relative ages are fixed.
- "Reference machine" means the machine that the phase-0 report names.

### FR-63: Markdown

**Happy path:**
- WHEN the user runs `teak --format md`
- THEN the first line SHALL be `# <root>/ — <size>, <n> files`
- AND the tree SHALL be in a fenced `text` block, in ASCII
- AND the output SHALL have the type table and the line `Folded: …`
- AND a CommonMark parser SHALL parse the output with all fences closed

**Edge case:**
- GIVEN a file name that contains three backticks
- THEN the fence SHALL be longer than any run of backticks in the tree, so the block stays closed

### FR-64: JSON

**Happy path:**
- WHEN the user runs `teak --format json`
- THEN the output SHALL be valid JSON with a top-level `schema_version`
- AND it SHALL pass validation against the JSON Schema in the repo
- AND it SHALL hold only the visible rows and the summaries
- WHEN the user runs `teak --json-full`
- THEN the number of file entries SHALL equal the file count from an independent walk

### FR-65: Stable output

**Happy path:**
- GIVEN a fixed fixture and a fixed `SOURCE_DATE_EPOCH`
- WHEN the user runs teak two times with `-j 1` and two times with `-j 8`, in `plain`, `md`, and `json`
- THEN the four outputs of each format SHALL be byte-identical

### FR-66: Config files

**Happy path:**
- GIVEN `~/.config/teak/config.toml` with `lines = 50` and `.teak.toml` at the root with `lines = 40`
- WHEN the user runs `teak -n 30`
- THEN the budget SHALL be 30
- WHEN the user runs `teak`
- THEN the budget SHALL be 40
- GIVEN there is no `.teak.toml`
- WHEN the user runs `teak`
- THEN the budget SHALL be 50

**Edge case:**
- GIVEN `.teak.toml` is in a parent directory of the root
- THEN teak SHALL find and use it

**Failure paths:**
- GIVEN a config file with a TOML syntax error
- THEN teak SHALL name the file and the line, and exit with code 2
- GIVEN a config file with an unknown key
- THEN teak SHALL print one warning and continue

### FR-67: Salience weights

**Happy path:**
- GIVEN the fixture from FR-13, and a config with size weight 0 and recency weight 1
- AND `a/` is changed to the newest mtime, and `z/` to an mtime 3 years ago
- WHEN the user runs `teak -n 12`
- THEN `a/` SHALL open before `z/`

### FR-68: Install

**Happy path:**
- GIVEN the release pipeline
- WHEN it builds for Linux x86_64, Linux aarch64, macOS x86_64, and macOS arm64
- THEN each target SHALL produce a binary that prints its version with `teak --version`
- AND `cargo install teak` (from the package) SHALL work on Linux and macOS CI
- AND `brew install <tap>/teak` SHALL work on macOS CI

### FR-69: Completions and man page

**Happy path:**
- GIVEN the release archive
- THEN it SHALL contain bash, zsh, and fish completions and `teak.1`
- AND `man ./teak.1` SHALL render with no error
- AND bash completion for `--fo` SHALL offer `--focus`, `--fold`, `--follow`, and `--format`

## Done Criteria
- [ ] `tests/formats.rs` passes for FR-63 (CommonMark parse with `pulldown-cmark`, all fences closed, backtick edge case) and FR-64 (validation against `schema/teak.schema.json` with `jsonschema`; `--json-full` count equals an independent walk)
- [ ] `tests/stable.rs` passes for FR-65: two runs with `-j 1` and two with `-j 8` give byte-identical `plain`, `md`, and `json`
- [ ] `tests/props.rs`: FR-12 holds for the `md` tree block and for `json` rows; FR-21 holds through `--format json`
- [ ] `tests/chaff.rs` checks the FR-30 classes through `--format json`
- [ ] `tests/safety.rs` passes FR-77 in `md`, `json`, and `--json-full`
- [ ] `tests/config.rs` passes for FR-66 (all layers, parent `.teak.toml`, syntax error, unknown key) and FR-67
- [ ] `tests/release.rs` passes for FR-69: `gen_assets` writes all four files; `man ./teak.1` renders with no error; bash completion for `--fo` offers `--focus`, `--fold`, `--follow`, and `--format`
- [ ] The cargo-dist release workflow builds Linux x86_64, Linux aarch64, macOS x86_64, and macOS arm64; each binary prints `teak --version` (FR-68)
- [ ] The release archive holds the binary, the three completions, and `teak.1`
- [ ] `cargo install --path .` works on Linux and macOS CI, and `cargo publish --dry-run` passes
- [ ] The Homebrew tap formula is generated by cargo-dist, and `brew install <tap>/teak` works on macOS CI
- [ ] `CHANGELOG.md` records JSON `schema_version: 1` and the `plain`/`md` format as v1
- [ ] All earlier tests still pass; `cargo clippy --all-targets -- -D warnings` and `cargo test` pass on Linux and macOS CI
