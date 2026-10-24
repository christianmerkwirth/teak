# Design: teak

## Trace
- **FR-IDs covered:** FR-01, FR-02, FR-03, FR-04, FR-05, FR-06, FR-07, FR-08, FR-09, FR-10, FR-11, FR-12, FR-13, FR-14, FR-15, FR-16, FR-17, FR-18, FR-19, FR-20, FR-21, FR-22, FR-23, FR-24, FR-25, FR-26, FR-27, FR-28, FR-29, FR-30, FR-31, FR-32, FR-33, FR-34, FR-35, FR-36, FR-37, FR-38, FR-39, FR-40, FR-41, FR-42, FR-43, FR-44, FR-45, FR-46, FR-47, FR-48, FR-49, FR-50, FR-51, FR-52, FR-53, FR-54, FR-55, FR-56, FR-57, FR-58, FR-59, FR-60, FR-61, FR-62, FR-63, FR-64, FR-65, FR-66, FR-67, FR-68, FR-69, FR-70, FR-71, FR-72, FR-73, FR-74, FR-75, FR-76, FR-77
- **Requirements:** ../requirements.md
- **PRD:** ../PRD.md

## Architecture

teak is one Rust crate with a library and a thin binary. The library is a staged
pipeline. Each stage reads the result of the stage before it. All output formats
render one shared `View`, so the line budget is enforced in one place.

### Crate layout

```
teak/
├── Cargo.toml          features: default = ["magika"], magika, test-hooks
├── src/
│   ├── main.rs         parse CLI, call teak::run, map result to exit code
│   ├── lib.rs          run(): drives the pipeline
│   ├── cli.rs          clap definitions (also feed completions + man page)
│   ├── io.rs           the one gate for all file-content reads (+ test hooks)
│   ├── walk.rs  tree.rs  aggregate.rs
│   ├── chaff/          mod.rs, rules.rs, packs.rs, gitignore.rs,
│   │                   gitattributes.rs, content.rs, templates.rs
│   ├── series.rs  salience.rs  select.rs  view.rs
│   ├── detect/         mod.rs, names.rs, magic.rs, ext.rs, magika.rs,
│   │                   sample.rs, secret.rs
│   ├── cache.rs  preview.rs  git.rs  kind.rs  explain.rs  dupes.rs
│   ├── layout.rs  style.rs
│   ├── render/         mod.rs, text.rs (tty + plain), md.rs, json.rs
│   └── config.rs
├── templates/          built-in boilerplate texts (include_str!)
├── schema/teak.schema.json
├── examples/gen_tree.rs   seeded fixture generator for benchmarks
├── benches/            criterion benches
├── scripts/bench.sh    hyperfine runs for FR-71..FR-75
├── tests/              integration tests + tests/support/ + snapshots
└── docs/phase0-report.md
```

### Components
- `cli`: clap flags, value checks, usage errors (exit 2) — new
- `io`: the only place that opens files for content; counts reads and injects delays under `test-hooks` — new
- `walk`: parallel stat walk with the `ignore` crate, built-in filters off; symlinks not followed; `-x`; pseudo-FS marking; hard-link dedupe; per-entry errors become flags — new
- `tree`: arena of compact `Node`s, names in one byte pool, children sorted by name bytes — new
- `aggregate`: per-directory totals in one bottom-up pass — new
- `chaff`: rule chain (user → `.gitattributes` → rule packs → global names), gitignore matcher, hidden rules, class actions; content checks from P2 — new
- `series`: name shapes, special slots, series units, uniform directories — new
- `salience`: per-node score terms and weighted sum — new
- `select`: greedy budget planner that builds the `View` — new
- `view`: `Row` and `View` types, footer counts — new
- `detect`: stat, name rules, magic bytes (`infer`), extension, Magika, sampling, global cap, mismatch, secret check — new
- `cache`: type cache file in the user cache directory — new
- `preview`: per-type preview strings — new (P3)
- `git`: in-process status with `gix`, no program runs — new (P3)
- `kind`: project kind from marker files — new (P3)
- `explain`: `--why` output from the same terms that `select` uses — new (P3)
- `dupes`: duplicate finder for `--dupes` — new (P3)
- `layout`: column set, width fit, drop order, name cut, glyph set — new
- `style`: type-group colors, dim, bold, `LS_COLORS` for names, color policy — new
- `render`: `text` (tty + plain), `md`, `json` — new
- `config`: TOML layers and salience weights — new (P4)

