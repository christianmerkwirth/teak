# teak — Product Requirements Document

Status: draft v0.2 · 2026-10-24 · Owner: Christian Merkwirth · License: MIT

> **Name.** We wanted `oak`, but the name is taken twice. Homebrew core ships `oak`
> (the Oak programming language), so `brew install oak` puts a different `oak`
> binary on macOS. The `oak` crate on crates.io is also taken (a PEG parser, 2021).
> **teak** is free on crates.io, Homebrew, and Debian/Ubuntu. Teak is a strong,
> durable hardwood, so the idea of "a strong tree" stays.

---

## 1. Summary

teak prints a directory tree as text. It fits the tree into a fixed number of lines
and puts the most useful information into each line. It joins three tools:

- `tree` — the structure
- `ls -lah` — the size, age, and permissions of each entry
- Google Magika — the real file type, from the content and not from the extension

teak opens the parts of the tree that matter. It folds the rest into short summary
lines, so you can still see what each folded part contains. It finds minor and
boilerplate files ("chaff") and moves them out of the way.

teak has two kinds of users: people at a terminal, and LLM agents that need a fast
and cheap picture of a directory. Output is always plain text. Colors are optional.
Markdown and JSON are available.

## 2. Problem

- `tree` on a real project prints thousands of lines. The important parts are
  lost in `node_modules/`, build output, and caches.
- `tree -L 2` cuts at a fixed depth. It hides deep folders that matter and shows
  shallow folders that do not.
- `eza --tree --long`, `erdtree`, and `lstr` add columns, but they still print
  every entry.
- `broot` folds well, but it is an interactive program. You cannot pipe it to a file
  or give it to an agent.
- All of these tools guess the file type from the extension. They cannot see that
  `report.pdf` is really HTML, or what is inside a file with no extension.
- An LLM agent that runs `find` or `tree` uses thousands of tokens and still does
  not know where the code is, what is generated, or what changed.

## 3. Goals and non-goals

### Goals

| # | Goal | Measure |
|---|---|---|
| G1 | A big tree fits on one screen and stays useful. | Output never has more lines than the budget (hard rule, tested). |
| G2 | Show the real file types. | Magika labels on all visible files. Type mix on all folded directories. |
| G3 | Keep chaff out of the way, but never hide it silently. | Every folded or hidden entry is counted in a visible line. |
| G4 | Fast on large trees. | See §9. |
| G5 | Good for people and for agents. | Same engine. Output formats: tty, plain, md, json. |
| G6 | Explain every decision. | `--why PATH` tells why an entry is open, folded, or hidden. |

### Non-goals (v1)

- No interactive TUI. broot already does this well.
- No file operations (copy, delete, move). teak is read-only.
- No network access, no telemetry, no LLM calls. teak runs fully offline.
- No Windows support in v1. Linux and macOS only.

## 4. Users and use cases

**P1 — Developer at a terminal.** Opens an unknown repo or an old project folder.
Wants to know in 5 seconds: what is here, where the bulk is, what changed, and
what is generated.

**P2 — Person who cleans a disk.** Has a home folder or a data disk with years of
files. Wants to see the big parts, the old parts, and the odd files (wrong
extension, duplicates, huge single files).

**P3 — LLM agent.** Gets a repo path. Wants a compact, stable map of the tree in a
few hundred tokens: structure, languages, entry points, what to ignore.

**P4 — Data engineer.** Has folders with thousands of similar files
(`part-00000.parquet`, `IMG_0001.jpg`). Wants one line per series, not 10,000.

Typical commands:

```
teak                         # current directory, fits the terminal
teak ~/data -n 30            # 30 lines
teak . --focus src/api       # spend most lines on src/api, keep context around it
teak . --format md -n 120    # Markdown for an LLM prompt
teak . --why vendor/         # why is vendor/ folded?
teak . -A                    # also show hidden entries (dotfiles, .git/)
teak . -a                    # show everything, no chaff folding
```

## 5. Output design

### 5.1 Example (tty, colors not shown)

