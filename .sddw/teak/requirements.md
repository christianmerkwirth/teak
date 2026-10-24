# teak — Requirements

Source: [PRD.md](PRD.md) v0.2 · Spec date: 2026-10-24

Phase tags: **P1** core · **P2** types · **P3** context · **P4** agents and release.
A phase-0 test (build, size, and speed report) comes before P1. It is a gate, not a
feature (see §6).

---

## 1. Project

- Path: `.`

---

## 2. Purpose

teak lets a person or an LLM agent understand a large directory tree at a glance.
It fits the tree into a fixed line budget, shows the real file type of each entry
from its content, and folds minor and boilerplate content into counted summary
lines, so nothing is hidden silently.

---

## 3. User Stories

- **US-1 Developer.** As a developer who opens an unknown repo, I want one screen
  that shows the structure, where the bulk is, what changed, and what is
  generated, so that I can orient myself in seconds.
- **US-2 Disk cleaner.** As a person who cleans a disk, I want to see the large,
  old, and suspicious files (wrong extension, duplicates, huge single files), so
  that I know what to keep and what to delete.
- **US-3 LLM agent.** As an LLM agent, I want a compact and stable Markdown or JSON
  map of a tree within a line budget, so that I understand a repo for a few
  hundred tokens.
- **US-4 Data engineer.** As a data engineer, I want long series of similar files
  shown as one row with count, range, and typical size, so that 10,000 files do
  not hide the structure.
- **US-5 Control and trust.** As any user, I want to reveal what teak folded or
  hid, and to ask why, so that I can trust the view and change it.

---

## 4. Functional Requirements

### A. Traversal and safety

- FR-01 [P1]: User can run `teak [PATH]...`. With no PATH, teak SHALL show the current directory.
- FR-02 [P1]: teak SHALL NOT follow symlinks unless the user gives `--follow`. With `--follow`, teak SHALL detect loops and mark them `↻`.
- FR-03 [P1]: teak SHALL NOT open FIFOs, sockets, or device files.
- FR-04 [P1]: teak SHALL NOT write to or change anything in the scanned tree.
- FR-05 [P1]: When teak has no access to an entry, it SHALL mark the entry `⊘`, continue the scan, and exit with code 1.
- FR-06 [P1]: User can keep the scan on one file system with `-x`.
- FR-07 [P1]: teak SHALL show names that are not UTF-8 (escaped) and very long names (cut in the middle) without a crash.
- FR-08 [P1]: teak SHALL NOT read file content in pseudo file systems (`/proc`, `/sys`).
- FR-09 [P1]: teak SHALL count hard-linked files only once in sizes.
- FR-10 [P1]: teak SHALL NOT make network connections or send telemetry.
- FR-76 [P1, holds in all phases]: teak SHALL NOT run any program or file from the scanned tree, including git hooks, git filters, and `core.fsmonitor` commands.
- FR-77 [P1, holds in all phases]: teak SHALL NOT show content previews of likely secret files. These files show only their type and size.

### B. Line budget and selection

- FR-11 [P1]: User can set the line budget with `-n N` (default 100) or `-n auto` (terminal height − 2).
- FR-12 [P1]: The tree view (header, rows, footer) SHALL NOT have more lines than the budget. In `md`, the budget applies to the tree block. In `json`, it applies to the number of visible rows.
- FR-13 [P1]: teak SHALL choose which directories to open by salience, not by a fixed depth.
- FR-14 [P1]: An open directory that has more children than fit SHALL show one overflow row with count, size, and type mix.
- FR-15 [P1]: A chain of directories that each have only one child directory SHALL show as one row (for example `src/main/java/`).
- FR-16 [P1]: User can limit the depth with `-L N`.
- FR-17 [P1]: User can change the display order with `--sort`. This SHALL NOT change which rows teak selects. The default order is directories first, then by name.
- FR-18 [P3]: User can give `--focus PATH`. teak SHALL open all ancestors of PATH, spend most of the budget inside PATH, and keep context around it.
- FR-19 [P1]: The first line SHALL be a header with the root, total size, and file count. From P3, the header SHALL also show the project kind and the git state.
- FR-20 [P1]: The last line SHALL be a footer with lines used, total files, and folded and hidden counts by class.
- FR-21 [P1]: Every entry SHALL be visible, or counted in a summary, overflow, series, or chaff row, or counted in the footer.

### C. Columns and display