### Data Flow
```
argv → cli::parse → Opts (defaults ← user config ← project config ← flags)
Opts.roots → walk::scan → Tree (arena, flags)
Tree → aggregate::compute → Vec<DirAgg>
Tree + Aggs → chaff::Classifier::classify_names → Vec<ChaffTag>
Tree + Tags → series::group (per dir) → Units
Tree + Aggs + Tags + Units + Signals(git, focus) → salience::score → Vec<f32>
all of the above → select::plan(budget) → View (skeleton rows)
View → detect::run (visible rows + samples, cap, cache) → TypeInfo per row
View → chaff content checks (generated, minified, boilerplate) → may move a row into the chaff line
View → preview::of, git status, project kind → cells
View → layout::fit(width) → Lines → render (tty | plain | md | json) → stdout
errors → one line each on stderr → exit code 0 | 1 | 2
```

Content detection runs **after** selection. Salience uses only stat data, names,
chaff tags, git, and focus. Content results change labels and flags, not which
rows show. A content check can move one visible file into its directory's chaff
line. That replaces one row with at most one chaff line, so the budget holds.

## Data Models

No database, so no migration. Two persistent formats carry a version: the type
cache file and the JSON output schema. A version change makes old cache data
invalid; a JSON schema change needs a new `schema_version` and a changelog entry.

- `NodeId = u32`. `u32::MAX` means none.
- `Node` (about 48 bytes, no `PathBuf`):
  - `parent, first_child, next_sibling: NodeId`
  - `name: (u32 offset, u16 len)` into `Tree.names: Vec<u8>` (raw `OsStr` bytes)
  - `kind: Dir | File | Symlink | Fifo | Socket | Device`
  - `size: u64` (0 for a repeated hard link), `mtime_ns: i64`, `mode: u32`, `uid: u32`, `gid: u32`, `dev: u64`, `ino: u64`
  - `flags: u16` bitset: `NO_ACCESS` (⊘), `LOOP` (↻), `BROKEN`, `MOUNT`, `PSEUDO_FS`, `HIDDEN`, `GITIGNORED`, `SECRET`, `ROOT`
- `Tree`: `nodes: Vec<Node>`, `names: Vec<u8>`, `link_targets: HashMap<NodeId, Vec<u8>>`, `roots: Vec<NodeId>`, `scan_time: i64` (`SOURCE_DATE_EPOCH` or now), `errors: Vec<ScanError>`
- `DirAgg`: `total_size: u64`, `file_count: u64`, `dir_count: u64`, `newest_mtime: i64`, `oldest_mtime: i64`, `ext_hist: SmallVec<[(ExtId, u32 count, u64 bytes); 4]>`
- `ChaffClass`: `Vendor | Build | Cache | Vcs | Editor | Lock | Generated | Minified | Boilerplate | Empty | Duplicate`, plus the pseudo-class `Ignored` for gitignored entries
- `Action`: `Show | Dim | Fold | Hide`
- `ChaffTag`: `class: Option<ChaffClass>`, `action: Action`, `rule: RuleRef` (source: `User | Gitattributes | Pack(kind) | Content | Global | Gitignore | Hidden`, plus the pattern), `label: Option<String>` (for example `MIT`), `dup_of: Option<NodeId>`
- `TypeGroup`: `Code | Text | Data | Image | Av | Archive | Exec | Binary | Unknown`
- `TypeInfo`: `label: LabelId`, `group: TypeGroup`, `mime: &'static str`, `is_text: bool`, `score: f32`, `source: Stat | Name | Magika | Magic | Ext`, `estimated: bool`, `mismatch: Option<LabelId>` (the label the extension says)
- `Unit`: `Node(NodeId)` or `Series(Series)`
- `Series`: `prefix/suffix` tokens, `slot: Num | Date | Hash | Uuid`, `range: (String, String)`, `missing: u64`, `members: Vec<NodeId>`, `total_size: u64`, `median_size: u64`
- `Row`, one output line each:
  - `Header { root, agg, kind: Option<String>, git: Option<GitHead>, flags }`
  - `Entry { node, depth, joined: Vec<NodeId>, cells }`
  - `Overflow { parent, depth, dirs, files, size, mix }`
  - `Series { parent, depth, series, mix }`
  - `Summary { node, depth, agg, mix }` (a folded directory)
  - `Chaff { parent, depth, items: Vec<ChaffItem>, more: u64, bold: bool }`
  - `Footer { used, budget, total_files, folded: ClassCounts, hidden: ClassCounts }`
