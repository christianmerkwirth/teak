# Task 2: Build the P1 core — walk, chaff, series, selection, fast types, tty and plain output

## Trace
- **FR-IDs:** FR-01, FR-02, FR-03, FR-04, FR-05, FR-06, FR-07, FR-08, FR-09, FR-10, FR-11, FR-12, FR-13, FR-14, FR-15, FR-16, FR-17, FR-19 (P1 part), FR-20, FR-21, FR-22, FR-23, FR-25, FR-26, FR-27, FR-28, FR-29, FR-30, FR-31, FR-32, FR-33, FR-34, FR-35, FR-36, FR-37, FR-44, FR-45, FR-46, FR-47, FR-48, FR-58, FR-62, FR-70, FR-71, FR-76 (no execution of tree files), FR-77
- **Depends on:** task-1
- **Design:** ../design.md

## Files
- `Cargo.toml` — modify
- `src/main.rs` — modify
- `src/lib.rs` — modify
- `src/cli.rs` — create
- `src/walk.rs` — create
- `src/tree.rs` — create
- `src/aggregate.rs` — create
- `src/chaff/mod.rs` — create
- `src/chaff/rules.rs` — create
- `src/chaff/packs.rs` — create
- `src/chaff/gitignore.rs` — create
- `src/chaff/gitattributes.rs` — create
- `src/series.rs` — create
- `src/salience.rs` — create
- `src/select.rs` — create
- `src/view.rs` — create
- `src/detect/mod.rs` — create
- `src/detect/names.rs` — create
- `src/detect/magic.rs` — create
- `src/detect/ext.rs` — create
- `src/detect/secret.rs` — create
- `src/layout.rs` — create
- `src/style.rs` — create
- `src/render/mod.rs` — create
- `src/render/text.rs` — create
- `benches/scan.rs` — create
- `tests/support/mod.rs` — modify
- `tests/smoke.rs` — update (interface change: the `not implemented yet` path goes away)
- `tests/traversal.rs` — create
- `tests/budget.rs` — create
- `tests/display.rs` — create
- `tests/chaff.rs` — create
- `tests/series.rs` — create
- `tests/detect_fast.rs` — create
- `tests/safety.rs` — create
- `tests/props.rs` — create
- `tests/snapshots/` — create (insta snapshots)

## Design References
- design.md §Architecture (Components: `cli`, `walk`, `tree`, `aggregate`, `chaff`, `series`, `salience`, `select`, `view`, `detect`, `layout`, `style`, `render`; Data Flow)
- design.md §Data Models (Node, Tree, DirAgg, ChaffClass, Action, ChaffTag, TypeGroup, TypeInfo, Unit, Series, Row, View)
- design.md §Interface Contracts (CLI; `teak::run`; `walk::scan`; `aggregate::compute`; `chaff::Classifier`; `series::group`; `salience::score`; `select::plan`; `detect::Detector`; `detect::secret::is_secret`; `layout::fit`; `render::write`)
- design.md §Design Decisions (Pipeline order: detect after select)
- design.md §Design Decisions (Walk all entries; match gitignore ourselves)
- design.md §Design Decisions (Deterministic tree)
- design.md §Design Decisions (Budget accounting)
- design.md §Design Decisions (Salience defaults)
- design.md §Design Decisions (Chaff action resolution)
- design.md §Design Decisions (No content reads with `--detect off`)
- design.md §Design Decisions (Pseudo file systems)
- design.md §Design Decisions (Secret guard)

## Contracts (task-specific)

### CLI flags added in this task
- `[PATH]...`, `-n/--lines N|auto` (min 3), `-L/--depth N`, `--sort salience|name|size|age|type`, `--series-min N`
- `-A/--almost-all`, `-a/--all`, `--no-gitignore`, `--chaff show|dim|fold|hide`, `--show/--fold/--hide CLASS,…` (repeatable, order kept)
- `-x/--one-file-system`, `--follow`
- `--format tty|plain`, `--color auto|always|never`, `--ascii`, `--width N`, `--time relative|iso`, `--bytes`, `--si`, `-j/--threads N`
- `--detect off|fast|full`: until task-3, `full` acts as `fast`
- `--preview off|short|full`: parsed only; task-4 adds previews. Safety tests use the final flags now.
- Usage errors list the valid values and exit with code 2. `-n` errors say that the smallest budget is 3.