- FR-22 [P1]: Default rows SHALL show name, size, file count (directories only), type, relative age, flags, and permissions only when they are unusual.
- FR-23 [P1]: Sizes SHALL be human-readable. User can switch to bytes with `--bytes` and to SI units with `--si`.
- FR-24 [P3]: User can see all `ls -lah` columns with `-l`.
- FR-25 [P1]: Rows SHALL fit the output width. When space is short, columns SHALL drop out in this order: owner/group, preview, git, age, type-mix detail.
- FR-26 [P1]: User can choose `--time relative` (default) or `--time iso`.
- FR-27 [P1]: Colors SHALL show the type group. Chaff and overflow rows SHALL be dim. teak SHALL respect `--color auto|always|never`, `NO_COLOR`, and `CLICOLOR_FORCE`. It SHALL use `LS_COLORS` for names only.
- FR-28 [P1]: Color SHALL NOT be the only carrier of any information.
- FR-29 [P1]: User can ask for ASCII glyphs only with `--ascii`.

### D. Chaff

- FR-30 [P1]: teak SHALL put entries into the chaff classes `vendor`, `build`, `cache`, `vcs`, `editor`, `lock`, `generated`, `minified`, `boilerplate`, `empty`, and `duplicate`, each with its default action from PRD §6.1.
- FR-31 [P1]: Chaff rules SHALL apply in this order (the first match wins): user config and flags, then `.gitattributes` (`linguist-generated`, `linguist-vendored`), then project-kind rule packs, then content checks and global rules.
- FR-32 [P1]: Project-kind rules SHALL apply only next to their marker file.
- FR-33 [P1]: Each directory SHALL have at most one chaff line. Large entries come first with their size, then small entries by name, then `+N` for the rest.
- FR-34 [P1]: When the chaff in a directory is larger than everything else in it, the chaff line SHALL be the first child row and its size SHALL be bold.
- FR-35 [P1]: User can show hidden entries, folded into the chaff line, with `-A`. User can show everything as normal rows, with no gitignore rules, with `-a`.
- FR-36 [P1]: User can set chaff actions with `--chaff LEVEL` and with `--show`, `--fold`, and `--hide CLASS,…`.
- FR-37 [P1]: Gitignored entries SHALL be folded, not hidden, by default. User can turn gitignore rules off with `--no-gitignore`.
- FR-38 [P2]: teak SHALL classify a file as `generated` when the first 2 KB contain a known marker.
- FR-39 [P2]: teak SHALL classify minified files as `minified`.
- FR-40 [P2]: teak SHALL match standard boilerplate texts against a built-in template table and label the row, for example `LICENSE (MIT)`.
- FR-41 [P2]: User can add templates with `--templates DIR`, use only their own templates with `--templates-only`, and export the built-in templates with `--dump-templates DIR`.
- FR-42 [P2]: Content checks SHALL read at most 64 KiB of each checked file, in one read after type detection. They SHALL NOT read files that type detection does not read. (Amended 2026-10-24, see design.md §Design Decisions "Content reads".)
- FR-43 [P3]: User can find duplicate files with `--dupes`. A duplicate of a visible file SHALL be folded and marked `= original-path`.

### E. Series

- FR-44 [P1]: N or more siblings (`--series-min N`, default 4) with the same name shape and extension SHALL show as one series row, with the range and the number of missing items.
- FR-45 [P1]: teak SHALL recognize dates, hex hashes (16 or more hex characters), and UUIDs as special series slots.
- FR-46 [P1]: A series row SHALL show count, total size, median size, and the type or type mix.
- FR-47 [P1]: A directory in which more than 90% of the files share one type and one shape SHALL show as one row, also when it is open.

### F. Type detection

- FR-48 [P1]: Fast detection SHALL use, in this order: stat, exact name rules, magic bytes, extension.
- FR-49 [P2]: In `--detect full` (the default), teak SHALL type every visible file with Magika.
- FR-50 [P2]: A folded directory SHALL show a type mix from a sample that is stratified by extension, marked `≈`.
- FR-51 [P2]: Content detection SHALL stop at a global cap (5,000 files or 2 seconds, whichever comes first). After the cap, teak SHALL use the extension and mark the result as estimated. Inside this cap, Magika SHALL type at most 300 files per run (visible rows first, then samples); the other files use fast detection. (Amended 2026-10-24: Plan A1 of the phase-0 report, so FR-72 can hold.)
- FR-52 [P2]: User can choose `--detect off|fast|full`.
- FR-53 [P2]: A file whose content type does not match its extension SHALL get a `⚠` mark. User can list only these files with `--mismatches`.
- FR-54 [P2]: teak SHALL cache detected types and SHALL NOT return a cached type for a changed file. User can turn the cache off with `--no-cache`.
- FR-55 [P2]: A build without Magika SHALL work and use fast detection.

### G. Previews, git, summaries, explanations