- `View`: `rows: Vec<Row>`, `budget: usize`. Invariant: `rows.len() <= budget`.
- `CacheEntry` (in `types.bin`): `dev u64 | ino u64 | size u64 | mtime_ns i64 | label u16 | score f32 | chaff u8 | template u16 | last_used u32`. File header: magic `TEAKC`, `format_version u16`, `magika_model_version` string, `count u32`. Records sorted by `(dev, ino)`.
- JSON output (P4): top-level `schema_version: 1`, `root`, `scan_time`, `header`, `rows[]`, `footer`, `types[]`. `--json-full` adds `entries[]` with every node. Schema: `schema/teak.schema.json`.

## Interface Contracts

### CLI (shared)
- The CLI is the public interface. Flags, defaults, and exit codes follow PRD §11 and requirements §4.
- Exit codes: `0` success, `1` success with errors (a missing root, `⊘` entries), `2` usage error (bad flag value, bad config, bad `--focus`/`--why` path).
- stderr: one line for each error or warning. stdout: only the view.
- Flags that a later phase adds SHALL NOT change the output of an earlier phase's tests unless the requirement says so.

### Internal Interfaces (shared)
- `teak::run(opts: Opts, out: &mut impl Write, err: &mut impl Write) -> ExitCode`: runs the whole pipeline for each root in order.
  - Pre: `opts` is valid (clap and config checks passed).
  - Post: each root gives one full view (header to footer); the exit code is the worst result over all roots.
- `walk::scan(root: &Path, opts: &WalkOpts) -> Result<Tree, ScanError>`: stat-only walk.
  - Pre: none (a missing root is an `Err`).
  - Post: no file content was read; children are sorted by name bytes; access errors are `NO_ACCESS` flags, not aborts; repeated hard links have size 0; with `-x`, a mount point is one node with `MOUNT` and no children.
- `aggregate::compute(tree: &Tree) -> Vec<DirAgg>`: one bottom-up pass.
  - Post: `agg[root].file_count` equals the number of non-dir nodes in the tree.
- `chaff::Classifier::new(tree, opts: &ChaffOpts, templates: &Templates) -> Classifier`
- `Classifier::classify_names(&self, tree: &Tree) -> Vec<ChaffTag>`: names, gitignore, gitattributes, packs, hidden.
  - Post: every node has exactly one tag; the first matching source wins (FR-31).
- `Classifier::classify_content(&self, id: NodeId, head: &[u8], size: u64) -> Option<ChaffTag>`: generated, minified, boilerplate (P2).
  - Pre: `head` came from `io::read_head` with the content-check limit.
- `series::group(tree, dir: NodeId, children: &[NodeId], min: usize) -> Vec<Unit>`
  - Post: every child is in exactly one unit; units are in name order.
