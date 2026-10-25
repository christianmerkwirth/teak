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

teak runs on Linux (x86_64, aarch64) and macOS (Apple silicon, Intel).

### Prebuilt binaries

The installer script downloads the right archive for your machine and puts
`teak` into `~/.cargo/bin`:

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/christianmerkwirth/teak/releases/latest/download/teak-installer.sh | sh
```

The script also adds `~/.cargo/bin` to your `PATH`, if it is not there yet.
Open a new shell, then check the install:

```sh
teak --version    # teak 0.1.0 (magika: on)
```

You can also download an archive from the
[releases page](https://github.com/christianmerkwirth/teak/releases) and
unpack it yourself:

```sh
tar -xJf teak-x86_64-unknown-linux-gnu.tar.xz
install -m 755 teak-x86_64-unknown-linux-gnu/teak ~/.local/bin/
```

The archive names are `teak-<target>.tar.xz`, with these targets:

| Machine | Target |
|---|---|
| Linux x86_64 | `x86_64-unknown-linux-gnu` |
| Linux ARM 64-bit | `aarch64-unknown-linux-gnu` |
| macOS, Apple silicon | `aarch64-apple-darwin` |
| macOS, Intel | `x86_64-apple-darwin` |

Each archive holds the binary, the bash, zsh, and fish completions, and the
man page `teak.1`. A `.sha256` file is next to each archive.

The macOS binaries are not signed. If you download an archive with a web
browser, macOS blocks the binary. To allow it, run
`xattr -d com.apple.quarantine teak` once. The installer script and `curl`
downloads do not have this problem.

### From source

You need:

- Rust 1.88 or newer. Get it from [rustup.rs](https://rustup.rs), or update
  with `rustup update`.
- A C compiler, for the bundled xz and zstd libraries. On Debian or Ubuntu:
  `sudo apt install build-essential`. On macOS: `xcode-select --install`.
- Network access during the build. The build downloads a prebuilt ONNX
  Runtime for Magika. teak itself never uses the network.

Install the latest version from GitHub:

```sh
cargo install --locked --git https://github.com/christianmerkwirth/teak
```

Or install from a local checkout:

```sh
git clone https://github.com/christianmerkwirth/teak
cd teak
cargo install --locked --path .
```

Both commands put the binary into `~/.cargo/bin`. rustup adds that directory
to your `PATH`.

To build with no network access, turn Magika off. teak then types files by
magic bytes and file names:

```sh
cargo install --locked --path . --no-default-features
```

To try teak without an install, run it from the checkout:

```sh
cargo run --release -- -n 40 ~/some/dir
```

teak is not on crates.io yet, so `cargo install teak` does not work.

### Intel macOS

On Intel macOS, teak builds without Magika, because no prebuilt ONNX Runtime
exists for that target. teak then types files by magic bytes and file names,
and `teak --version` prints `magika: off`. You do not need a special flag.

### Uninstall

```sh
rm ~/.cargo/bin/teak        # after the installer script
cargo uninstall teak        # after cargo install
```

Then remove the type cache: `~/.cache/teak/` on Linux,
`~/Library/Caches/teak/` on macOS. Remove `~/.config/teak/` too if you made a
config file.

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

The files are in `assets/` in the repo, and at the top level of each release
archive. Copy the ones for your shell. These commands run from a checkout; in
an unpacked archive, leave out `assets/`:

```sh
# bash (needs the bash-completion package)
mkdir -p ~/.local/share/bash-completion/completions
cp assets/teak.bash ~/.local/share/bash-completion/completions/teak

# zsh: put _teak into a directory in $fpath
mkdir -p ~/.zfunc
cp assets/_teak ~/.zfunc/
# then in ~/.zshrc, before compinit: fpath=(~/.zfunc $fpath)

# fish
mkdir -p ~/.config/fish/completions
cp assets/teak.fish ~/.config/fish/completions/

# man page
mkdir -p ~/.local/share/man/man1
cp assets/teak.1 ~/.local/share/man/man1/
```

Open a new shell, then try `teak --fo<Tab>` and `man teak`. If `man teak`
finds nothing, add `export MANPATH="$HOME/.local/share/man:"` to your shell
profile.

After a change to the CLI, make the files again:

```sh
cargo run --example gen_assets -- assets
```

## License

MIT