- FR-56 [P3]: Rows SHALL show a short preview for each type (PRD §8.4). User can choose `--preview off|short|full`.
- FR-57 [P3]: In a git repo, teak SHALL show the git status of each file, the count of changed files for each directory, and the branch and ahead/behind state in the header. User can turn this off with `--no-git`.
- FR-58 [P1]: The summary of a folded directory SHALL show size, file count, directory count, the top 3 types plus `+N types`, and the newest change time.
- FR-59 [P3]: Summaries SHALL show the project kind when teak finds marker files.
- FR-60 [P3]: User can run `--why PATH` to see the matched rule, the salience terms, and the decision for PATH.
- FR-61 [P3]: User can run `--stats` to see only the header, the type table, and the footer.

### H. Output formats, config, and release

- FR-62 [P1]: The format SHALL default to `tty` on a terminal and to `plain` in a pipe. `plain` SHALL have no ANSI codes and a default width of 120.
- FR-63 [P4]: User can get Markdown output with `--format md`.
- FR-64 [P4]: User can get JSON output with `--format json` (visible rows and summaries, versioned schema) or `--json-full` (every entry).
- FR-65 [P4]: The same tree and the same scan time SHALL give byte-identical output in `plain`, `md`, and `json`.
- FR-66 [P4]: User can set defaults in `~/.config/teak/config.toml` and in `.teak.toml`. CLI flags SHALL override the project file, and the project file SHALL override the user file.
- FR-67 [P4]: User can change the salience weights in the config.
- FR-68 [P4]: teak SHALL install with `cargo install teak` and from a Homebrew tap, on Linux (x86_64, aarch64) and macOS (x86_64, arm64).
- FR-69 [P4]: teak SHALL ship bash, zsh, and fish completions and a man page.
- FR-70 [P1]: Exit codes SHALL be 0 for success, 1 for success with errors, and 2 for a usage error.

### I. Performance (SHOULD until the phase-0 report; SHALL after it)

- FR-71 [P1]: A scan of 10k files with a warm cache SHOULD take less than 150 ms.
- FR-72 [P2]: A scan of 100k files with `--detect full` and a warm OS cache SHOULD take less than 1 s.
- FR-73 [P2]: A scan of 1M files with a warm OS cache SHOULD take less than 5 s and use less than 300 MB of memory.
- FR-74 [P2]: Magika on about 100 visible files SHOULD take less than 300 ms.
- FR-75 [P2]: On a second run with the type cache, the cost of type detection SHOULD be close to zero.

---

## 5. Acceptance Criteria

**Shared test conditions.** Unless a scenario says otherwise:

- Tests run as a normal user, not as root.
- The output format is `plain`, with `--width 120` and `--color never`.
- "Fixture" means a tree that the test suite builds in a temp directory.
- `SOURCE_DATE_EPOCH` sets the scan time, so relative ages are fixed.
- "Reference machine" means the machine that the phase-0 report names.

### A. Traversal and safety

#### FR-01: Run on paths

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

#### FR-02: Symlinks

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

#### FR-03: Special files

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

#### FR-04: Read-only

**Happy path:**
- GIVEN a fixture with a snapshot of all paths, sizes, modes, mtimes, and content hashes
- WHEN the user runs `teak -a --dupes --detect full --preview full` and every output format
- THEN the snapshot after the runs SHALL be equal to the snapshot before
- AND no new file SHALL exist in the fixture

**Edge case:**
- GIVEN the fixture is on a read-only mount (or has mode `a-w` on all entries)
- WHEN the user runs `teak`
- THEN the exit code SHALL be 0

#### FR-05: No access

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

#### FR-06: One file system

**Happy path:**
- GIVEN a fixture with a different file system mounted at `mnt/` (the test is skipped when it cannot mount one)
- WHEN the user runs `teak -x`
- THEN `mnt/` SHALL show as one row with a mount mark
- AND its content SHALL NOT count in the totals

#### FR-07: Odd names

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

#### FR-08: Pseudo file systems

**Happy path (Linux only):**
- GIVEN the path `/proc`
- WHEN the user runs `teak /proc -n 20 --detect full --preview full`
- THEN teak SHALL finish with exit code 0 or 1
- AND every type SHALL come from stat or name rules, not from content
- AND no row SHALL have a content preview

#### FR-09: Hard links

**Happy path:**
- GIVEN a 1 MiB file `f1` and a hard link `f2` to it, in the directory `d/`
- WHEN the user runs `teak --bytes`
- THEN `d/` SHALL show the size 1048576
- AND both `f1` and `f2` SHALL show as rows

#### FR-10: No network

**Happy path:**
- GIVEN a fixture
- WHEN the user runs teak with no network access (for example in `unshare -n` on Linux)
- THEN the output SHALL be byte-identical to a run with network access

**Static check:**
- GIVEN the release build
- WHEN the test inspects the dependency tree
- THEN it SHALL find no HTTP client or telemetry crate

#### FR-76: No execution of repo code

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

