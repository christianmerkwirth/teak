//! The command line: clap definitions, value checks, and the resolved
//! [`Opts`]. Usage errors exit with code 2.

use std::ffi::OsString;
use std::path::PathBuf;

use clap::{ArgAction, CommandFactory, FromArgMatches, Parser, ValueEnum};

use crate::chaff::{Action, ChaffClass, ChaffOpts};
use crate::detect::DetectMode;
use crate::layout::{PreviewMode, SizeUnits, TimeMode};
use crate::render::Format;
use crate::select::SortKey;

/// `-n`: a line count, or the terminal height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Budget {
    Lines(usize),
    Auto,
}

/// `--color`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ColorWhen {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum SortArg {
    Salience,
    Name,
    Size,
    Age,
    Type,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ActionArg {
    Show,
    Dim,
    Fold,
    Hide,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ClassArg {
    Vendor,
    Build,
    Cache,
    Vcs,
    Editor,
    Lock,
    Generated,
    Minified,
    Boilerplate,
    Empty,
    Duplicate,
    Ignored,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum FormatArg {
    Tty,
    Plain,
    Md,
    Json,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum TimeArg {
    Relative,
    Iso,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum DetectArg {
    Off,
    Fast,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum PreviewArg {
    Off,
    Short,
    Full,
}

pub(crate) fn parse_budget(s: &str) -> Result<Budget, String> {
    if s == "auto" {
        return Ok(Budget::Auto);
    }
    match s.parse::<usize>() {
        Ok(n) if n >= 3 => Ok(Budget::Lines(n)),
        _ => Err("expected a number or 'auto'; the smallest budget is 3".to_string()),
    }
}

/// Fit a large directory tree into a fixed line budget.
#[derive(Parser, Debug)]
#[command(
    name = "teak",
    disable_version_flag = true,
    args_override_self = true,
    max_term_width = 100,
    after_help = "Config: ~/.config/teak/config.toml (or $XDG_CONFIG_HOME/teak/config.toml) and the \
                  nearest .teak.toml in the root or a parent. Flags override the project file; \
                  the project file overrides the user file."
)]
pub struct CliArgs {
    /// Directories or files to show (default: the current directory)
    #[arg(value_name = "PATH")]
    pub(crate) paths: Vec<PathBuf>,

    /// Line budget: a number (3 or more), or auto (terminal height - 2) [default: 100]
    #[arg(short = 'n', long = "lines", value_name = "N|auto", value_parser = parse_budget, help_heading = "Layout")]
    pub(crate) lines: Option<Budget>,

    /// Never open deeper than N
    #[arg(short = 'L', long = "depth", value_name = "N", help_heading = "Layout")]
    pub(crate) depth: Option<usize>,

    /// All columns (ls -lah style)
    #[arg(short = 'l', long, help_heading = "Layout")]
    pub(crate) long: bool,

    /// Display order inside a directory
    #[arg(long, value_enum, value_name = "KEY", help_heading = "Layout")]
    pub(crate) sort: Option<SortArg>,

    /// Spend most of the budget on PATH, keep the context around it
    #[arg(long, value_name = "PATH", help_heading = "Layout")]
    pub(crate) focus: Option<PathBuf>,

    /// Smallest group that becomes a series row [default: 4]
    #[arg(long = "series-min", value_name = "N",
          value_parser = clap::value_parser!(u64).range(2..), help_heading = "Layout")]
    pub(crate) series_min: Option<u64>,

    /// Content detection [default: full]
    #[arg(long, value_enum, value_name = "MODE", help_heading = "Content")]
    pub(crate) detect: Option<DetectArg>,

    /// Previews [default: short]
    #[arg(long, value_enum, value_name = "MODE", help_heading = "Content")]
    pub(crate) preview: Option<PreviewArg>,

    /// Find duplicate files: fold copies of visible files
    #[arg(long, help_heading = "Content")]
    pub(crate) dupes: bool,

    /// List only files whose content does not match the extension
    #[arg(long, help_heading = "Content")]
    pub(crate) mismatches: bool,

    /// Do not read or write the type cache
    #[arg(long = "no-cache", help_heading = "Content")]
    pub(crate) no_cache: bool,

    /// Add boilerplate templates from DIR (boilerplate/ and generated/)
    #[arg(long, value_name = "DIR", action = ArgAction::Append, help_heading = "Content")]
    pub(crate) templates: Vec<PathBuf>,

    /// Use only the templates from --templates
    #[arg(long = "templates-only", help_heading = "Content")]
    pub(crate) templates_only: bool,

    /// Write the built-in templates to DIR and exit
    #[arg(long = "dump-templates", value_name = "DIR", help_heading = "Content")]
    pub(crate) dump_templates: Option<PathBuf>,

    /// Show hidden entries (dotfiles, .git/), folded into the chaff line
    #[arg(short = 'A', long = "almost-all", help_heading = "Filtering")]
    pub(crate) almost_all: bool,

    /// Show everything: no chaff folding, no hidden entries, no gitignore
    #[arg(short = 'a', long = "all", help_heading = "Filtering")]
    pub(crate) all: bool,

    /// Do not use .gitignore rules
    #[arg(long = "no-gitignore", help_heading = "Filtering")]
    pub(crate) no_gitignore: bool,

    /// Set the action for all chaff classes
    #[arg(long, value_enum, value_name = "LEVEL", help_heading = "Filtering")]
    pub(crate) chaff: Option<ActionArg>,

    /// Show these chaff classes as normal rows
    #[arg(long, value_enum, value_name = "CLASS,…", value_delimiter = ',', action = ArgAction::Append, help_heading = "Filtering")]
    pub(crate) show: Vec<ClassArg>,

    /// Fold these chaff classes into the chaff line
    #[arg(long, value_enum, value_name = "CLASS,…", value_delimiter = ',', action = ArgAction::Append, help_heading = "Filtering")]
    pub(crate) fold: Vec<ClassArg>,

    /// Count these chaff classes only in the footer
    #[arg(long, value_enum, value_name = "CLASS,…", value_delimiter = ',', action = ArgAction::Append, help_heading = "Filtering")]
    pub(crate) hide: Vec<ClassArg>,

    /// Do not cross into other file systems
    #[arg(short = 'x', long = "one-file-system", help_heading = "Filtering")]
    pub(crate) one_file_system: bool,

    /// Follow symlinks
    #[arg(long, help_heading = "Filtering")]
    pub(crate) follow: bool,

    /// Output format (default: tty on a terminal, else plain)
    #[arg(long, value_enum, value_name = "FMT", help_heading = "Output")]
    pub(crate) format: Option<FormatArg>,

    /// JSON with every entry (implies --format json)
    #[arg(long = "json-full", help_heading = "Output")]
    pub(crate) json_full: bool,

    /// Colors [default: auto]
    #[arg(long, value_enum, value_name = "WHEN", help_heading = "Output")]
    pub(crate) color: Option<ColorWhen>,

    /// ASCII glyphs only
    #[arg(long, help_heading = "Output")]
    pub(crate) ascii: bool,

    /// Output width (default: the terminal width, or 120)
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u64).range(8..), help_heading = "Output")]
    pub(crate) width: Option<u64>,

    /// Age format [default: relative]
    #[arg(long, value_enum, value_name = "MODE", help_heading = "Output")]
    pub(crate) time: Option<TimeArg>,

    /// Sizes in whole bytes
    #[arg(long, help_heading = "Output")]
    pub(crate) bytes: bool,

    /// Sizes in SI units (base 1000)
    #[arg(long, help_heading = "Output")]
    pub(crate) si: bool,

    /// No git status
    #[arg(long = "no-git", help_heading = "Output")]
    pub(crate) no_git: bool,

    /// Explain the decision for PATH
    #[arg(long, value_name = "PATH", help_heading = "Help")]
    pub(crate) why: Option<PathBuf>,

    /// Print only the header, the type table, and the footer
    #[arg(long, help_heading = "Help")]
    pub(crate) stats: bool,

    /// Walk threads
    #[arg(short = 'j', long = "threads", value_name = "N",
          value_parser = clap::value_parser!(u64).range(1..), help_heading = "Help")]
    pub(crate) threads: Option<u64>,

    /// Print the version
    #[arg(long, help_heading = "Help")]
    pub(crate) version: bool,

    /// `--show`, `--fold`, and `--hide` in command-line order (FR-36).
    #[arg(skip)]
    pub(crate) overrides: Vec<(ChaffClass, Action)>,
}

