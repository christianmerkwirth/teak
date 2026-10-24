# Task 4: Add P3 context — previews, git status, focus, why, stats, project kinds, long view, duplicates

## Trace
- **FR-IDs:** FR-18, FR-19 (P3 part), FR-24, FR-43, FR-56, FR-57, FR-59, FR-60, FR-61, FR-76 (git part)
- **Depends on:** task-3
- **Design:** ../design.md

## Files
- `Cargo.toml` — modify (add `gix`, `imagesize`, `flate2`, `zstd`, `xz2`, `zip`, `uzers`)
- `src/cli.rs` — modify
- `src/lib.rs` — modify
- `src/preview.rs` — create
- `src/git.rs` — create
- `src/kind.rs` — create
- `src/explain.rs` — create
- `src/dupes.rs` — create
- `src/salience.rs` — modify (git and focus signals)
- `src/select.rs` — modify (focus path, `--why` hooks)
- `src/view.rs` — modify (git and kind in the header; preview and git cells)
- `src/layout.rs` — modify (`-l` columns; preview and git columns; drop order)
- `src/render/text.rs` — modify (`--stats`, `--why`)
- `tests/preview.rs` — create
- `tests/git.rs` — create
- `tests/focus.rs` — create
- `tests/why.rs` — create
- `tests/stats.rs` — create
- `tests/kind.rs` — create
- `tests/long.rs` — create
- `tests/dupes.rs` — create
- `tests/safety.rs` — update (interface change: FR-76 with git status on; FR-77 with real previews)
- `tests/budget.rs` — update (interface change: FR-19 P3 header)
- `tests/props.rs` — update (interface change: add `-l` and `--focus` to the FR-12 flag set)
- `tests/snapshots/` — update (interface change: header kind and git, preview and git columns)

## Design References
- design.md §Architecture (Components: `preview`, `git`, `kind`, `explain`, `dupes`, `salience`, `select`, `layout`)
- design.md §Data Models (Row: Header with `kind` and `git`; ChaffTag `dup_of`)
- design.md §Interface Contracts (`preview::of`; `git::open`, `GitRepo::status`, `GitRepo::head`; `explain::why`)
- design.md §Design Decisions (Git status: gix, stat-only, no filters)
- design.md §Design Decisions (Secret guard)
- design.md §Design Decisions (Salience defaults — focus boost)
- design.md §Design Decisions (Budget accounting)
- design.md §Design Decisions (No content reads with `--detect off`)

## Contracts (task-specific)

### CLI flags added in this task
- `--preview off|short|full`: `short` (the default) cuts the preview to 40 columns; `full` uses all the width that is left.
- `--no-git`, `--focus PATH`, `--why PATH`, `--stats`, `-l/--long`, `--dupes`.
- `--focus` and `--why` resolve PATH against the current directory; a path outside the root → exit 2.

### Text format additions
- Header (P3): `<root>/  <size> · <n> files · <kind> · git <branch> ↑<ahead> ↓<behind> · <c> changed`. Parts with no data are left out. Kinds join with ` + `.
- Git cell: file `M`, `A`, `D`, `?`, `!`; directory `M<n>` (number of changed files below). The column is absent outside a repo and with `--no-git`.
- `-l` columns: `-rw-r--r--` style mode, owner name, group name, size, count, type, age (and the oldest age for dirs), git, preview.
- Column drop order when width is short: owner/group, preview, git, age, type-mix detail.
- Duplicate marker in the chaff line and in `--why`: `= <relative path of the original>`.
- `--why PATH` output (plain lines): `path:`, `class:` (with rule source and pattern) or `class: none`, `action:`, `salience:` (each term and its value, then the weighted total), `decision:` (`visible`, `folded into <row>`, `in overflow of <dir>`, `in series <row>`, or `hidden, counted in footer`).
- `--stats` output: header, a table `group  files  size` sorted by size, footer. No tree rows.