```
shop/  2.1G · 48,213 files · rust crate + node pkg · git main ↑2 · 14 changed
├── src/                  14M   812  rust 70% ts 25% toml 5%        2h  M3
│   ├── main.rs           12K        rust      fn main() {          2h  M
│   ├── api/             2.1M   140  rust                           1d  M2
│   └── … 9 dirs         9.8M   660  rust 64% ts 36%                3d
├── assets/              1.4G  3.2k  png 80% jpeg 18% svg 2%       5mo
│   └── IMG_[0001–3188].png  1.1G  3,188  png 1920×1080 ~350K     5mo
├── data/                640M    12                                 3w
│   ├── dump.sql.gz      610M        gzip → sql                     3w
│   ├── report.pdf       220K        html  ⚠ ext says pdf           3w
│   └── … 10 files        30M    10  csv 8 · json 2                 3w
├── deploy.sh            2.1K  rwx   shell     #!/usr/bin/env bash  1y
├── README.md            4.0K        markdown  # Shop backend      2mo
└── ▪ chaff  node_modules/ 180M 44k · target/ 1.2G · Cargo.lock · LICENSE (MIT) · +4
── 15 of 100 lines · 48,213 files · folded 45,130 (vendor 44,190 · build 912 · boilerplate 28)
```

### 5.2 Row kinds

| Row kind | When | Content |
|---|---|---|
| Header | Always, first line | Root path, total size, file count, project kind, git branch and state. |
| Entry | A visible file, dir, or symlink | Columns from §5.3. |
| Overflow | An open directory has more children than it can show | `… N dirs`, `… N files`, or both, with total size and type mix. |
| Series | Four or more siblings share one name pattern (§7) | `IMG_[0001–3188].png`, count, type, typical size. |
| Chaff | A directory has chaff entries (§6) | One dim line that lists the chaff in short form. |
| Footer | Always, last line | Lines used, total files, folded counts by chaff class. |

### 5.3 Columns

Default columns are chosen for density. `-l` shows the full `ls -lah` set.

| Column | Default | `-l` | Notes |
|---|---|---|---|
| Tree glyphs + name | yes | yes | Long names are cut in the middle: `very_long…name.txt`. Names that are not UTF-8 are escaped. |
| Size | yes | yes | Human-readable (`1.4G`). A directory shows the total of its contents. `--bytes` and `--si` change the units. Hard links are counted once. |
| Count | dirs only | dirs only | Number of files below. Compact: `3.2k`. |
| Type | yes | yes | Magika label (`rust`, `png`, `pdf`). Type mix for directories. `≈` marks an estimate from a sample. |
| Preview | yes | yes | See §8.4. Cut to the width that is left. |
| Permissions | only if unusual | always | Unusual: executable, setuid/setgid, world-writable, or a different owner. Default form is short (`rwx`, `suid`, `o+w`). |
| Owner / group | no | yes | |
| Age | yes | yes | Relative mtime: `2h`, `3d`, `5mo`, `1y`. `--time=iso` gives absolute dates. |
| Git | if in a repo | yes | `M`, `A`, `D`, `?`, `!`. A directory shows the count of changed files (`M3`). |
| Flags | when needed | when needed | `⚠` type mismatch, `⊘` no access, `↻` symlink loop, `⇢` symlink target. |

The table aligns to the terminal width. Columns drop out in this order when space is
short: owner/group, preview, git, age, type mix detail.

### 5.4 Colors

- Colors show the **type group**: code, text/doc, data, image, audio/video,
  archive, executable, binary, unknown. Directories are bold.
- Chaff rows and overflow rows are dim.
- Warnings (`⚠`, `⊘`) are yellow or red.
- Size can get a heat color (larger = brighter). This is off by default.
- Color is never the only signal. Every colored item also has a label or a symbol.
- `--color=auto|always|never`. teak respects `NO_COLOR` and `CLICOLOR_FORCE`.
  `LS_COLORS` changes the name colors only.
