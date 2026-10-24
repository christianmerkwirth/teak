# Task 2 Completion: Build the P1 core — walk, chaff, series, selection, fast types, tty and plain output

## Summary
Built the P1 pipeline in six TDD slices: the clap CLI, the parallel stat walk and the arena in a fixed order, totals, chaff classes and actions, series and uniform directories, salience, the greedy budget planner, fast detection with the secret guard, layout, colors, and the `tty` and `plain` renderers. Added the safety, property, and snapshot tests, the scan bench, and the kernel check.

## Commits
- `9488743` test(teak): add failing traversal and budget tests (FR-01, FR-02, FR-03, FR-05, FR-06, FR-07, FR-08, FR-09, FR-11, FR-13, FR-14, FR-15, FR-16, FR-17, FR-19, FR-70)
- `e1de607` feat(teak): add the P1 pipeline: walk, tree, totals, salience, selection, layout, plain output (same FR-IDs)
- `5acb672` test(teak): add failing chaff tests (FR-20, FR-30, FR-31, FR-32, FR-33, FR-34, FR-35, FR-36, FR-37)
- `9f9ba95` feat(teak): classify chaff by gitattributes, packs, names, and gitignore (same FR-IDs)
- `b3d094a` test(teak): add failing fast-detection and secret tests (FR-48, FR-77)
- `6f84fdd` feat(teak): add fast detection and the secret check (FR-48, FR-77)
- `a92573a` test(teak): add failing series tests (FR-44, FR-45, FR-46, FR-47)
- `2a1fc0f` feat(teak): add series rows and uniform directories (FR-44, FR-45, FR-46, FR-47)
- `45ffd8d` test(teak): add display tests; color tests fail (FR-22, FR-23, FR-25, FR-26, FR-27, FR-28, FR-29, FR-34, FR-58, FR-62)
- `68e3921` feat(teak): add colors: type groups, dim chaff and overflow rows, LS_COLORS names (FR-27, FR-28, FR-34, FR-62)
- `34ae19f` fix(teak): leave count and type blank for unreadable directories (FR-05)
- `9620f26` test(teak): add safety, property, and snapshot tests (FR-04, FR-10, FR-12, FR-21, FR-76, FR-77)
- `9335218` chore(teak): add the scan bench, record the kernel check, drop smallvec (FR-71)

## Deviations
- **Rule 1: Bug (salience tuning)** — on the kernel tree, small folders opened before the 1.1 GiB `drivers/`. A checkout spread over seconds gave random recency, and rarity scored every file in mixed folders → recency is now a log scale of age against the scan time (10 years = 0); rarity counts only when one extension holds at least half of the files; opening a directory gains its first two children minus half its own score. design.md §Salience defaults keeps the weights.
- **Rule 1: Bug** — a repeated hard link (size 0) was classed `empty` → new `LINK_DUP` flag.
- **Rule 1: Bug** — a `⊘` directory showed `0  empty` → its count and type stay blank (found in the snapshot review).
- **Rule 1: Bug (tests)** — the timeout helper did not drain the pipes and hung on `/proc`; `.git/` in the `-a` test was a joined chain; the read report keys are the opened paths (`./big.dat`); budget fixtures with numbered names became series (now `support::word`); the hash fixture overflowed and its folder was uniform (moved to the root).
- **Rule 2: Missing Critical** — `io::read_rule_file` reads `.gitignore` and `.gitattributes`; the hooks do not count it (FR-52 counts content only).
- **Rule 2: Missing Critical** — the footer lists fewer classes (with `+N`) when it is wider than the output.
- **Rule 3: Blocking** — `lscolors` with default features pulls `nu-ansi-term` → `default-features = false`. `smallvec` is not needed (type mixes come from per-node labels) → removed from the design's dependency list.
- **Interface (internal, not architectural)** — `aggregate::compute(tree, skip)` leaves hidden entries out; one `Tree` per root (`run` loops the roots); `teak::run(opts, env, out, err)` takes an `Env`; `teak::build` is public for the tests; `RowKind::Summary` carries `uniform`; `ChaffTag::label` is a `u16` id (Plan A2); `detect::run` takes `&mut Tree` to set `SECRET`.
- **Spec choices (for verify)** — symlinks count as files; unclassed hidden dotfiles show as `dotfile` in the footer; `ignored` is a valid class for `--show/--fold/--hide`; `empty` chaff items always go into `+N` (this gives FR-33's `+4`); auto color is on only for the `tty` format, so `plain` on a terminal has no codes (FR-62); folder mixes are by size %, file-only overflow rows and series by count.
- **Test placement** — FR-20 is in `tests/chaff.rs` (it needs chaff), not `budget.rs`; the FR-34 bold check is in `display.rs`; FR-58 checks `markdown` (the Magika label), where the AC says `md`; FR-04 runs without `--dupes` and only `tty`/`plain` (task-4 and task-5 add them).

## Difficulties
- User namespaces are blocked here (AppArmor) → the FR-06 tmpfs mount and the FR-10 `unshare -n` run skip with a message. They need CI or a machine with user namespaces.
- `scripts/bench.sh 100k` and `1m` pass `--no-cache`, which task-3 adds → measured 1M by hand: 0.48 s, 130 MB peak RSS (Plan A2 stop line 280 MB). The script comment now says so.
- The macOS target is not installed → the macOS `statfs` code is not compiled here. The repo has no GitHub remote, so CI has not run.
- `ignore`'s parallel walker reads a directory before it visits it → loops, broken links, and unreadable directories map from its errors (`Loop`, `WithPath`).

## Notes
- Performance: 10k files 12.1 ms (`bench.sh 10k`, FR-71 target 150 ms); library pipeline 9.7 ms (10k) and 51 ms (100k); kernel tree (97k files) 50–90 ms.
- Property tests: 40 cases by default; `PROPTEST_CASES=300` passed in release and 120 in debug (with the `select::plan` assert on).
- `docs/phase1-kernel.txt` holds the `-n 40` kernel view (clone in `target/kernel`, about 2.1 GB).
- For task-3: `detect::run` is the place for Magika; `TypeInfo::mismatch` is already set by the fast path; series sample 3 members above 64; `--detect full` acts as `fast` until then.
- Plans A1, A2, A3, and B were approved before this task; A2 is done here.