### Internal Interfaces
- `preview::of(tree, id, info, head) -> Option<String>`: the rules in PRD §8.4 and the FR-56 table. Text previews escape control characters as `\xHH`. Image size, zip central directory, and decompression peeks may read a few KiB more (PRD §8.4).
- `kind::detect(tree, dir, sample_types) -> Vec<&'static str>`: markers `Cargo.toml` → `rust crate`, `package.json` → `node pkg`, `pyproject.toml` → `python pkg`, `pyvenv.cfg` → `venv`, more than 80% images with EXIF → `photos`, `.git/` → `git repo`.
- `dupes::find(tree, visible: &[NodeId]) -> HashMap<NodeId, NodeId>`: same size → head + tail hash → full hash. Empty files are never duplicates. Runs only with `--dupes`.
- `git::GitRepo::status` opens the repo with `gix` with config overrides that turn off `core.fsmonitor`, filters, and hooks. It runs only for files in visible directories.

## Acceptance Criteria

**Shared test conditions.** Unless a scenario says otherwise:

- Tests run as a normal user, not as root.
- The output format is `plain`, with `--width 120` and `--color never`.
- "Fixture" means a tree that the test suite builds in a temp directory.
- `SOURCE_DATE_EPOCH` sets the scan time, so relative ages are fixed.
- "Reference machine" means the machine that the phase-0 report names.

### FR-76: No execution of repo code

**Happy path:**
- GIVEN a git repo fixture whose `.git/config` sets `core.fsmonitor` to `touch $MARK1`
- AND it has an executable `.git/hooks/post-checkout` that runs `touch $MARK2`
- AND it has a clean/smudge filter in `.gitattributes` that runs `touch $MARK3`
- AND it has an executable `run.sh` in the tree that runs `touch $MARK4`
- WHEN the user runs `teak` with git status on (P3 and later) and with `-a --detect full --preview full`
- THEN none of the files `$MARK1` to `$MARK4` SHALL exist
- AND the git status column SHALL still show correct values

**Process check (Linux):**
- GIVEN the same fixture
- WHEN the test runs teak under `strace -f -e trace=execve`
- THEN the trace SHALL show only the one `execve` that starts teak itself

### FR-18: Focus

**Happy path:**
- GIVEN a repo fixture with `src/api/` (200 files) and 8 other top-level directories
- WHEN the user runs `teak --focus src/api -n 30`
- THEN `src/` and `src/api/` SHALL be open
- AND at least half of the rows SHALL be inside `src/api/`
- AND each of the 8 other top-level directories SHALL be visible as a row or counted in an overflow row

**Failure path:**
- WHEN the user runs `teak --focus nope` and `nope` is not inside the root
- THEN teak SHALL print an error and exit with code 2

### FR-19: Header

**Happy path:**
- GIVEN the fixture `fx/` with 3 files of 1 KiB each
- WHEN the user runs `teak fx`
- THEN the first line SHALL start with `fx/  3.0K · 3 files`

**P3:**
- GIVEN a git repo on branch `main`, 2 commits ahead, with `Cargo.toml` at the root
- WHEN the user runs `teak`
- THEN the header SHALL contain `rust crate` and `git main ↑2`

### FR-24: Long view

**Happy path:**
- GIVEN `notes.txt` (mode 644)
- WHEN the user runs `teak -l`
- THEN its row SHALL show `-rw-r--r--`, the owner name, and the group name

### FR-43: Duplicates

**Happy path:**
- GIVEN `a.bin` (visible), `copy/b.bin` (same content), and `c.bin` (same size, other content)
- WHEN the user runs `teak --dupes`
- THEN `b.bin` SHALL be folded and marked `= a.bin`
- AND `c.bin` SHALL NOT be marked

**Default off:**
- WHEN the user runs `teak` without `--dupes`
- THEN no file SHALL be read beyond the bytes that type detection and content checks read (read counter)

**Edge case:**
- GIVEN two empty files
- THEN both SHALL be in class `empty`, not `duplicate`

### FR-56: Previews

**Happy path (one fixture file for each type):**