#### FR-77: No secret previews

**Happy path:**
- GIVEN a fixture with `.env` (`API_KEY=sk-test-123`), `id_rsa`, `server.pem`, `credentials.json`, and `.netrc`
- WHEN the user runs `teak -a --preview full` in every output format, including `--json-full`
- THEN the output SHALL NOT contain `sk-test-123`, `BEGIN`, or any other bytes from these files
- AND each of these rows SHALL show its type, its size, and the text `no preview (secret)`

**Edge case (secret found by content):**
- GIVEN `backup.txt` that starts with `-----BEGIN OPENSSH PRIVATE KEY-----`
- WHEN the user runs `teak --preview full`
- THEN the row SHALL show no content preview

### B. Line budget and selection

#### FR-11: Budget flag

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

#### FR-12: Hard budget limit

**Property test:**
- GIVEN random trees (0 to 20,000 entries, random depth, names, sizes, and chaff) and a random budget N from 3 to 300
- WHEN teak runs with any combination of `-A`, `-a`, `-l`, `--sort`, `--focus`, and `--chaff`
- THEN the number of lines in `tty` and `plain` output SHALL be ≤ N
- AND the number of lines in the `md` tree block SHALL be ≤ N
- AND the number of visible rows in `json` SHALL be ≤ N

#### FR-13: Selection by salience

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

#### FR-14: Overflow row

**Happy path:**
- GIVEN `logs/` with 50 files of 1 KB each
- WHEN teak shows 8 of them
- THEN `logs/` SHALL have one row `… 42 files 42K` with the type mix of the 42 files

**Edge case:**
- GIVEN a directory where teak cannot show 3 subdirectories and 40 files
- WHEN teak builds the overflow row
- THEN the row SHALL read `… 3 dirs, 40 files` with their total size

#### FR-15: Joined directory chains

**Happy path:**
- GIVEN only `src/main/java/com/acme/App.java`
- WHEN the user runs `teak`
- THEN one row SHALL read `src/main/java/com/acme/`
- AND `App.java` SHALL be its child row

**Edge case:**
- GIVEN `a/b/` where `b/` has two child directories `c/` and `d/`
- WHEN the user runs `teak`
- THEN the joined row SHALL be `a/b/`, with `c/` and `d/` as children

#### FR-16: Depth limit

**Happy path:**
- GIVEN a tree 5 levels deep
- WHEN the user runs `teak -L 2`
- THEN no row SHALL be deeper than level 2
- AND the totals SHALL still include all levels

**Edge case:**
- WHEN the user runs `teak -L 0`
- THEN the output SHALL show only the header, one summary row for the root, and the footer

#### FR-17: Display order

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

#### FR-18: Focus

**Happy path:**
- GIVEN a repo fixture with `src/api/` (200 files) and 8 other top-level directories
- WHEN the user runs `teak --focus src/api -n 30`
- THEN `src/` and `src/api/` SHALL be open
- AND at least half of the rows SHALL be inside `src/api/`
- AND each of the 8 other top-level directories SHALL be visible as a row or counted in an overflow row

**Failure path:**
- WHEN the user runs `teak --focus nope` and `nope` is not inside the root
- THEN teak SHALL print an error and exit with code 2

#### FR-19: Header

**Happy path:**
- GIVEN the fixture `fx/` with 3 files of 1 KiB each
- WHEN the user runs `teak fx`
- THEN the first line SHALL start with `fx/  3.0K · 3 files`

**P3:**
- GIVEN a git repo on branch `main`, 2 commits ahead, with `Cargo.toml` at the root
- WHEN the user runs `teak`
- THEN the header SHALL contain `rust crate` and `git main ↑2`

#### FR-20: Footer

**Happy path:**
- GIVEN a fixture with `node_modules/` (120 files) and 30 other files
- WHEN the user runs `teak`
- THEN the last line SHALL contain `of 100 lines`, `150 files`, and `vendor 120`

#### FR-21: Every entry is counted

**Property test:**
- GIVEN random trees and random flags, as in FR-12
- WHEN teak runs in `json` format
- THEN visible files + the files counted in summary, overflow, series, and chaff rows + the hidden files counted in the footer SHALL equal the file count from an independent walk in the test

### C. Columns and display

#### FR-22: Default columns

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

#### FR-23: Size units

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

#### FR-24: Long view

**Happy path:**
- GIVEN `notes.txt` (mode 644)
- WHEN the user runs `teak -l`
- THEN its row SHALL show `-rw-r--r--`, the owner name, and the group name

#### FR-25: Width

**Happy path:**
- GIVEN the full fixture from PRD §5.1
- WHEN the user runs `teak --width 60`
- THEN no line SHALL be wider than 60 display columns
- AND previews SHALL drop out before ages do

