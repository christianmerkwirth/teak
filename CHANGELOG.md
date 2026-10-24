# Changelog

All notable changes to teak are in this file. The `plain` and `md` formats and
the JSON schema are stable interfaces: a change to them gets an entry here,
and a JSON change also gets a new `schema_version`.

## [Unreleased]

## [0.1.0]

The first release.

### Output formats (v1)

- `plain` v1: one header line, one line per row, one footer line. No ANSI
  codes; width 120 unless `--width`.
- `md` v1: line 1 is `# <root>/ — <size>, <n> files`; line 2 has the project
  kinds, git, the changed count, and `scanned <YYYY-MM-DD HH:MM>` (UTC). Then
  the `plain` rows in ASCII in a fenced `text` block, the type table
  (`| Group | Files | Size |`), and the line `Folded: … Run `teak -a` to show
  all.`
- JSON `schema_version: 1` (`schema/teak.schema.json`): `root`, `scan_time`,
  `header`, `rows`, `footer`, and `types`; `--json-full` adds `entries`.
  Several roots give an array of documents.

### Added

- The line budget, salience, and selection (P1).
- Chaff classes and the chaff line, gitignore, `.gitattributes`, and rule
  packs (P1).
- Content types with Magika, series rows, the type cache, and boilerplate
  templates (P2).
- Previews, git status, `--focus`, `--why`, `--stats`, `-l`, and `--dupes`
  (P3).
- `--format md`, `--format json`, and `--json-full` (P4).
- Config files: `~/.config/teak/config.toml` and `.teak.toml`, with chaff
  rules and salience weights (P4).
- Shell completions (bash, zsh, fish) and a man page (P4).
- Release archives for Linux x86_64 and aarch64 and macOS x86_64 and arm64,
  and a Homebrew tap (P4). Intel macOS builds without Magika.