- `salience::score(inp: &ScoreInputs, w: &Weights) -> Scores`: one `f32` in `[0, 1]` per node plus the term values (for `--why`).
- `select::plan(inp: &SelectInputs, budget: usize, opts: &SelectOpts) -> View`
  - Pre: `budget >= 3`.
  - Post: `view.rows.len() <= budget`; every node is in exactly one row (entry, summary, overflow, series, or chaff) or in a footer count (FR-21); the result does not depend on thread count; ties break by name.
- `io::read_head(path: &Path, max: usize) -> io::Result<Vec<u8>>`: the gate for all content reads.
  - Pre: the node is a regular file and not on a pseudo FS.
  - Post: under `test-hooks`, the bytes are added to the per-path read counter, and `TEAK_TEST_READ_DELAY_MS` is applied.
- `detect::Detector::new(mode: DetectMode, cache: Option<Cache>) -> Detector`
- `Detector::type_of(&self, tree: &Tree, id: NodeId) -> TypeInfo`: the first answer from stat → names → (Magika | magic) → extension.
- `detect::run(view: &mut View, tree: &Tree, det: &Detector, cap: Cap) -> DetectStats`: visible rows first, then stratified samples, until the cap.
- `detect::secret::is_secret(name: &[u8], head: Option<&[u8]>) -> bool`: name patterns and private-key headers.
- `preview::of(tree, id, info: &TypeInfo, head: &[u8]) -> Option<String>`: returns `None` for `SECRET` nodes.
- `git::open(root: &Path) -> Option<GitRepo>`; `GitRepo::status(&self, tree: &Tree) -> GitStatus`; `GitRepo::head(&self) -> GitHead`
  - Post: no subprocess; no filters, hooks, or fsmonitor run.
- `layout::fit(view: &View, cols: &ColumnSet, width: usize, glyphs: Glyphs) -> Vec<Line>`
  - Post: one line per row; no line is wider than `width` display columns.
- `render::write(lines_or_view, fmt: Format, style: &Style, out: &mut impl Write) -> io::Result<()>`
- `explain::why(path: &Path, inp: &SelectInputs, view: &View) -> Result<Explanation, UsageError>` (P3)
- `config::load(root: &Path) -> Result<ConfigLayers, ConfigError>` (P4)

## Design Decisions

### Pipeline order: detect after select
- **Chosen:** select rows from stat, name, chaff, git, and focus signals; run content detection only on visible rows and samples.
- **Rationale:** one read per file on 1M files breaks FR-71 and FR-73. Matches PRD §9.3 step 7.
- **Rejected:** fast detection on every file before selection — too slow. Two-pass selection — more complex, and the second pass can change the view.

### One crate, library + binary
- **Chosen:** one `teak` crate; `src/lib.rs` holds the engine, `src/main.rs` is thin.
- **Rationale:** simplest for `cargo install teak` (FR-68); integration tests use both.
- **Rejected:** a workspace (`teak-core`, `teak-detect`, `teak`) — publish and version overhead.

### Walk all entries; match gitignore ourselves
- **Chosen:** `ignore::WalkBuilder` with `standard_filters(false)`; the `chaff::gitignore` module evaluates `.gitignore` and `.git/info/exclude` per directory with `ignore::gitignore::Gitignore`.
- **Rationale:** FR-21 and FR-37 need the size and count of ignored entries (a gitignored 40G `data/` must show).
- **Rejected:** the walker's built-in filters — they skip ignored entries, so they cannot be counted.

### Deterministic tree
- **Chosen:** collect entries per thread, then build the arena with children sorted by name bytes. Ties in selection break by name. Scan time comes from `SOURCE_DATE_EPOCH` when set.
- **Rationale:** FR-13 edge case and FR-65 (same output for `-j 1` and `-j 8`).
- **Rejected:** walk-order output — it changes between runs.