- `--ascii` uses `|--` and `` `-- `` in place of box-drawing glyphs and `...` in place of `…`.

### 5.5 Output formats

| Format | Use | Details |
|---|---|---|
| `tty` (default on a terminal) | People | Colors, Unicode glyphs, width from the terminal. |
| `plain` (default in a pipe) | Scripts, logs, agents | No ANSI codes. Fixed width (`--width`, default 120). |
| `md` | LLM prompts, notes, issues | See below. |
| `json` | Tools | The visible tree plus summaries. `--json-full` gives every entry. The schema has a version. |

**Markdown format:**

````md
# shop/ — 2.1G, 48,213 files
Rust crate + Node package · git `main` · 14 changed · scanned 2026-10-24 11:53

```text
<the tree, plain format, ASCII by default>
```

| Group | Files | Size |
|---|---:|---:|
| code | 1,204 | 18M |
| image | 3,201 | 1.4G |

Folded: vendor 44,190 · build 912 · boilerplate 28. Run `teak -a` to show all.
````

**Stable output for agents.** In `plain`, `md`, and `json`, the same tree gives the
same output. Sort order is stable. Relative ages use the scan time in the header.
`--time=iso` removes relative ages fully.

## 6. Chaff: minor and boilerplate content

This is the main way to keep the view clean. The idea: **sort entries by how much
they tell you, not only by how big they are.** Chaff is not lost. It is folded into
one line for each directory and counted in the footer.

### 6.1 Chaff classes

teak puts each entry into zero or one chaff class. The rules come from four
sources, in this order (the first match wins):

1. **The user:** `.teak.toml` in the project, `~/.config/teak/config.toml`, and
   CLI flags.
2. **The project:** `.gitattributes` with `linguist-generated` and
   `linguist-vendored`. GitHub uses the same attributes, so many repos already
   mark their generated and vendored files.
3. **Project-kind rule packs:** teak finds the project kind from marker files
   (`Cargo.toml` → rust, `package.json` → node, `pyproject.toml` → python,
   `go.mod`, `pom.xml`, `*.xcodeproj`, …). Each pack knows its own build and
   vendor folders. Example: `target/` is build output only next to `Cargo.toml`.
   In a folder without it, `target/` is a normal folder.
4. **Content checks** (see 6.2) and global name rules.

| Class | Examples | Default action |
|---|---|---|
| `vendor` | `node_modules/`, `vendor/`, `.venv/`, `site-packages/`, `Pods/` | fold |
| `build` | `target/`, `dist/`, `build/`, `.next/`, `*.o`, `*.class`, `*.pyc` | fold |
| `cache` | `__pycache__/`, `.cache/`, `.pytest_cache/`, `.gradle/` | fold |
| `vcs` | `.git/`, `.hg/`, `.svn/` | hide (with `-A`: one folded line with the repo size) |
| `editor` | `.idea/`, `.vscode/`, `*.swp`, `*~`, `.DS_Store`, `Thumbs.db` | hide |
| `lock` | `Cargo.lock`, `package-lock.json`, `yarn.lock`, `poetry.lock` | fold |
| `generated` | files with "DO NOT EDIT", `@generated`, or "Code generated"; `*_pb2.py`, `*.pb.go` | fold |
| `minified` | `*.min.js`, `*.map`, and text files with an average line length > 500 | fold |
| `boilerplate` | a standard `LICENSE`, `.gitignore`, `.editorconfig`, `CODE_OF_CONDUCT.md`, `.prettierrc` | fold |
| `empty` | 0-byte files, `.gitkeep`, empty `__init__.py`, empty dirs | fold |
| `duplicate` | a file with the same content as a file that is already visible | fold, marked `= path` |

Actions:

- **show:** a normal row.
- **dim:** a normal row in dim color, with a low score (it is removed first when lines are short).
- **fold:** goes into the directory's chaff line.
- **hide:** counted only in the footer.

Hidden entries are dotfiles and the classes with the action `hide` (`vcs`,
`editor`). The flags work like `ls -A` and `ls -a`:

| Flag | Hidden entries | Folded chaff |
|---|---|---|
| (none) | hidden, counted in the footer | folded |
| `-A, --almost-all` | shown, but folded into the chaff line (`.git/` shows its size) | folded |
| `-a, --all` | shown as normal rows | shown as normal rows, no gitignore |

### 6.2 Content checks

Name rules do not catch everything. teak also checks content, but only on files it
already reads for type detection (no extra disk reads):

- **Generated markers.** teak looks for known markers in the first 2 KB: `DO NOT
  EDIT`, `@generated`, `Code generated by`, `autogenerated`, and similar.
- **Minified code.** The average line length is very high, or there are almost no
  line breaks in a large text file.
- **Known boilerplate.** teak normalizes the text (removes years, names, and
  whitespace) and hashes it. It compares the hash with a small built-in table:
  common license texts (MIT, Apache-2.0, BSD, GPL), the standard `.gitignore`
  templates, and the default files from common project generators. A match gets a
  short label, for example `LICENSE (MIT)`. Users get the summary and do not need
  to open the file.

  The table is built into the binary. Users can add their own templates on
  request: `--templates DIR` (or `templates = ["DIR", …]` in the config) loads
  text files from a directory. teak normalizes and hashes each file the same way.
  The file name without extension becomes the label, and a subdirectory name sets
  the class (`boilerplate/` by default, `generated/`). `--templates-only` uses the
  user files in place of the built-in table. `teak --dump-templates DIR` writes the
  built-in templates to a directory, as a start for your own set.
- **Duplicates.** Files with the same size are hashed (head + tail first, the full
  file only on a match). A duplicate of a visible file is folded and points to
  the original. Duplicate checks are on with `--dupes`, because they cost I/O.

### 6.3 The chaff line

Each directory that has chaff gets at most **one** chaff line, at the end of its
children:

```
└── ▪ chaff  node_modules/ 180M 44k · target/ 1.2G · Cargo.lock · LICENSE (MIT) · +4
```

- Large chaff entries come first, with their size.
- Small entries show by name only.
- `+N` counts the rest.
- If the chaff in a directory is larger than everything else in it, the chaff
  line comes first and its size is in bold. A 1.2G `target/` is not "minor" for a
  person who wants free disk space.

### 6.4 Controls

```
-A, --almost-all        show hidden entries (dotfiles, .git/), folded into the chaff line
-a, --all               show all: no chaff folding, no hidden entries, no gitignore
--chaff=LEVEL           show | dim | fold | hide   (sets all classes; default: per class)
--show=CLASS,…          example: --show=lock,generated
--fold=CLASS,…
--hide=CLASS,…
--no-gitignore          also show entries that .gitignore excludes
--templates DIR         add your own boilerplate templates (see 6.2)
--why PATH              tell which rule matched and the score for PATH
```

`.gitignore` entries are a separate source of chaff. By default, gitignored
entries are folded (not hidden), because a gitignored 40G `data/` folder is
important to see.

## 7. Series: compressing similar siblings

Many directories hold long series of similar names. teak shows each series as one
row.

**Method:**

1. Split each name into runs of digits and runs of other characters:
   `IMG_0042.jpg` → `IMG_`, `0042`, `.jpg`.
2. The **shape** is the name with each digit run replaced by a slot:
   `IMG_#.jpg`.