### Text format (tty and plain)
- Header: `<root>/  <size> · <n> files`, with `⊘` after the root name when the root is unreadable. For `.`, the root name is the base name of the current directory. `<n>` uses thousands separators.
- Entry row: tree glyphs + name, size, count (dirs only), type, age, perms (only when unusual), flags.
- Size: `< 1024` → whole bytes (`0`, `512`); otherwise one decimal below 10 (`1.5K`, `3.0K`) and whole above (`14M`). `--si` uses base 1000 and a lower-case `k`. `--bytes` prints whole bytes.
- Count column: `812`, `3.2k`, `44k`. Header, footer, and series counts use full numbers with separators.
- Age: the largest unit with a value of 1 or more: `s`, `m`, `h`, `d`, `w`, `mo`, `y`. `--time iso` → `YYYY-MM-DDTHH:MM` in UTC.
- Short perms: `rwx` (any execute bit on a file), `suid`, `sgid`, `o+w`; more than one are joined with `,`.
- Flags: `⊘` no access, `↻` loop, `⇢ <target>` symlink, `⇢ <target> (broken)`, `⊞` mount point.
- Overflow row: `… <d> dirs, <f> files` (either part can be absent), then size and type mix.
- Series row: `<prefix>[<first>–<last>]<suffix>`, total size, `<present> of <range>` when items are missing (else the count), type or type mix, `~<median>`. Special slots: `[hash]`, `[uuid]`, dates as the range.
- Chaff line: `▪ chaff  <item> · <item> · … · +N`. An item is large when its size is 1 MiB or more, or 10% or more of the directory total. Large items come first by size, with size (and count for dirs); then small items by name; then `+N`. Items that do not fit the width go into `+N`.
- Footer: `── <used> of <budget> lines · <n> files · folded <F> (<class> <n> · …) · hidden <H> (<class> <n> · …)`. Classes sort by count (high first), then by name. Empty groups are left out.
- Glyphs: Unicode `├── `, `└── `, `│   `, `…`, `–`, `──`; with `--ascii`: `|-- `, `` `-- ``, `|   `, `...`, `-`, `--`, and `!`, `@loop`, `->`, `[mnt]` for the flags.
- Names: bytes that are not UTF-8 and control characters print as `\xHH`. A long name is cut in the middle with `…`, and its extension stays.
- Colors: one SGR color per type group; dirs bold; chaff and overflow rows dim (SGR 2); `⚠` and `⊘` yellow or red; the large-chaff size bold (SGR 1). `LS_COLORS` changes only the name color. Policy: `--color` wins, then `CLICOLOR_FORCE`, then `NO_COLOR`, then "is stdout a terminal".

### Internal Interfaces
- `detect::fast::type_of(name: &[u8], head: Option<&[u8]>) -> TypeInfo`: name rules, then `infer` magic on `head`, then the extension table.
  - Pre: `head` comes from `io::read_head(path, 4096)`; it is `None` with `--detect off`, for non-regular files, and on a pseudo FS.
- Series members are typed with fast detection when the series has 64 members or fewer; otherwise a sample of 3 per extension, marked `≈`.
- `chaff::packs::PACKS: &[Pack]`: `Pack { marker: &str, entries: &[(&str, ChaffClass)] }`. Entries apply only to siblings of the marker file.
- `chaff::rules::GLOBAL: &[(&str, ChaffClass)]`: global name rules from PRD §6.1. In this task, `LICENSE` is not chaff by name; task-3 adds template matching.

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

### FR-01: Run on paths

**Happy path:**
- GIVEN the current directory is a fixture `proj/` with the file `a.txt`
- WHEN the user runs `teak`
- THEN the header SHALL start with `proj/`
- AND the output SHALL contain a row for `a.txt`

**Several paths:**
- GIVEN the fixtures `p1/` and `p2/`
- WHEN the user runs `teak p1 p2`
- THEN the output SHALL show a full view for `p1/` and then for `p2/`, each with its own header and footer
- AND the line budget SHALL apply to each view

**Failure path:**
- GIVEN the path `missing/` does not exist
- WHEN the user runs `teak missing p1`
- THEN teak SHALL write one error line that names `missing` to stderr
- AND it SHALL still show the view for `p1/`
- AND it SHALL exit with code 1

**Edge case:**
- GIVEN PATH is a regular file `notes.txt`
- WHEN the user runs `teak notes.txt`
- THEN teak SHALL show the header and one row for that file, with all its columns

### FR-02: Symlinks

**Happy path:**
- GIVEN a fixture with `link` → `data/` (100 files)
- WHEN the user runs `teak`
- THEN the row for `link` SHALL show `⇢ data`
- AND the 100 files SHALL be counted once (under `data/`), not twice

**With `--follow`:**
- GIVEN the same fixture
- WHEN the user runs `teak --follow`
- THEN `link` SHALL show as a directory with its own size and count

**Edge case (loop):**
- GIVEN `a/b` → `a`
- WHEN the user runs `teak --follow`
- THEN the row for `a/b` SHALL have the `↻` flag
- AND the scan SHALL end, with exit code 0

**Edge case (broken link):**
- GIVEN `dead` → `nowhere`, which does not exist
- WHEN the user runs `teak`
- THEN the row for `dead` SHALL show the target and a broken mark
- AND the exit code SHALL be 0

### FR-03: Special files

**Happy path:**
- GIVEN a fixture with a FIFO `pipe` that has no writer, and a Unix socket `sock`
- WHEN the user runs `teak --detect full --preview full`
- THEN the rows SHALL show the types `fifo` and `socket`
- AND teak SHALL finish within 2 seconds (an open FIFO would block)

**Edge case:**
- GIVEN the path `/dev/null`
- WHEN the user runs `teak /dev/null`
- THEN the row SHALL show the type `device`
- AND teak SHALL NOT open the file

### FR-04: Read-only

**Happy path:**
- GIVEN a fixture with a snapshot of all paths, sizes, modes, mtimes, and content hashes
- WHEN the user runs `teak -a --dupes --detect full --preview full` and every output format
- THEN the snapshot after the runs SHALL be equal to the snapshot before
- AND no new file SHALL exist in the fixture

**Edge case:**
- GIVEN the fixture is on a read-only mount (or has mode `a-w` on all entries)
- WHEN the user runs `teak`
- THEN the exit code SHALL be 0

### FR-05: No access

**Failure path:**
- GIVEN the directory `locked/` with mode `000`, and the file `ok.txt` next to it
- WHEN the user runs `teak`
- THEN the row for `locked/` SHALL have the `⊘` flag
- AND the row for `ok.txt` SHALL show
- AND stderr SHALL have one warning line for `locked/`
- AND the exit code SHALL be 1

**Edge case:**
- GIVEN the root directory itself has mode `000`
- WHEN the user runs `teak root`
- THEN the header SHALL have the `⊘` flag
- AND the exit code SHALL be 1

### FR-06: One file system

**Happy path:**
- GIVEN a fixture with a different file system mounted at `mnt/` (the test is skipped when it cannot mount one)
- WHEN the user runs `teak -x`
- THEN `mnt/` SHALL show as one row with a mount mark
- AND its content SHALL NOT count in the totals

### FR-07: Odd names

**Happy path:**
- GIVEN a file whose name is the bytes `0xFF 0x61`
- WHEN the user runs `teak`
- THEN the row SHALL show the escaped name `\xFFa`
- AND the exit code SHALL be 0

**Edge case (long name):**
- GIVEN a file with a 250-character name that ends in `.txt`
- WHEN the user runs `teak --width 80`
- THEN the name SHALL be cut in the middle with `…`
- AND it SHALL still end in `.txt`
- AND no line SHALL be wider than 80 columns

**Edge case (deep tree):**
- GIVEN a chain of 500 nested directories
- WHEN the user runs `teak`
- THEN teak SHALL NOT crash
- AND the chain SHALL show as joined rows (FR-15)

### FR-08: Pseudo file systems

**Happy path (Linux only):**
- GIVEN the path `/proc`
- WHEN the user runs `teak /proc -n 20 --detect full --preview full`
- THEN teak SHALL finish with exit code 0 or 1
- AND every type SHALL come from stat or name rules, not from content
- AND no row SHALL have a content preview

### FR-09: Hard links

**Happy path:**
- GIVEN a 1 MiB file `f1` and a hard link `f2` to it, in the directory `d/`
- WHEN the user runs `teak --bytes`
- THEN `d/` SHALL show the size 1048576
- AND both `f1` and `f2` SHALL show as rows

### FR-10: No network

**Happy path:**
- GIVEN a fixture
- WHEN the user runs teak with no network access (for example in `unshare -n` on Linux)
- THEN the output SHALL be byte-identical to a run with network access

**Static check:**
- GIVEN the release build
- WHEN the test inspects the dependency tree
- THEN it SHALL find no HTTP client or telemetry crate

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

### FR-77: No secret previews

**Happy path:**
- GIVEN a fixture with `.env` (`API_KEY=sk-test-123`), `id_rsa`, `server.pem`, `credentials.json`, and `.netrc`
- WHEN the user runs `teak -a --preview full` in every output format, including `--json-full`
- THEN the output SHALL NOT contain `sk-test-123`, `BEGIN`, or any other bytes from these files
- AND each of these rows SHALL show its type, its size, and the text `no preview (secret)`

**Edge case (secret found by content):**
- GIVEN `backup.txt` that starts with `-----BEGIN OPENSSH PRIVATE KEY-----`
- WHEN the user runs `teak --preview full`
- THEN the row SHALL show no content preview

### FR-11: Budget flag

**Happy path:**
- GIVEN a fixture with 500 files in 40 directories
- WHEN the user runs `teak`
- THEN the output SHALL have at most 100 lines
- WHEN the user runs `teak -n 30`
- THEN the output SHALL have at most 30 lines

**`auto`:**
- GIVEN a pseudo-terminal with 40 rows
- WHEN the user runs `teak -n auto`
- THEN the output SHALL have at most 38 lines
- GIVEN the output goes to a pipe
- WHEN the user runs `teak -n auto`
- THEN the budget SHALL be 100

**Failure path:**
- WHEN the user runs `teak -n abc`, `teak -n 0`, or `teak -n 2`
- THEN teak SHALL print a usage error that says the smallest budget is 3
- AND the exit code SHALL be 2

**Edge case:**
- GIVEN a fixture with 5 files
- WHEN the user runs `teak -n 100`
- THEN teak SHALL show all 5 files and print fewer than 100 lines, with no padding

### FR-12: Hard budget limit

**Property test:**
- GIVEN random trees (0 to 20,000 entries, random depth, names, sizes, and chaff) and a random budget N from 3 to 300
- WHEN teak runs with any combination of `-A`, `-a`, `-l`, `--sort`, `--focus`, and `--chaff`
- THEN the number of lines in `tty` and `plain` output SHALL be ≤ N
- AND the number of lines in the `md` tree block SHALL be ≤ N
- AND the number of visible rows in `json` SHALL be ≤ N

### FR-13: Selection by salience

**Happy path:**
- GIVEN root with `a/` (1 file, 1 KB, mtime 3 years ago)
- AND `z/deep/deeper/` with 50 files changed in the last hour, which hold 90% of the total size
- WHEN the user runs `teak -n 12`
- THEN the rows SHALL show files from inside `z/deep/deeper/`
- AND `a/` SHALL show as one folded row

**Edge case:**
- GIVEN 10 sibling directories with equal size, age, and content
- WHEN the user runs `teak -n 12` two times
- THEN both runs SHALL select the same rows (ties break by name)

### FR-14: Overflow row

**Happy path:**
- GIVEN `logs/` with 50 files of 1 KB each
- WHEN teak shows 8 of them
- THEN `logs/` SHALL have one row `… 42 files 42K` with the type mix of the 42 files

**Edge case:**
- GIVEN a directory where teak cannot show 3 subdirectories and 40 files
- WHEN teak builds the overflow row
- THEN the row SHALL read `… 3 dirs, 40 files` with their total size

### FR-15: Joined directory chains

**Happy path:**
- GIVEN only `src/main/java/com/acme/App.java`
- WHEN the user runs `teak`
- THEN one row SHALL read `src/main/java/com/acme/`
- AND `App.java` SHALL be its child row

**Edge case:**
- GIVEN `a/b/` where `b/` has two child directories `c/` and `d/`
- WHEN the user runs `teak`
- THEN the joined row SHALL be `a/b/`, with `c/` and `d/` as children

### FR-16: Depth limit

**Happy path:**
- GIVEN a tree 5 levels deep
- WHEN the user runs `teak -L 2`
- THEN no row SHALL be deeper than level 2
- AND the totals SHALL still include all levels

**Edge case:**
- WHEN the user runs `teak -L 0`
- THEN the output SHALL show only the header, one summary row for the root, and the footer

### FR-17: Display order

**Happy path:**
- GIVEN `b.txt` (10 KB), `a.txt` (1 KB), and the directory `c/`
- WHEN the user runs `teak`
- THEN the order SHALL be `c/`, `a.txt`, `b.txt`
- WHEN the user runs `teak --sort size`
- THEN `b.txt` SHALL come before `a.txt`
- AND the set of visible rows SHALL be the same as in the default run

**Failure path:**
- WHEN the user runs `teak --sort foo`
- THEN teak SHALL list the valid keys and exit with code 2

### FR-19: Header

**Happy path:**
- GIVEN the fixture `fx/` with 3 files of 1 KiB each
- WHEN the user runs `teak fx`
- THEN the first line SHALL start with `fx/  3.0K · 3 files`

**P3:**
- GIVEN a git repo on branch `main`, 2 commits ahead, with `Cargo.toml` at the root
- WHEN the user runs `teak`
- THEN the header SHALL contain `rust crate` and `git main ↑2`

### FR-20: Footer

**Happy path:**
- GIVEN a fixture with `node_modules/` (120 files) and 30 other files
- WHEN the user runs `teak`
- THEN the last line SHALL contain `of 100 lines`, `150 files`, and `vendor 120`

### FR-21: Every entry is counted

**Property test:**
- GIVEN random trees and random flags, as in FR-12
- WHEN teak runs in `json` format
- THEN visible files + the files counted in summary, overflow, series, and chaff rows + the hidden files counted in the footer SHALL equal the file count from an independent walk in the test

### FR-22: Default columns

**Happy path:**
- GIVEN `run.sh` (mode 755) and `notes.txt` (mode 644)
- WHEN the user runs `teak`
- THEN the row for `run.sh` SHALL show `rwx`
- AND the row for `notes.txt` SHALL show no permission field
- AND directory rows SHALL show a file count, and file rows SHALL NOT

**Edge cases:**
- GIVEN a file with mode 4755
- THEN its row SHALL show `suid`
- GIVEN a file with mode 666
- THEN its row SHALL show `o+w`

### FR-23: Size units

**Happy path:**
- GIVEN a file of 1536 bytes
- WHEN the user runs `teak`
- THEN the size SHALL read `1.5K`
- WHEN the user runs `teak --bytes`
- THEN the size SHALL read `1536`
- WHEN the user runs `teak --si`
- THEN the size SHALL read `1.5k`

**Edge case:**
- GIVEN an empty file
- THEN the size SHALL read `0`

### FR-25: Width

**Happy path:**
- GIVEN the full fixture from PRD §5.1
- WHEN the user runs `teak --width 60`
- THEN no line SHALL be wider than 60 display columns
- AND previews SHALL drop out before ages do

**Edge case:**
- WHEN the user runs `teak --width 20`
- THEN each row SHALL show at least the tree glyphs, a cut name, and the size
- AND no line SHALL be wider than 20 columns

### FR-26: Time format

**Happy path:**
- GIVEN a file with an mtime 2 hours before the scan time
- WHEN the user runs `teak`
- THEN the age SHALL read `2h`
- WHEN the user runs `teak --time iso`
- THEN the age SHALL read the mtime as `YYYY-MM-DDTHH:MM` in UTC

### FR-27: Colors

**Happy path:**
- GIVEN `main.rs` and `photo.png`
- WHEN the user runs `teak --color always`
- THEN the two names SHALL have different SGR color codes
- AND the chaff line SHALL have the dim attribute (SGR 2)

**Environment:**
- GIVEN `NO_COLOR=1` and a pseudo-terminal
- WHEN the user runs `teak`
- THEN the output SHALL have no ANSI codes
- GIVEN `CLICOLOR_FORCE=1` and a pipe
- WHEN the user runs `teak`
- THEN the output SHALL have ANSI codes
- GIVEN `NO_COLOR=1`
- WHEN the user runs `teak --color always`
- THEN the output SHALL have ANSI codes (the flag wins)

**Edge case:**
- GIVEN `LS_COLORS='*.rs=01;31'`
- WHEN the user runs `teak --color always`
- THEN the name `main.rs` SHALL be bold red
- AND the colors of the other columns SHALL NOT change

### FR-28: Not color only

**Happy path:**
- GIVEN any fixture
- WHEN the test strips all ANSI codes from `teak --color always`
- THEN the result SHALL be byte-identical to `teak --color never`

### FR-29: ASCII glyphs

**Happy path:**
- GIVEN a fixture with ASCII-only names
- WHEN the user runs `teak --ascii`
- THEN every byte of the output SHALL be below 0x80
- AND the tree SHALL use `|--` and `` `-- ``

