# teak

teak shows a large directory tree in a fixed number of lines. It opens the
directories that tell you the most, folds the rest into summary rows, and
types files by their content with Google Magika.

teak on its own repo, in 14 lines (`teak -n 14`):

```
teak/  61G · 1,298,465 files · rust crate
├── ▪ chaff  target/ 61G 1.3M · .gitignore · Cargo.lock
├── src/                    378K  39  ≈ rust                                         2m
│   ├── detect/              58K   7  ≈ rust                                         2m
│   ├── io.rs               8.9K      rust                                           2m
│   ├── lib.rs               23K      rust                                           2m
│   ├── main.rs             1016      rust                                           5m
│   └── … 2 dirs, 18 files  287K  29  ≈ rust                                         3m
├── tests/                  223K  54  ≈ rust 93% txt 2% html 2% +12 types · 3 dirs   2m
├── CHANGELOG.md            1.6K      markdown                                      20s
├── Cargo.toml              4.5K      toml                                          52s
├── README.md               3.6K      markdown                                      20s
└── … 7 dirs, 3 files       192K  29  ≈ txt 54% rust 20% markdown 11% +3 types       2m
── 14 of 14 lines · 1,298,465 files · folded 1,297,823 · hidden 517
```

## Install

```sh
cargo install teak
brew install christianmerkwirth/tap/teak
```

Release archives for Linux (x86_64, aarch64) and macOS (x86_64, arm64) are on
the GitHub releases page. Each archive holds the binary, the bash, zsh, and
fish completions, and the man page `teak.1`.

On Intel macOS, teak builds without Magika, because no prebuilt ONNX Runtime
exists for that target. Detection then uses magic bytes and extensions, and
`teak --version` prints `magika: off`.

## Use

```sh
teak                      # the current directory, in at most 100 lines
teak -n 40 ~/data         # 40 lines
teak -l                   # all columns, like ls -lah
teak --focus src/api      # spend most lines on one path
teak --why target/        # why a row shows, folds, or hides
teak --format md -n 120   # Markdown, for an LLM prompt
teak --format json        # JSON, for tools
```

Run `teak --help` or `man teak` for all flags.

## Output formats

| Format | Use |
|---|---|
| `tty` | The default on a terminal: colors and Unicode glyphs. |
| `plain` | The default in a pipe: no colors, width 120. |
| `md` | A heading, the tree in a fenced block (ASCII), the type table, and the folded classes. |
| `json` | The visible rows and the summaries. `--json-full` adds every entry. |

The same tree and the same scan time always give the same bytes in `plain`,
`md`, and `json`. Set `SOURCE_DATE_EPOCH` to fix the scan time. The JSON
schema is in [`schema/teak.schema.json`](schema/teak.schema.json); its
`schema_version` changes only with a changelog entry.

## Config

teak reads `~/.config/teak/config.toml` (or
`$XDG_CONFIG_HOME/teak/config.toml`) and the nearest `.teak.toml` in the root or
a parent directory. Flags override the project file, and the project file
overrides the user file.

```toml
lines = 60
sort = "size"
format = "plain"
templates = ["teak-templates"]   # relative to this file

[chaff]
show = ["lock"]
hide = ["empty"]

[[chaff.rules]]
pattern = "docs/api/"            # a trailing / matches directories only
class = "generated"

[weights]                        # salience terms, defaults shown
size = 0.30
recency = 0.20
git = 0.15
rarity = 0.10
role = 0.10
anomaly = 0.15
```

Other keys: `depth`, `series_min`, `detect`, `preview`, `color`, `width`, and
`time`. A syntax error or a bad value stops teak with exit code 2. An unknown
key gives one warning.

## Exit codes

- `0`: success.
- `1`: success with errors, for example a missing root or entries that teak
  cannot read.
- `2`: a usage error: a bad flag value or a bad config file.

## Completions and man page

The release archives hold them. From a source checkout:

```sh
cargo run --example gen_assets -- assets
```

This writes `teak.bash`, `_teak` (zsh), `teak.fish`, and `teak.1` into
`assets/`.

## License

MIT