**Edge case:**
- WHEN the user runs `teak --width 20`
- THEN each row SHALL show at least the tree glyphs, a cut name, and the size
- AND no line SHALL be wider than 20 columns

#### FR-26: Time format

**Happy path:**
- GIVEN a file with an mtime 2 hours before the scan time
- WHEN the user runs `teak`
- THEN the age SHALL read `2h`
- WHEN the user runs `teak --time iso`
- THEN the age SHALL read the mtime as `YYYY-MM-DDTHH:MM` in UTC

#### FR-27: Colors

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

#### FR-28: Not color only

**Happy path:**
- GIVEN any fixture
- WHEN the test strips all ANSI codes from `teak --color always`
- THEN the result SHALL be byte-identical to `teak --color never`

#### FR-29: ASCII glyphs

**Happy path:**
- GIVEN a fixture with ASCII-only names
- WHEN the user runs `teak --ascii`
- THEN every byte of the output SHALL be below 0x80
- AND the tree SHALL use `|--` and `` `-- ``

### D. Chaff

#### FR-30: Chaff classes

**Happy path:**
- GIVEN a fixture with one example for each class (`node_modules/`, `target/` with `Cargo.toml`, `__pycache__/`, `.git/`, `.DS_Store`, `Cargo.lock`, `api.pb.go`, `app.min.js`, an MIT `LICENSE`, an empty `.gitkeep`)
- WHEN the user runs `teak --format json`
- THEN each entry SHALL carry its class
- AND `vendor`, `build`, `cache`, `lock`, `generated`, `minified`, `boilerplate`, and `empty` entries SHALL be in the chaff line
- AND `vcs` and `editor` entries SHALL be counted only in the footer

#### FR-31: Rule order

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

#### FR-32: Project-kind rules in context

**Happy path:**
- GIVEN `app/Cargo.toml` and `app/target/`
- THEN `app/target/` SHALL be in class `build`

**Failure path:**
- GIVEN `photos/target/` with no `Cargo.toml` next to it
- THEN `photos/target/` SHALL be a normal directory

**Edge case:**
- GIVEN `web/package.json` and `web/dist/`
- THEN `web/dist/` SHALL be in class `build`

#### FR-33: One chaff line

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

#### FR-34: Large chaff first

**Happy path:**
- GIVEN `target/` (1.2 GB) and `src/` (14 MB) in one directory
- WHEN the user runs `teak --color always`
- THEN the chaff line SHALL be the first child row of that directory
- AND its size SHALL have the bold attribute (SGR 1)

**Failure path:**
- GIVEN the chaff is 10 MB and the rest is 14 MB
- THEN the chaff line SHALL be the last child row

#### FR-35: `-A` and `-a`

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

#### FR-36: Chaff controls

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

#### FR-37: Gitignore

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

#### FR-38: Generated markers

**Happy path:**
- GIVEN `a.go` that starts with `// Code generated by protoc-gen-go. DO NOT EDIT.`
- WHEN the user runs `teak`
- THEN `a.go` SHALL be in class `generated`

**Failure path:**
- GIVEN `b.go` with the same marker at byte 3,000
- THEN `b.go` SHALL NOT be in class `generated`

**Edge case:**
- GIVEN `a.go` as above
- WHEN the user runs `teak --detect off`
- THEN `a.go` SHALL NOT be in class `generated`, because teak reads no content

#### FR-39: Minified files

**Happy path:**
- GIVEN `app.js` (200 KB on one line)
- THEN `app.js` SHALL be in class `minified`

**Failure path:**
- GIVEN `lib.js` (200 KB with 80-character lines)
- THEN `lib.js` SHALL NOT be in class `minified`

**Edge case:**
- GIVEN `x.min.js` with 10 bytes
- THEN `x.min.js` SHALL be in class `minified` (name rule)

#### FR-40: Built-in boilerplate templates

**Happy path:**
- GIVEN the teak repo's own `LICENSE` file (MIT, "Copyright (c) 2026 Christian Merkwirth")
- WHEN the user runs `teak`
- THEN the entry SHALL read `LICENSE (MIT)` and be in class `boilerplate`

**Normalization:**
- GIVEN the MIT text with the year 1999 and a different name
- THEN the label SHALL also be `LICENSE (MIT)`

**Failure path:**
- GIVEN the MIT text with one added sentence
- THEN the file SHALL be a normal row, with no label

#### FR-41: User templates

**Happy path:**
- GIVEN `tpl/boilerplate/acme-header.txt` and a file `src/HEADER` with the same normalized text
- WHEN the user runs `teak --templates tpl`
- THEN `src/HEADER` SHALL be labeled `acme-header` and be in class `boilerplate`
- GIVEN `tpl/generated/stub.txt` and a matching file
- THEN that file SHALL be in class `generated`