### FR-30: Chaff classes

**Happy path:**
- GIVEN a fixture with one example for each class (`node_modules/`, `target/` with `Cargo.toml`, `__pycache__/`, `.git/`, `.DS_Store`, `Cargo.lock`, `api.pb.go`, `app.min.js`, an MIT `LICENSE`, an empty `.gitkeep`)
- WHEN the user runs `teak --format json`
- THEN each entry SHALL carry its class
- AND `vendor`, `build`, `cache`, `lock`, `generated`, `minified`, `boilerplate`, and `empty` entries SHALL be in the chaff line
- AND `vcs` and `editor` entries SHALL be counted only in the footer

### FR-31: Rule order

**Happy path:**
- GIVEN `.gitattributes` with `docs/api.md linguist-generated`
- WHEN the user runs `teak`
- THEN `docs/api.md` SHALL be in class `generated`

**User wins:**
- GIVEN the same fixture
- WHEN the user runs `teak --show=generated`
- THEN `docs/api.md` SHALL show as a normal row

**Negation:**
- GIVEN `.gitattributes` with `vendor/** -linguist-vendored`
- WHEN the user runs `teak`
- THEN `vendor/` SHALL be a normal directory, not chaff

### FR-32: Project-kind rules in context

**Happy path:**
- GIVEN `app/Cargo.toml` and `app/target/`
- THEN `app/target/` SHALL be in class `build`