/// The resolved options.
#[derive(Clone, Debug)]
pub struct Opts {
    pub paths: Vec<PathBuf>,
    pub budget: Budget,
    pub depth: Option<usize>,
    pub sort: SortKey,
    pub series_min: usize,
    /// `--focus`, relative to the current directory.
    pub focus: Option<PathBuf>,
    /// `--why`, relative to the current directory.
    pub why: Option<PathBuf>,
    /// `--stats`: header, type table, footer.
    pub stats: bool,
    /// `-l`: mode, owner, group, and the oldest age.
    pub long: bool,
    /// `--dupes`: fold copies of visible files.
    pub dupes: bool,
    pub detect: DetectMode,
    pub preview: PreviewMode,
    /// `--mismatches`: the list view.
    pub mismatches: bool,
    /// `--no-cache`: no type-cache I/O.
    pub no_cache: bool,
    pub templates: Vec<PathBuf>,
    pub templates_only: bool,
    pub dump_templates: Option<PathBuf>,
    pub chaff: ChaffOpts,
    pub one_fs: bool,
    pub follow: bool,
    pub format: Option<Format>,
    /// `--json-full`: JSON with an `entries` list of every node.
    pub json_full: bool,
    pub color: ColorWhen,
    pub ascii: bool,
    pub width: Option<usize>,
    pub time: TimeMode,
    pub units: SizeUnits,
    /// `--no-git`: no git status, column, or header part.
    pub no_git: bool,
    pub threads: Option<usize>,
    /// Salience weights from the config (FR-67).
    pub weights: crate::salience::Weights,
}

