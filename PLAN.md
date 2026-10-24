# teak — plan (research notes)

> Research notes. The current spec is [.sddw/teak/PRD.md](.sddw/teak/PRD.md). Working name is now **teak**, because `oak` is taken on Homebrew and crates.io.

A command-line tool that shows a large directory tree in a small, information-rich
view. It joins the ideas of `tree`, `ls -lah`, and Google Magika (content-based file
type detection).

## 1. What already exists

| Tool | Tree | ls-style columns | Content type detection | Collapses big dirs | Previews | Notes |
|---|---|---|---|---|---|---|
| `tree` | yes | partial (`-ph`) | no | no | no | Floods the screen on big trees. |
| `eza --tree --long --git` | yes | yes | no (extension only) | no | no | Best "ls + tree" today. |
| `lsd --tree` | yes | yes | no | no | no | Like eza. |
| `erdtree` (`erd`) | yes | yes (`--long`) | no | no (depth, prune) | no | Disk usage, line counts, du-style layouts. |
| `broot` | yes | sizes, dates, git | no | yes (hides "less relevant" rows) | in TUI only | Interactive. Good collapse idea, but not a one-shot printout. |
| `lstr` | yes | perms, sizes, git | no | no | no | Fast and small. |
| `dust`, `dua`, `ncdu` | partial | size only | no | yes (by size) | no | Disk usage only. |
| `smart-tree` (`st`) | yes | yes | no (extension) | partial | no | Many formats, AI/MCP focus, noisy project. |
| `magika` CLI | no | no | yes (ML, ~99%) | no | no | Flat list of types only. |

**Verdict:** No tool joins all three ideas. No tool fits a big tree into a fixed line
budget and still shows *what* is inside the hidden parts. That is the gap oak fills.

## 2. Core idea: a line budget

Most tree tools print every entry and stop at a depth. oak gets a **line budget**
(default: terminal height, or `-n 80`). It spends those lines where the information is.

1. Walk the full tree. Collect stat data only (fast, parallel).
2. Give each directory a score: size, file count, recent changes, git changes, and
   "surprise" (an odd file type or a large single file).
3. Expand directories with the best score first until the budget is used.
4. Every directory that stays closed gets **one summary line**: file count, size,
   type mix, age range.
5. Run content type detection only on the files that you can see, and on a sample of
   files for the summaries. This keeps a 1M-file tree fast.

## 3. Output sketch

```
~/repos/shop            2.1G  48,213 files  ▕code 61% data 22% img 9% other 8%▏  2h
├── src/                 14M     812  rust 70% ts 25% toml        2h  M3
│   ├── main.rs          12K  rust     "fn main() {"             2h  M
│   ├── api/             2.1M    140  rust                        1d
│   └── … 9 more dirs    9.8M    660  rust 64% ts 36%             3d
├── assets/              1.4G  3,201  png 80% jpeg 18% svg
│   └── IMG_0001…IMG_3188.png  1.1G  3,188 files  1920×1080       5mo
├── data/                640M     12
│   ├── dump.sql.gz      610M  gzip → sql                        3w   ⚠ large
│   └── … 11 more files   30M      11  csv 9, json 2
├── node_modules/ ⊘      180M 44,190  (ignored, collapsed)
├── deploy.sh       -rwx  2K  shell    "#!/usr/bin/env bash"      1y
└── README.md             4K  markdown "# Shop backend"           2mo
```

Rules for the columns:

- **Size** is human-readable. Directories show the total. An optional bar shows the share of the parent.
- **Count** shows only for directories.
- **Type** comes from the Magika label. A short type mix shows for directories.
- **Preview** is a short hint: the first line for text, `W×H` for images, the entry count for archives, the duration for media.
- **Age** is relative (`2h`, `3w`, `1y`).
- **Git** shows status (`M`, `A`, `?`). A directory shows the count of changed files.
- **Permissions** show only when they are unusual: executable, world-writable, setuid, or not owned by you. `-l` shows all columns, as in `ls -lah`.

## 4. Feature list, by value

### Must have (MVP)
- Fast parallel walk. Respect `.gitignore` and hidden files (as ripgrep does).
- Line budget with score-based expand and collapse (`-n`, `--depth`).
- One summary line for each closed directory (count, size, type mix).
- **Sibling grouping:** many similar names (`IMG_0001…IMG_3188.png`, `part-00000…`) become one row.
- Colors by type group: code, text/doc, data, image, audio/video, archive, executable, binary, unknown. Respect `NO_COLOR` and `LS_COLORS`.
- Known heavy dirs (`node_modules`, `.git`, `target`, `.venv`, `__pycache__`) are closed by default.

### Should have
- Magika content detection. Fast fallback: extension plus magic bytes (`infer` crate).
- Mark files whose content does not match their extension (`report.pdf` that is HTML). This is a big win of Magika.
- Previews: the first meaningful line, image size, archive entry count.
- Git status column and changed-file counts for each directory.
- `--focus PATH`: spend most of the budget on one subtree, but keep context around it.
- Sort keys: name, size, age, type.

### Could have
- Output modes: `--json`, `--md` (compact for LLM context), plain for pipes.
- A detection cache keyed by inode, size, and mtime, so repeat runs are instant.
- Line counts for code (as `erd` or `tokei` does).
- Duplicate hints (same size and hash).
- Config file (`~/.config/oak/config.toml`).
- A simple interactive mode later. broot already does the TUI well, so this has a low priority.

## 5. Technology

**Rust**, for these reasons:
- Magika 1.x is written in Rust. The `magika` crate (1.1.0) gives `Session::new()`, `identify_file_sync`, and `identify_content_sync`. A session is safe to use from many threads.
- `ignore` (the ripgrep walker) gives parallel walks with gitignore support.
- One static binary for Linux and macOS.

Planned crates: `ignore`, `magika` + `ort`, `infer`, `clap`, `owo-colors` or
`anstream`, `gix` (git status, optional), `serde_json`, `imagesize`.

**Risk:** Magika needs ONNX Runtime through `ort`. That adds about 15–25 MB to the
binary and can cause build problems on some platforms. Plan: put Magika behind a
cargo feature (`--features magika`, on by default). If it fails, the tool falls back
to `infer` plus the extension.

## 6. Phases

0. **Spike (½ day):** Build `magika` + `ort` on Linux, and check macOS in CI. Measure the time for each file and the binary size. Measure `ignore` walk speed on a 1M-file tree.
1. **MVP:** walk, aggregate, line budget, summaries, sibling grouping, colors from extension and magic bytes.
2. **Magika:** content types, type mix in summaries, mismatch markers, sampling for closed dirs.
3. **Context:** previews, git status, `--focus`, sort keys.
4. **Output and polish:** JSON and Markdown modes, cache, config, shell completions, man page, release builds (cargo-dist, Homebrew tap).

## 7. Open questions

1. Language: Rust (recommended), or Go or Python for a faster first version?
2. Name: `oak` (the folder name)?
3. Is Magika required, or is extension plus magic bytes enough for the first release?
4. Main user: a person in a terminal, an LLM agent that needs context, or both? This sets the default output.