**Failure path:**
- GIVEN `photos/target/` with no `Cargo.toml` next to it
- THEN `photos/target/` SHALL be a normal directory

**Edge case:**
- GIVEN `web/package.json` and `web/dist/`
- THEN `web/dist/` SHALL be in class `build`

### FR-33: One chaff line

**Happy path:**
- GIVEN a directory with `node_modules/` (180 MB), `target/` (1.2 GB, next to `Cargo.toml`), `Cargo.lock`, an MIT `LICENSE`, and 4 empty files
- WHEN the user runs `teak`
- THEN the directory SHALL have exactly one chaff line
- AND `target/ 1.2G` SHALL come before `node_modules/ 180M`
- AND `Cargo.lock` and `LICENSE (MIT)` SHALL show by name
- AND the line SHALL end with `+4`

**Edge case:**
- GIVEN the chaff items do not all fit in the width
- WHEN teak builds the chaff line
- THEN the items that do not fit SHALL be counted in `+N`
- AND the line SHALL NOT be wider than the output width

### FR-34: Large chaff first

**Happy path:**
- GIVEN `target/` (1.2 GB) and `src/` (14 MB) in one directory
- WHEN the user runs `teak --color always`
- THEN the chaff line SHALL be the first child row of that directory
- AND its size SHALL have the bold attribute (SGR 1)