**Templates only:**
- WHEN the user runs `teak --templates tpl --templates-only`
- THEN the MIT `LICENSE` SHALL NOT get a label

**Round trip:**
- WHEN the user runs `teak --dump-templates out/` (an empty directory)
- AND then runs `teak --templates out --templates-only`
- THEN the labels SHALL be the same as with the built-in table

**Failure paths:**
- WHEN the user runs `teak --templates missing/`
- THEN teak SHALL exit with code 2
- WHEN `out/` already holds a file with a name that `--dump-templates` would write
- THEN teak SHALL NOT overwrite it, and it SHALL exit with code 2

#### FR-42: Bounded reads for content checks

**Happy path:**
- GIVEN a test build with a read counter for each file
- WHEN the user runs `teak --detect full` with all content checks on, and then with all content checks off
- THEN for each file, the bytes read with checks on SHALL be at most the bytes read with checks off plus 65,536
- AND a file that is not read with checks off SHALL NOT be read with checks on

**Edge case:**
- GIVEN a visible 1 MiB text file
- WHEN the user runs `teak --detect full`
- THEN the content checks SHALL read at most the first 64 KiB of it

#### FR-43: Duplicates

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

### E. Series

#### FR-44: Series rows

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

#### FR-45: Special slots

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

#### FR-46: Series row content

**Happy path:**
- GIVEN 10 PNG files of 300, 340, 350, 350, 350, 360, 360, 380, 400, and 410 KB
- THEN the series row SHALL show `10`, the total size, `~355K`, and `png`

**Edge case:**
- GIVEN the same series, where one file is really a JPEG
- THEN the row SHALL show the type mix `png 9 · jpeg 1`

#### FR-47: Uniform directories

**Happy path:**
- GIVEN `photos/` with 95 files `IMG_####.jpg` and 5 other files
- WHEN teak selects `photos/` to open
- THEN `photos/` SHALL show as one row with `jpeg` and the median size

**Failure path:**
- GIVEN 85 such files and 15 other files
- THEN `photos/` SHALL open in the normal way

### F. Type detection

#### FR-48: Fast detection

**Happy path:**
- GIVEN `Makefile`, `x.png` (PNG magic), `noext` (PDF magic), and `a.rs` (Rust text)
- WHEN the user runs `teak --detect fast`
- THEN the types SHALL be `makefile`, `png`, `pdf`, and `rust`

**Edge case:**
- GIVEN `y.png` with JPEG magic
- THEN the type SHALL be `jpeg` (magic bytes win over the extension)

#### FR-49: Magika on visible files

**Happy path:**
- GIVEN `script` with no extension and Python content
- WHEN the user runs `teak`
- THEN the type SHALL be `python`
- AND `--why script` SHALL name `magika` as the source

#### FR-50: Sampled type mix

**Happy path:**
- GIVEN a folded directory with 1,000 `.png` files and 10 `.txt` files
- WHEN the user runs `teak`
- THEN its summary SHALL show `≈` and `png` first
- AND teak SHALL type at most 3 files for each extension in that directory (read counter)

**Edge case:**
- GIVEN 20 files with no extension in a folded directory
- THEN teak SHALL type each of them (within the global cap)

#### FR-51: Global cap

**File cap:**
- GIVEN 10,000 files with no extension in folded directories
- WHEN the user runs `teak`
- THEN teak SHALL type at most 5,000 files with content detection
- AND the summaries SHALL mark the other types as estimated
- AND the exit code SHALL be 0

**Time cap:**
- GIVEN a test build that adds 10 ms to each content read
- WHEN the user runs `teak` on 1,000 files
- THEN content detection SHALL stop after about 2 seconds (± 0.5 s)

**Magika sub-cap:**
- GIVEN 1,000 files with no extension in folded directories
- WHEN the user runs `teak --detect full`
- THEN at most 300 types SHALL come from Magika
- AND the other typed files SHALL use fast detection

#### FR-52: Detection modes

**Happy path:**
- WHEN the user runs `teak --detect off`
- THEN the read counter SHALL be 0 for all files
- AND the types SHALL come from stat, name rules, and the extension
- WHEN the user runs `teak --detect fast`
- THEN no type SHALL come from Magika

**Failure path:**
- WHEN the user runs `teak --detect max`
- THEN teak SHALL exit with code 2

#### FR-53: Mismatches

**Happy path:**
- GIVEN `report.pdf` that holds HTML
- WHEN the user runs `teak`
- THEN its row SHALL read `html ⚠ ext says pdf`

**List mode:**
- WHEN the user runs `teak --mismatches`
- THEN the output SHALL list only the mismatched files, with their paths
- GIVEN a tree with no mismatches
- THEN the output SHALL say `no mismatches`, and the exit code SHALL be 0