3. Siblings with the same shape and the same extension form a group. A group of
   4 or more (set with `--series-min`) becomes one series row.
4. The row shows the range of the slot that changes: `IMG_[0001–3188].jpg`. If
   numbers are missing, it says so: `3,150 of 3,188`.
5. Special slots: dates (`2024-05-01`), hex hashes (16 or more hex characters →
   `[hash]`), and UUIDs → `[uuid]`.
6. The row shows count, total size, typical size (median), and the type. If the
   types differ inside the series, it shows the type mix.

**Uniform directories.** If more than 90% of the files in a directory share one
type and one shape, teak shows the directory as one row even when it is open:
`photos/2019/  12G  4,812  jpeg ~2.5M 4032×3024`.

## 8. File type detection

### 8.1 Pipeline

For each file that teak must type, these steps run in order. The first step that
gives an answer wins:

1. **Stat:** directory, symlink, FIFO, socket, device. teak **never opens** FIFOs,
   sockets, or devices for reading, because opening one can block or change state.
2. **Exact name rules:** `Makefile`, `Dockerfile`, `LICENSE`, lock files, and
   similar. These are fast and sure.
3. **Magika:** a content-based model (about 200 types). It reads only small parts
   at the start, middle, and end of the file. It gives a label, group, MIME type,
   description, and score.
4. **Fallback** when Magika is not available or fails: magic bytes (`infer` crate),
   then the extension.

The result is a `TypeInfo`: label, group, mime, is_text, score, source
(name / magika / magic / ext), and `estimated` (true if the result comes from a
sample).