**Failure path:**
- GIVEN the chaff is 10 MB and the rest is 14 MB
- THEN the chaff line SHALL be the last child row

### FR-35: `-A` and `-a`

**Default:**
- GIVEN `.env`, `.git/` (10 MB), `.vscode/`, and a gitignored `build.log`
- WHEN the user runs `teak`
- THEN `.env`, `.git/`, and `.vscode/` SHALL NOT appear as rows or in a chaff line
- AND the footer SHALL count them as hidden

**`-A`:**
- WHEN the user runs `teak -A`
- THEN `.env`, `.git/ 10M`, and `.vscode/` SHALL appear in the root's chaff line

**`-a`:**
- WHEN the user runs `teak -a`
- THEN `.env`, `.git/`, `.vscode/`, and `build.log` SHALL appear as normal rows
- AND the output SHALL have no chaff line

### FR-36: Chaff controls

**Happy path:**
- WHEN the user runs `teak --chaff show`
- THEN the output SHALL have no chaff line, and all chaff entries SHALL be normal rows
- WHEN the user runs `teak --show=lock`
- THEN `Cargo.lock` SHALL be a normal row, and the other chaff SHALL stay folded
- WHEN the user runs `teak --hide=vendor`
- THEN `node_modules/` SHALL NOT be in the chaff line, and the footer SHALL count it