**Edge cases:**
- GIVEN `a.yml` (YAML) and `b.jpg` (JPEG)
- THEN neither SHALL get `⚠` (equal type names)
- GIVEN `blob.dat` with any content
- THEN it SHALL NOT get `⚠` (no known type for the extension)
- GIVEN a file where Magika gives a low score
- THEN it SHALL NOT get `⚠`

#### FR-54: Type cache

**Happy path:**
- GIVEN `XDG_CACHE_HOME` points to a temp directory
- AND run 1 typed `x` as `python`
- WHEN the test replaces `x` with PNG content (new mtime) and runs teak again
- THEN run 2 SHALL show `png`

**Cache use:**
- GIVEN no file changed after run 1
- WHEN run 2 starts
- THEN the read counter for content detection SHALL be 0

**Failure path:**
- GIVEN a corrupt cache file
- WHEN the user runs `teak`
- THEN teak SHALL ignore the cache, build a new one, and exit with code 0

**Edge cases:**
- GIVEN a cache written with a different Magika model version
- THEN teak SHALL NOT use those entries
- WHEN the user runs `teak --no-cache`
- THEN the cache directory SHALL NOT be read or written

Note: "changed" means a change in device, inode, size, or mtime (ns).

#### FR-55: Build without Magika

**Happy path:**
- GIVEN a build without the `magika` feature
- WHEN the user runs `teak --detect full`
- THEN teak SHALL use fast detection and exit with code 0
- AND `teak --version` SHALL say `magika: off`

### G. Previews, git, summaries, explanations

#### FR-56: Previews

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

#### FR-57: Git status

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

#### FR-58: Folded-directory summary

**Happy path:**
- GIVEN a folded directory with 2 subdirectories and files of 5 types (rust 50%, toml 20%, md 15%, json 10%, txt 5% by size)
- THEN its row SHALL show the total size, the file count, `2 dirs`, `rust`, `toml`, `md`, `+2 types`, and the newest mtime as an age

**Edge case:**
- GIVEN an empty folded directory
- THEN its row SHALL show `0` and `empty`

#### FR-59: Project kinds

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

#### FR-60: `--why`

**Happy path:**
- WHEN the user runs `teak --why node_modules`
- THEN the output SHALL name the class `vendor`, the rule and its source, the action `fold`, each salience term with its value, and the final decision
- WHEN the user runs `teak --why src/main.rs` and `src/main.rs` is visible
- THEN the output SHALL say `visible` and list the salience terms

**Failure path:**
- WHEN the user runs `teak --why /etc/passwd` and the root is a different tree
- THEN teak SHALL exit with code 2

#### FR-61: `--stats`

**Happy path:**
- WHEN the user runs `teak --stats`
- THEN the output SHALL have the header, a table with type group, file count, and size, and the footer
- AND the output SHALL have no tree rows

### H. Output formats, config, and release

#### FR-62: Default format

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

#### FR-63: Markdown

**Happy path:**
- WHEN the user runs `teak --format md`
- THEN the first line SHALL be `# <root>/ — <size>, <n> files`
- AND the tree SHALL be in a fenced `text` block, in ASCII
- AND the output SHALL have the type table and the line `Folded: …`
- AND a CommonMark parser SHALL parse the output with all fences closed

**Edge case:**
- GIVEN a file name that contains three backticks
- THEN the fence SHALL be longer than any run of backticks in the tree, so the block stays closed

#### FR-64: JSON

**Happy path:**
- WHEN the user runs `teak --format json`
- THEN the output SHALL be valid JSON with a top-level `schema_version`
- AND it SHALL pass validation against the JSON Schema in the repo
- AND it SHALL hold only the visible rows and the summaries
- WHEN the user runs `teak --json-full`
- THEN the number of file entries SHALL equal the file count from an independent walk

#### FR-65: Stable output

**Happy path:**
- GIVEN a fixed fixture and a fixed `SOURCE_DATE_EPOCH`
- WHEN the user runs teak two times with `-j 1` and two times with `-j 8`, in `plain`, `md`, and `json`
- THEN the four outputs of each format SHALL be byte-identical

#### FR-66: Config files

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

#### FR-67: Salience weights

**Happy path:**
- GIVEN the fixture from FR-13, and a config with size weight 0 and recency weight 1
- AND `a/` is changed to the newest mtime, and `z/` to an mtime 3 years ago
- WHEN the user runs `teak -n 12`
- THEN `a/` SHALL open before `z/`

#### FR-68: Install

**Happy path:**
- GIVEN the release pipeline
- WHEN it builds for Linux x86_64, Linux aarch64, macOS x86_64, and macOS arm64
- THEN each target SHALL produce a binary that prints its version with `teak --version`
- AND `cargo install teak` (from the package) SHALL work on Linux and macOS CI
- AND `brew install <tap>/teak` SHALL work on macOS CI

