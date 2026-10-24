# Task 1 Completion: Build the phase-0 spike and write the phase-0 report

## Summary
Set up the crate (features `magika`, `test-hooks`), the `--version` stub, the `io` read gate with test hooks, the seeded `gen_tree` generator, the criterion benches, `scripts/bench.sh`, and CI. Measured the build, the Magika speed, and the walk speed and memory on 1M files, and wrote `docs/phase0-report.md` with a go/no-go for FR-71 to FR-75.

## Commits
- `5406091` test(teak): add failing phase-0 smoke tests (FR-71, FR-72, FR-73, FR-74, FR-75)
- `110adad` feat(teak): add version stub, io read gate with test hooks, and gen_tree (FR-71, FR-72, FR-73, FR-74, FR-75)
- `173e350` chore(teak): add phase-0 benches, bench script, and CI (FR-71, FR-72, FR-73, FR-74, FR-75)
- `13dab18` chore(teak): add phase-0 report (FR-71, FR-72, FR-73, FR-74, FR-75)

## Deviations
- **Rule 1: Bug** — `gen_tree`: two series with the same prefix in one directory shared paths, and parallel writers raced → the second series gets its own prefix.
- **Rule 1: Bug** — spike arena: a full component sort took 2.43 s and 372 MB RSS on 1M files (above the 300 MB target) → a directory map, per-list name sorts, relative paths, and in-place shards give 358 ms and 242 MB.
- **Rule 3: Blocking** — `ort/download-binaries` needs a TLS feature → chose `tls-rustls` (no OpenSSL headers needed).
- **Rule 3: Blocking** — the project was not a git repo, and hyperfine was missing → `git init -b main`, `cargo install hyperfine` (the user approved the defaults).
- **Rule 2: Missing Critical** — added `io::MagikaInput` (a `magika::SyncInput` through the io gate). The task file does not list it, but design.md makes `io` the only reader, and it proves the report's custom-reader answer.
- **Spec gap (for verify)** — FR-51's 2 s cap conflicts with FR-72 (< 1 s) under `--detect full`. The report has Plan A1 (a Magika sub-cap of 300 files). It needs user approval and a requirements change. The requirements are not changed yet.

## Difficulties
- Pyke has no prebuilt ONNX Runtime for `x86_64-apple-darwin` → recorded as Plan B in the report (recommended: Intel macOS ships without `magika`).
- Four parallel Magika sessions give no speed-up (about 575 files/s) → recorded as an open question for task-3; the targets do not need it.
- macOS binary sizes cannot be measured here → the CI `checks` job writes them to the job summary; the report marks them as pending.

## Notes
- Done criteria: all met except the macOS size row, which waits for the first CI run (the repo has no GitHub remote yet). CI itself has not run yet.
- Plans A1, A2, A3, and B in `docs/phase0-report.md` wait for user approval before task-2 and task-3 use them.
- `LICENSE`, `PLAN.md`, and `.sddw/` are not committed; they were there before this task.
- Bench trees (4.8 GB for 1M files) are in `target/bench-data/` and are shared by `benches/walk.rs` and `scripts/bench.sh`.
- Task-2: number the arena nodes in a fixed order (FR-65); the spike ids follow walk order.