### 8.2 What gets typed

Running Magika on all files of a 1M-file tree costs too much. teak types:

- **All visible files** (at most the line budget, usually < 200).
- **A sample for each folded directory**, stratified by extension. For each
  (directory, extension) bucket, teak types up to `k = 3` files and applies the
  result to the whole bucket. Files with no extension are always typed, up to a cap.
  Type mix numbers from a sample are marked `≈`.
- **A global cap** (default 5,000 files or 2 seconds, whichever comes first). After
  the cap, teak uses the extension only and marks the result as estimated.

`--detect=off|fast|full`: `off` = stat and name only, `fast` = magic bytes and
extension, `full` = Magika (the default).

### 8.3 Mismatches

When the Magika label (with a high score) does not fit the extension, teak marks
the row with `⚠` and both types: `report.pdf  html ⚠ ext says pdf`. This finds
renamed files, broken downloads, and disguised files. `--mismatches` lists only
these files.

### 8.4 Previews

A preview is a short hint, cut to the width that is left on the row. `--preview=off|short|full`.

| Type | Preview |
|---|---|
| Source code | The first meaningful line. teak skips blank lines, license headers, and SPDX lines. |
| Markdown | The first heading. |
| JSON / YAML / TOML | The top-level keys: `{name, version, scripts, …}`. |
| CSV / TSV | The header row and an estimated row count. |
| Image | Width × height (from the header only). |
| gzip / zstd / xz | The inner type, from the first 4 KB after decompression: `gzip → sql`. |
| zip / jar | The number of entries (from the central directory). |
| ELF / Mach-O / PE | Architecture: `ELF x86-64`, `Mach-O arm64`. |
| Symlink | `⇢ target`. A broken link is marked. |
| Other binary | The Magika description. |

Previews read no more bytes than type detection already read, except the image
header and the zip central directory (a few KB each).

## 9. Selection: how teak spends the line budget

### 9.1 The budget

- `-n N` sets the budget. The default is 100 lines in all formats.
  `-n auto` fits the terminal height (the height minus 2). The config file can
  change the default.
- The header, the footer, and all chaff lines count against the budget.
- **Hard rule:** the output never has more lines than the budget. A property test
  checks this.

### 9.2 Salience score

Each entry gets a **salience** score from 0 to 1. It answers: "How much does
this row tell the user?" The weights are in the config file. `--why` prints all
terms.

| Term | High when |
|---|---|
| Size share | The entry is a large part of its parent (log scale). |
| Recency | The entry changed more recently than most of the tree. |
| Git | The entry is modified, added, or untracked. A directory scores by its changed files. |
| Rarity | The type is rare in this directory (one `.py` among 3,000 `.png`). |
| Role | Entry points and key files: `README*`, `main.*`, `Makefile`, `Dockerfile`, a project manifest. |
| Anomaly | Type mismatch, a huge single file, unusual permissions, no access. |
| Chaff penalty | The entry is in a chaff class (dim), or it is gitignored. |
| Focus | The entry is on the `--focus` path or near it. |

Sort keys (`--sort=salience|name|size|age|type`) change only the display order
inside a directory. They do not change what teak selects. Default display order:
directories first, then by name. This keeps the output easy to scan.

### 9.3 Expansion algorithm

1. Walk the full tree. Collect stat data and aggregates (size, count, newest
   mtime, extension histogram) for each directory.
2. Start with the root open. All other directories are closed.
3. Keep a priority queue of closed directories. The key is **gain ÷ cost**:
   - gain = the sum of the salience of the rows that opening would show,
     minus the salience of the summary row that it replaces.
   - cost = the number of extra lines.
4. Open the best directory. Inside it, show the children with the highest
   salience. Put the rest into one overflow row. Each open directory uses at least
   two lines (one child + one overflow), unless it has only one child.
5. Repeat until the next step does not fit in the budget.
6. If a directory has exactly one child that is a directory, join them into one
   row: `src/main/java/com/acme/` (the same as GitHub).
7. Run type detection and previews only for the rows that are now visible, plus
   the samples for the folded directories.

The algorithm is greedy and runs in O(n log n). This is fast enough for 1M entries.

## 10. Directory summaries

A folded directory, an overflow row, and the header all use the same summary:

- total size, file count, directory count
- type mix: the top 3 type labels by size, then `+N types`
- age range: the newest mtime (and the oldest with `-l`)
- git: the number of changed files
- **project kind**, when teak finds marker files: `rust crate`, `node pkg`,
  `python pkg`, `git repo`, `venv`, `photos` (mostly images with EXIF),
  `media`, `dataset` (mostly csv/parquet/json series)

## 11. CLI reference (v1)

```
teak [OPTIONS] [PATH]...

Layout
  -n, --lines N|auto       line budget (default 100; auto = terminal height - 2)
  -L, --depth N            never open deeper than N
      --focus PATH         spend most of the budget on PATH, keep the context around it
  -l, --long               all columns (ls -lah style)
      --sort KEY           salience | name | size | age | type   (display order)
      --series-min N       smallest group that becomes a series row (default 4)

Content
      --detect MODE        off | fast | full                       (default full)
      --preview MODE       off | short | full                      (default short)
      --mismatches         list only files whose content does not match the extension
      --dupes              find duplicate files

Filtering
  -A, --almost-all         show hidden entries (dotfiles, .git/), folded
  -a, --all                show everything
      --no-gitignore       do not use .gitignore rules
      --templates DIR      add boilerplate templates from DIR
      --templates-only     use only the templates from --templates
      --dump-templates DIR write the built-in templates to DIR
      --chaff LEVEL        show | dim | fold | hide
      --show/--fold/--hide CLASS,…
  -x, --one-file-system    do not cross into other file systems
      --follow             follow symlinks (off by default)

Output
      --format FMT         tty | plain | md | json   (default: tty on a terminal, else plain)
      --json-full          JSON with every entry
      --color WHEN         auto | always | never
      --ascii              ASCII glyphs only
      --width N            output width (default: the terminal width, or 120)
      --time MODE          relative | iso
      --bytes, --si        size units
      --no-git             no git status

Help
      --why PATH           explain the decision for PATH
      --stats              print only the header, the type table, and the footer
      --no-cache           do not read or write the type cache
  -j, --threads N
```

Exit codes: `0` success, `1` success with errors (for example, no access to some
directories), `2` usage error.

Config: `~/.config/teak/config.toml` (the user) and `.teak.toml` (the project,
found in the root path or its parents). CLI flags override both. The config holds
defaults, chaff rules, salience weights, and colors.

## 12. Non-functional requirements

### 12.1 Performance (targets, to check in the phase-0 test)

| Case | Target |
|---|---|
| 10k files, warm cache | < 150 ms |
| 100k files, warm OS cache, `--detect=full` | < 1 s |
| 1M files, warm OS cache | < 5 s, < 300 MB memory |
| Magika on visible rows (≈ 100 files) | < 300 ms |
| Second run with the type cache | the cost of type detection is close to zero |

Methods: a parallel walk (`ignore` crate), compact nodes in an arena (no `PathBuf`
for each node), Magika on a thread pool with one shared `Session`, and lazy git
status (only for directories that we show).

### 12.2 Type cache

- Location: `$XDG_CACHE_HOME/teak/` (Linux), `~/Library/Caches/teak/` (macOS).
- Key: device, inode, size, mtime (ns), and the Magika model version.
- A model upgrade makes old entries invalid.
- The cache has a size limit and drops the oldest entries first. It must never
  give a wrong type for a changed file.

### 12.3 Safety and robustness

- Read-only. teak never writes to the scanned tree.
- It never opens FIFOs, sockets, or devices.
- It does not follow symlinks by default. With `--follow`, it detects loops.
- "No access" shows as `⊘` and continues. It does not stop the scan.
- It handles names that are not UTF-8, very long names, and very deep trees
  without crashing.
- Pseudo-filesystems (`/proc`, `/sys`) are not read for content.
- On a slow network filesystem, the global detection time cap (§8.2) limits the wait.

### 12.4 Distribution

- One binary for Linux (x86_64, aarch64; glibc and musl if Magika's runtime
  permits) and macOS (x86_64, arm64).
- Channels: `cargo install teak`, GitHub releases (cargo-dist), a Homebrew tap,
  and later Homebrew core.