| File | Expected preview |
|---|---|
| `main.rs`: an SPDX line, a blank line, then `fn main() {` | `fn main() {` |
| `README.md`: `# Shop backend` | `# Shop backend` |
| `package.json` with the keys name, version, scripts | `{name, version, scripts}` |
| `data.csv`: header `id,name,price`, 1,000 rows | `id,name,price ≈1.0k rows` |
| PNG of 1920×1080 | `1920×1080` |
| `dump.sql.gz` (gzipped SQL) | `gzip → sql` |
| `a.zip` with 12 entries | `12 entries` |
| ELF x86-64 binary | `ELF x86-64` |
| symlink `l` → `target` | `⇢ target` |
| broken symlink `d` → `nowhere` | `⇢ nowhere (broken)` |

**Off:**
- WHEN the user runs `teak --preview off`
- THEN no row SHALL have a preview

**Edge case:**
- GIVEN a text file whose first line has control characters
- THEN the preview SHALL show them escaped
- AND it SHALL NOT have raw bytes below 0x20

### FR-57: Git status

**Happy path:**
- GIVEN a repo on `main`, 2 commits ahead of its upstream, with `src/a.rs` modified and `src/b.rs` untracked
- WHEN the user runs `teak`
- THEN `a.rs` SHALL show `M`, and `b.rs` SHALL show `?`
- AND `src/` SHALL show `2` changed files
- AND the header SHALL contain `git main ↑2`

**Off:**
- WHEN the user runs `teak --no-git`
- THEN the output SHALL have no git column and no git data in the header

**Edge cases:**
- GIVEN a directory that is not in a repo
- THEN the output SHALL have no git column
- GIVEN a repo with no commits
- THEN teak SHALL NOT crash, and all files SHALL show `?`

### FR-59: Project kinds

**Happy path:**

| Marker in the directory | Kind |
|---|---|
| `Cargo.toml` | `rust crate` |
| `package.json` | `node pkg` |
| `pyproject.toml` | `python pkg` |
| `pyvenv.cfg` | `venv` |
| more than 80% images with EXIF data | `photos` |

**Edge case:**
- GIVEN both `Cargo.toml` and `package.json`
- THEN the kind SHALL read `rust crate + node pkg`

### FR-60: `--why`

**Happy path:**
- WHEN the user runs `teak --why node_modules`
- THEN the output SHALL name the class `vendor`, the rule and its source, the action `fold`, each salience term with its value, and the final decision
- WHEN the user runs `teak --why src/main.rs` and `src/main.rs` is visible
- THEN the output SHALL say `visible` and list the salience terms

**Failure path:**
- WHEN the user runs `teak --why /etc/passwd` and the root is a different tree
- THEN teak SHALL exit with code 2

### FR-61: `--stats`

**Happy path:**
- WHEN the user runs `teak --stats`
- THEN the output SHALL have the header, a table with type group, file count, and size, and the footer
- AND the output SHALL have no tree rows

## Done Criteria
- [ ] `tests/preview.rs` passes for every row of the FR-56 table, `--preview off`, and the control-character edge case
- [ ] `tests/git.rs` passes for FR-57, including `--no-git`, a dir outside a repo, and a repo with no commits
- [ ] `tests/safety.rs` passes FR-76 with git status on: `$MARK1` to `$MARK4` do not exist, the git column is correct, and strace shows one `execve`
- [ ] `tests/safety.rs` passes FR-77 with real previews in `tty` and `plain`
- [ ] `tests/focus.rs` passes for FR-18
- [ ] `tests/budget.rs` passes the FR-19 P3 header case (`rust crate`, `git main ↑2`)
- [ ] `tests/long.rs` passes for FR-24
- [ ] `tests/dupes.rs` passes for FR-43, including "default off" with the read counter and the empty-file edge case
- [ ] `tests/kind.rs` passes for every row of the FR-59 table and the two-marker edge case
- [ ] `tests/why.rs` passes for FR-60; `tests/stats.rs` passes for FR-61
- [ ] `tests/props.rs`: FR-12 holds with `-l` and `--focus` added to the random flag set
- [ ] `cargo tree -e normal` still shows no HTTP client (FR-10), with `gix` included
- [ ] Snapshot tests cover all row kinds (PRD phase-3 exit test)
- [ ] All task-2 and task-3 tests still pass; `cargo clippy --all-targets -- -D warnings` and `cargo test` pass on Linux and macOS CI