### Budget accounting
- **Chosen:** one row is one line; layout never wraps, it cuts. Header and footer take 2 lines, so rows get `budget - 2`. Each open directory uses at least its shown children plus one overflow row (unless all children fit) plus at most one chaff line. The greedy queue (PRD §9.3) opens the directory with the best gain ÷ cost while the cost fits. `select::plan` asserts the invariant in debug builds; a proptest checks it (FR-12).
- **Rationale:** the budget is the main promise (G1). One place enforces it for all formats.
- **Rejected:** trimming lines after rendering — it would drop rows without counting them (breaks FR-21).

### Salience defaults
- **Chosen:** `score = clamp(Σ wᵢ·termᵢ) × chaff_factor + focus_boost`. Default weights: size share 0.30, recency 0.20, git 0.15, rarity 0.10, role 0.10, anomaly 0.15. `chaff_factor` is 0.2 for `Dim` and gitignored entries. `focus_boost` is 1.0 on the focus path, and decays by half for each step away. All weights are config keys from P4 (FR-67).
- **Rationale:** size and recency carry the most signal for FR-13; the weights are a start point to tune on real trees.
- **Rejected:** fixed depth (`tree -L`) — fails FR-13.

### Chaff action resolution
- **Chosen:** start from the per-class defaults (PRD §6.1). `--chaff LEVEL` sets all classes. `--show`, `--fold`, and `--hide` then apply in command-line order, so the later flag wins (FR-36). Dotfiles and `vcs`/`editor` are hidden unless `-A` (fold) or `-a` (show). `-a` also turns off gitignore and chaff folding (FR-35).
- **Rationale:** matches `ls -A`/`ls -a` habits and the FR-36 edge case.
- **Rejected:** a fixed precedence between `--show` and `--hide` — breaks the "later flag wins" rule.

### Content reads: Magika reads; checks re-read (FR-42 amended)
- **Chosen:** Magika reads each file through its own API (`identify_file_sync` or its feature extractor). Content checks then call `io::read_head(path, 64 KiB)` once per checked file: markers use the first 2 KiB, the minified check uses line stats of the head, and template matching uses the whole text (only files ≤ 64 KiB). Magika's reads also pass through the `io` counter: phase 0 confirms whether the `magika` crate takes a custom reader; if not, the hook adds the byte sizes that the model config reads.
- **Rationale:** the user chose the simpler code path. FR-42 is amended in requirements.md to a 64 KiB bound per checked file. The limit is 64 KiB, not 16 KiB, because the GPL-3.0 text is about 35 KiB.
- **Rejected:** one shared bounded `Probe` for Magika and checks — keeps the original FR-42, but needs a lower-level Magika API.

### Cache stores check results
- **Chosen:** a cache entry stores the content-check result (chaff class and template id) next to the type label.
- **Rationale:** with the re-read design, a cache hit would still re-read for checks, which breaks the "zero reads" tests of FR-54 and FR-75.
- **Rejected:** caching only the type label.

### No content reads with `--detect off`
- **Chosen:** `--detect off` turns off all content reads: Magika, magic bytes, content checks, and previews that need content. Stat-based previews (symlink targets) stay.
- **Rationale:** FR-52 requires a read counter of 0 for all files, and FR-38's edge case needs no generated class with `--detect off`.
- **Rejected:** previews that read files in `off` mode — break FR-52.

### Pseudo file systems
- **Chosen:** call `statfs` once per device (`rustix`). On Linux, `proc`, `sysfs`, `devpts`, `cgroup`, `cgroup2`, `debugfs`, `tracefs`, `securityfs`, `bpf`, and `configfs` get `PSEUDO_FS`; on macOS, `devfs`. `io::read_head` refuses `PSEUDO_FS` nodes.
- **Rationale:** FR-08; sizes in `/proc` are not real and reads can block.
- **Rejected:** a path prefix list (`/proc`, `/sys`) — misses bind mounts and other mount points.