- Magika is a cargo feature (`magika`, on by default). A build without it uses
  the fast fallback. The ONNX Runtime link method (static or bundled) is set in
  phase 0.
- Shell completions (bash, zsh, fish) and a man page come from the clap definitions.

## 13. Architecture (outline)

```
walk ──▶ tree arena ──▶ aggregate ──▶ classify (chaff, project kind)
                                         │
                                         ▼
                              select (budget, salience)
                                         │
                       ┌─────────────────┼──────────────────┐
                       ▼                 ▼                  ▼
               detect types       previews            git status
               (visible+sample)   (visible)           (visible dirs)
                       └─────────────────┼──────────────────┘
                                         ▼
                              layout (columns, width)
                                         ▼
                          render: tty | plain | md | json
```

Crates (to confirm in phase 0): `ignore`, `magika` + `ort`, `infer`, `clap`,
`anstream`/`owo-colors`, `gix` (git status), `imagesize`, `flate2`/`zstd`/`xz2`
(peeks only), `zip` (central directory only), `serde`/`serde_json`, `redb` or a
simple file format for the cache, `insta` and `proptest` for tests.

Modules: `walk`, `tree`, `aggregate`, `chaff`, `series`, `detect`, `preview`,
`git`, `select`, `layout`, `render/{tty,plain,md,json}`, `config`, `cache`, `cli`.

## 14. Testing

- **Snapshot tests** (`insta`) for each output format on generated fixture trees:
  a rust repo, a node repo with `node_modules`, a photo folder, a data lake with
  series, a tree with odd permissions and broken links.
- **Property tests:** the line budget is never exceeded. Every entry is either
  visible, folded into a counted line, or hidden and counted in the footer. Totals
  are correct.
- **Detection tests:** a corpus of small files with wrong and missing extensions.
- **Benchmarks** (`hyperfine`, `criterion`) on the Linux kernel source, a large
  `node_modules`, and a synthetic tree with 1M files. We compare with `tree`,
  `eza --tree`, and `erd`.
- **CI** on Linux and macOS.

## 15. Milestones

| Phase | Content | Exit test |
|---|---|---|
| 0. Test build (½–1 day) | Build `magika` + `ort` on Linux and macOS. Measure binary size, time for each file, and walk speed on 1M files. Choose the ONNX link method. | Numbers in a short report. A go/no-go for each target in §12.1. |
| 1. Core | walk, arena, aggregates, budget selection, overflow rows, chaff (names + gitignore + rule packs), series rows, tty and plain output, fast type detection. | A useful view of the Linux kernel tree in 40 lines. |
| 2. Types | Magika, sampling, type mix, mismatches, type cache, content chaff checks (generated, minified, boilerplate hashes). | Detection corpus passes. 100k-file target met. |
| 3. Context | Previews, git status, `--focus`, `--why`, project kinds, `-l`. | Snapshot tests for all row kinds. |
| 4. Agents and release | md and json formats, stable output, config files, completions, man page, release pipeline, Homebrew tap. | `cargo install teak` and `brew install` work on both OSes. |

## 16. Risks

| Risk | Effect | Plan |
|---|---|---|
| ONNX Runtime makes builds hard or the binary large | Users cannot install it easily | Feature flag. Prebuilt binaries. Measure in phase 0. |
| Magika is slow on a cold disk | Slow first run | Sampling, time cap, cache, visible rows first. |
| Salience picks the "wrong" rows | Users do not trust the view | `--why`, weights in config, `--sort`, `-a`. Tune on real trees. |
| Chaff rules hide something important | A user misses a file | Fold instead of hide by default. Always count. Large chaff is bold. |
| Output changes between versions | Agent prompts and tests break | Versioned JSON schema. Stable plain/md format with a changelog. |

## 17. Decisions

| # | Question | Decision |
|---|---|---|
| D1 | Name | **teak**. The repo folder is renamed from `oak` to `teak`. |
| D2 | Line budget | A flag, `-n N`. The default is 100 lines. `-n auto` fits the terminal. |
| D3 | `.git/` and other hidden entries | Hidden by default. `-A` shows them, folded (`.git/` with its size). `-a` shows all. |
| D4 | Boilerplate templates | Built into the binary. Users can load more from files with `--templates DIR`. |
| D5 | License | MIT. |