#### FR-69: Completions and man page

**Happy path:**
- GIVEN the release archive
- THEN it SHALL contain bash, zsh, and fish completions and `teak.1`
- AND `man ./teak.1` SHALL render with no error
- AND bash completion for `--fo` SHALL offer `--focus`, `--fold`, `--follow`, and `--format`

#### FR-70: Exit codes

**Happy path:**
- GIVEN a readable fixture
- WHEN the user runs `teak`
- THEN the exit code SHALL be 0

**Failure paths:**
- GIVEN a fixture with an unreadable directory
- THEN the exit code SHALL be 1
- WHEN the user runs `teak --bogus`
- THEN the exit code SHALL be 2

### I. Performance

For all of these: the fixture generator uses a fixed seed. Each case runs with
`hyperfine` (3 warmup runs, then 10 runs). The test checks the median on the
reference machine. Until the phase-0 report, a failed target is a warning, not a
failure.

#### FR-71: 10k files

- GIVEN a generated tree with 10,000 files and a warm teak cache
- WHEN the test runs `teak`
- THEN the median time SHOULD be less than 150 ms

#### FR-72: 100k files

- GIVEN a generated tree with 100,000 files and a warm OS cache
- WHEN the test runs `teak --detect full --no-cache`
- THEN the median time SHOULD be less than 1 s

#### FR-73: 1M files

- GIVEN a generated tree with 1,000,000 files and a warm OS cache
- WHEN the test runs `teak --no-cache`
- THEN the median time SHOULD be less than 5 s
- AND the peak RSS SHOULD be less than 300 MB

#### FR-74: Magika on visible rows

- GIVEN a tree in which about 100 files are visible
- WHEN the test measures only the detection stage (from the timing output of the test build)
- THEN it SHOULD take less than 300 ms

#### FR-75: Warm type cache

- GIVEN the FR-72 tree and a teak cache from a previous run
- WHEN the test runs `teak --detect full`
- THEN the detection stage SHOULD take less than 5% of the total time
- AND the read counter for content detection SHOULD be 0

---

## 6. Constraints

### In Scope

- All v1 features in phases P1–P4, as listed in §4.
- A phase-0 test report on the build (`magika` + `ort` on Linux and macOS), binary size, Magika time per file, and walk speed on 1M files. The report names the reference machine and re-checks FR-71–FR-75. After the report, these FRs become SHALL.
- Linux (x86_64, aarch64) and macOS (x86_64, arm64).
- The output formats `tty`, `plain`, `md`, and `json`.
- User and project config files.

### Out of Scope

- An interactive TUI — broot already does this well.
- File operations (copy, move, delete) — teak is read-only by design.
- Windows support — we can add it after v1.
- LLM-based summaries and all network use — teak runs fully offline.
- Full archive listing or extraction — teak only peeks inside (entry count, inner type).
- Content search — `rg` and `grep` already do this.
- Watch mode and live updates — teak is a one-shot command.
- Submission to Homebrew core — after v1 is stable.

### Prohibitions

- SHALL NOT write anything into the scanned tree, including temp files. The type cache SHALL live only in the user cache directory — the tree may be someone else's data or a read-only mount.
- SHALL NOT open FIFOs, sockets, or devices, and SHALL NOT follow symlinks by default — opening a FIFO can block, and symlinks can loop or leave the tree.
- SHALL NOT use the network, send telemetry, or download the Magika model at runtime. The model SHALL be built into the binary — users and agents must be able to trust teak offline.
- SHALL NOT hide any entry without counting it — silent hiding breaks trust in the view (G3).
- SHALL NOT print more lines than the budget — agents and scripts rely on the limit (G1).
- SHALL NOT run any program or file from the scanned tree, including git hooks, filters, and `core.fsmonitor` — a hostile repo can make a plain `git status` run a command, and teak runs on unknown repos, often for agents.
- SHALL NOT show previews of likely secret files (`.env*`, `*.pem`, `*.key`, `id_rsa*`, `credentials*`, `.netrc`, and files whose content is a private key) — agents paste teak output into LLM prompts.
- SHALL NOT change the `plain`, `md`, or `json` format in an incompatible way without a schema or version change and a changelog entry — agent prompts and tests depend on it.
- SHALL NOT need root rights.

### Testing Approach

- TDD — write a failing test first, then make it pass. The acceptance criteria map
  to fixture trees, `insta` snapshot tests, and `proptest` invariants (FR-12 budget
  limit, FR-21 every entry counted, FR-65 stable output). Performance targets use
  `hyperfine` and `criterion`, and stay warnings until the phase-0 report.