### Secret guard
- **Chosen:** `detect::secret` marks a node `SECRET` by name (`.env*`, `*.pem`, `*.key`, `id_rsa*`, `id_ed25519*`, `credentials*`, `.netrc`, `.pgpass`) and by content (`-----BEGIN ... PRIVATE KEY-----` in the head). `preview::of` and all renderers (including `--json-full`) emit `no preview (secret)` and never content bytes for `SECRET` nodes.
- **Rationale:** FR-77; agents paste output into prompts.
- **Rejected:** a guard only in the preview stage — JSON or `--why` could leak bytes another way.

### Git status: gix, stat-only, no filters
- **Chosen:** `gix` with `default-features = false` and only the index, refs, revision, and object features. Read the index; compare stat data; hash raw file bytes only on a stat change; never apply filters, `core.fsmonitor`, or hooks. Untracked files come from our own walk + gitignore. Ahead/behind come from a rev-walk to the upstream ref. Status runs only for files in visible directories.
- **Rationale:** FR-76 (the strace test allows no `execve` except teak) and FR-10 (no network crates).
- **Rejected:** shelling out to `git` — runs fsmonitor and filters, and fails the strace test. `git2` — C dependency, and its status runs filter drivers unless turned off with care. Our own index reader — more code for refs and ahead/behind.

### Type cache format
- **Chosen:** one flat binary file `types.bin` in `$XDG_CACHE_HOME/teak/` (Linux) or `~/Library/Caches/teak/` (macOS). Load it whole at start. At exit, write a temp file in the cache dir and rename it. Trim to 200,000 entries by oldest `last_used`. A bad magic, a bad version, a model-version change, or a short read means: ignore the file and build a new one. `--no-cache` skips all cache I/O.
- **Rationale:** FR-54 and §12.2 with no extra crate; the key `(dev, ino, size, mtime_ns)` never returns a type for a changed file.
- **Rejected:** `redb` — an extra dependency and a format we do not control.

### Test hooks behind a cargo feature
- **Chosen:** the `test-hooks` feature compiles in: per-path read counters in `io`, `TEAK_TEST_READ_DELAY_MS` (delay per content read), `TEAK_TEST_NO_CONTENT_CHECKS=1` (turn off content checks), and `TEAK_TEST_REPORT=<file>` (JSON with `reads` and `stages_ms`). Release builds do not have them.
- **Rationale:** FR-42, FR-43, FR-50, FR-51, FR-52, FR-54, FR-74, and FR-75 need these signals; users never see them.
- **Rejected:** a hidden runtime flag — ships test code and widens the CLI.

### Magika as an optional feature
- **Chosen:** cargo feature `magika` (default on) pulls in `magika` and `ort`; the model is built into the binary. Without the feature, `--detect full` acts as `fast`, and `teak --version` prints `magika: off` (FR-55). Phase 0 chooses the ONNX Runtime link method.
- **Rationale:** the ONNX build is the top risk (PRD §16).
- **Rejected:** a runtime model download — forbidden (FR-10).

### Dependencies
- **Chosen:** `clap` (+ `clap_complete`, `clap_mangen`), `ignore`, `globset`, `infer`, `magika` + `ort` (optional), `gix` (P3, minimal features), `imagesize`, `flate2`/`zstd`/`xz2` (peek only), `zip` (central directory only), `serde`, `serde_json`, `toml`, `unicode-width`, `terminal_size`, `lscolors`, `rustix`, `rayon`, `smallvec`. Dev: `insta`, `proptest`, `tempfile`, `assert_cmd`, `criterion`, `jsonschema`, `pulldown-cmark`, `portable-pty`.
- **Rationale:** each crate maps to a requirement; none makes network calls at runtime.
- **Rejected:** any HTTP client or telemetry crate in the normal dependency graph (FR-10 static check uses `cargo tree -e normal`).