**Failure path:**
- WHEN the user runs `teak --show=foo`
- THEN teak SHALL list the valid classes and exit with code 2

**Edge case:**
- WHEN the user runs `teak --show=lock --hide=lock`
- THEN the later flag SHALL win, so `Cargo.lock` is hidden

### FR-37: Gitignore

**Happy path:**
- GIVEN `.gitignore` with `data/`, and `data/` holds a 40 GB sparse file
- WHEN the user runs `teak`
- THEN `data/ 40G` SHALL appear in the chaff line, as the first child row (FR-34)
- WHEN the user runs `teak --no-gitignore`
- THEN `data/` SHALL be a normal row

**Edge case:**
- GIVEN a directory that is not a git repo but has a `.gitignore`
- WHEN the user runs `teak`
- THEN teak SHALL apply that `.gitignore`

### FR-44: Series rows

**Happy path:**
- GIVEN `IMG_0001.png` to `IMG_0010.png`, without `IMG_0005.png`
- WHEN the user runs `teak`
- THEN one row SHALL read `IMG_[0001–0010].png` and show `9 of 10`

**Failure path:**
- GIVEN only `IMG_0001.png` to `IMG_0003.png`
- THEN they SHALL show as 3 separate rows
- WHEN the user runs `teak --series-min 2`
- THEN they SHALL show as one series row