/// A bad option value that clap cannot check (exit code 2).
#[derive(Clone, Debug)]
pub struct UsageError(pub String);

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub enum Parsed {
    Version,
    Run(Box<Opts>),
}

/// The arguments before the config files: `--version`, or a run.
pub enum Args {
    Version,
    Run(Box<CliArgs>),
}

/// The clap command: the source of the help, the completions, and the man
/// page (FR-69).
pub fn command() -> clap::Command {
    CliArgs::command()
}

/// Parses the arguments (the first one is the program name), without the
/// config files.
pub fn parse_args<I, T>(args: I) -> Result<Args, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = CliArgs::command().try_get_matches_from(args)?;
    let mut cli = CliArgs::from_arg_matches(&matches)?;
    if cli.version {
        return Ok(Args::Version);
    }
    // --show, --fold, and --hide apply in command-line order (FR-36).
    let mut overrides: Vec<(usize, ChaffClass, Action)> = Vec::new();
    for (id, action, values) in [
        ("show", Action::Show, &cli.show),
        ("fold", Action::Fold, &cli.fold),
        ("hide", Action::Hide, &cli.hide),
    ] {
        if let Some(indices) = matches.indices_of(id) {
            for (index, class) in indices.zip(values) {
                overrides.push((index, class_of(*class), action));
            }
        }
    }
    overrides.sort_by_key(|(i, _, _)| *i);
    cli.overrides = overrides.into_iter().map(|(_, c, a)| (c, a)).collect();
    Ok(Args::Run(Box::new(cli)))
}

/// Parses the arguments with no config files: flags over the defaults.
pub fn parse_from<I, T>(args: I) -> Result<Parsed, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    Ok(match parse_args(args)? {
        Args::Version => Parsed::Version,
        Args::Run(cli) => Parsed::Run(Box::new(crate::config::ConfigLayers::default().merge(&cli))),
    })
}

pub(crate) fn sort_of(s: SortArg) -> SortKey {
    match s {
        SortArg::Salience => SortKey::Salience,
        SortArg::Name => SortKey::Name,
        SortArg::Size => SortKey::Size,
        SortArg::Age => SortKey::Age,
        SortArg::Type => SortKey::Type,
    }
}

pub(crate) fn detect_of(d: DetectArg) -> DetectMode {
    match d {
        DetectArg::Off => DetectMode::Off,
        DetectArg::Fast => DetectMode::Fast,
        // Without the `magika` feature, `full` acts as `fast` (FR-55).
        DetectArg::Full => DetectMode::Full,
    }
}

pub(crate) fn preview_of(p: PreviewArg) -> PreviewMode {
    match p {
        PreviewArg::Off => PreviewMode::Off,
        PreviewArg::Short => PreviewMode::Short,
        PreviewArg::Full => PreviewMode::Full,
    }
}

pub(crate) fn format_of(f: FormatArg) -> Format {
    match f {
        FormatArg::Tty => Format::Tty,
        FormatArg::Plain => Format::Plain,
        FormatArg::Md => Format::Md,
        FormatArg::Json => Format::Json,
    }
}

pub(crate) fn time_of(t: TimeArg) -> TimeMode {
    match t {
        TimeArg::Relative => TimeMode::Relative,
        TimeArg::Iso => TimeMode::Iso,
    }
}

pub(crate) fn action_of(a: ActionArg) -> Action {
    match a {
        ActionArg::Show => Action::Show,
        ActionArg::Dim => Action::Dim,
        ActionArg::Fold => Action::Fold,
        ActionArg::Hide => Action::Hide,
    }
}

pub(crate) fn class_of(c: ClassArg) -> ChaffClass {
    match c {
        ClassArg::Vendor => ChaffClass::Vendor,
        ClassArg::Build => ChaffClass::Build,
        ClassArg::Cache => ChaffClass::Cache,
        ClassArg::Vcs => ChaffClass::Vcs,
        ClassArg::Editor => ChaffClass::Editor,
        ClassArg::Lock => ChaffClass::Lock,
        ClassArg::Generated => ChaffClass::Generated,
        ClassArg::Minified => ChaffClass::Minified,
        ClassArg::Boilerplate => ChaffClass::Boilerplate,
        ClassArg::Empty => ChaffClass::Empty,
        ClassArg::Duplicate => ChaffClass::Duplicate,
        ClassArg::Ignored => ChaffClass::Ignored,
    }
}