**Edge case:**
- GIVEN `IMG_0001.png` to `IMG_0004.png` and `IMG_0001.jpg` to `IMG_0004.jpg`
- THEN teak SHALL show two series rows, one for each extension

### FR-45: Special slots

**Happy path:**
- GIVEN `log-2024-05-01.txt` to `log-2024-05-30.txt`
- THEN the row SHALL read `log-[2024-05-01–2024-05-30].txt`
- GIVEN 20 files with 40-character hex names
- THEN the row SHALL read `[hash]` and show the count 20
- GIVEN 6 files with UUID names and the extension `.json`
- THEN the row SHALL read `[uuid].json`

**Edge case:**
- GIVEN names with 15 hex characters
- THEN teak SHALL NOT treat them as hashes

### FR-46: Series row content

**Happy path:**
- GIVEN 10 PNG files of 300, 340, 350, 350, 350, 360, 360, 380, 400, and 410 KB
- THEN the series row SHALL show `10`, the total size, `~355K`, and `png`

**Edge case:**
- GIVEN the same series, where one file is really a JPEG
- THEN the row SHALL show the type mix `png 9 · jpeg 1`

### FR-47: Uniform directories

**Happy path:**
- GIVEN `photos/` with 95 files `IMG_####.jpg` and 5 other files
- WHEN teak selects `photos/` to open
- THEN `photos/` SHALL show as one row with `jpeg` and the median size

**Failure path:**
- GIVEN 85 such files and 15 other files
- THEN `photos/` SHALL open in the normal way

### FR-48: Fast detection

**Happy path:**
- GIVEN `Makefile`, `x.png` (PNG magic), `noext` (PDF magic), and `a.rs` (Rust text)
- WHEN the user runs `teak --detect fast`
- THEN the types SHALL be `makefile`, `png`, `pdf`, and `rust`

**Edge case:**
- GIVEN `y.png` with JPEG magic
- THEN the type SHALL be `jpeg` (magic bytes win over the extension)

### FR-58: Folded-directory summary

**Happy path:**
- GIVEN a folded directory with 2 subdirectories and files of 5 types (rust 50%, toml 20%, md 15%, json 10%, txt 5% by size)
- THEN its row SHALL show the total size, the file count, `2 dirs`, `rust`, `toml`, `md`, `+2 types`, and the newest mtime as an age

**Edge case:**
- GIVEN an empty folded directory
- THEN its row SHALL show `0` and `empty`

### FR-62: Default format

**Happy path:**
- GIVEN the output goes to a pipe
- WHEN the user runs `teak`
- THEN the output SHALL have no ANSI codes and no line wider than 120 columns
- GIVEN a pseudo-terminal with 100 columns
- WHEN the user runs `teak`
- THEN the format SHALL be `tty` and no line SHALL be wider than 100 columns

**Edge case:**
- WHEN the user runs `teak --format plain --width 80` on a pseudo-terminal
- THEN the output SHALL have no ANSI codes and no line wider than 80 columns

### FR-70: Exit codes

**Happy path:**
- GIVEN a readable fixture
- WHEN the user runs `teak`
- THEN the exit code SHALL be 0

**Failure paths:**
- GIVEN a fixture with an unreadable directory
- THEN the exit code SHALL be 1
- WHEN the user runs `teak --bogus`
- THEN the exit code SHALL be 2

### FR-71: 10k files

- GIVEN a generated tree with 10,000 files and a warm teak cache
- WHEN the test runs `teak`
- THEN the median time SHOULD be less than 150 ms

## Done Criteria
- [ ] `tests/traversal.rs` passes for FR-01 to FR-09 (FR-06 skips when it cannot mount; FR-08 runs only on Linux)
- [ ] `tests/safety.rs` passes for FR-04, FR-10 (`cargo tree -e normal` check and the `unshare -n` run on Linux), FR-76 (`run.sh` is never run; the strace check shows one `execve`), and FR-77
- [ ] `tests/budget.rs` passes for FR-11, FR-13 to FR-17, FR-19 (P1 happy path only), FR-20, and FR-70
- [ ] `tests/props.rs` (proptest): FR-12 holds in `tty` and `plain` for random trees, budgets 3 to 300, and the P1 flags `-A`, `-a`, `--sort`, `--chaff`. Task-4 adds `-l` and `--focus`; task-5 adds `md` and `json`.
- [ ] `tests/props.rs`: FR-21 holds through the library `View` (visible + summarized + footer counts = an independent walk). Task-5 adds the JSON form.
- [ ] `tests/display.rs` passes for FR-22, FR-23, FR-25 to FR-29, FR-58, and FR-62 (pty tests with `portable-pty`)
- [ ] `tests/chaff.rs` passes for FR-30 to FR-37. The FR-30 class check uses the library tags until task-5 adds JSON. `LICENSE (MIT)` in FR-30 and FR-33 is checked in task-3.
- [ ] `tests/series.rs` passes for FR-44 to FR-47
- [ ] `tests/detect_fast.rs` passes for FR-48
- [ ] `select::plan` has a `debug_assert!` for `rows.len() <= budget`
- [ ] The output for `-j 1` and `-j 8` on the same fixture is byte-identical (plain)
- [ ] `insta` snapshots exist for the fixtures: rust repo, node repo with `node_modules`, photo folder, data lake with series, odd permissions and broken links
- [ ] `benches/scan.rs` and `scripts/bench.sh 10k` meet FR-71 on the reference machine, or the result is a recorded warning if phase 0 said no-go
- [ ] A useful view of the Linux kernel tree fits in `-n 40` (PRD phase-1 exit test; check by hand and save the output in `docs/phase1-kernel.txt`)
- [ ] `cargo clippy --all-targets -- -D warnings` and `cargo test` pass on Linux and macOS CI
