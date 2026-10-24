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
enum SortArg {
    Salience,
    Name,
    Size,
    Age,
    Type,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum ActionArg {
    Show,
    Dim,
    Fold,
    Hide,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum ClassArg {
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
enum FormatArg {
    Tty,
    Plain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum TimeArg {
    Relative,
    Iso,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum DetectArg {
    Off,
    Fast,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum PreviewArg {
    Off,
    Short,
    Full,
}

fn parse_budget(s: &str) -> Result<Budget, String> {
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
    max_term_width = 100
)]
struct Cli {
    /// Directories or files to show (default: the current directory)
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,

    /// Line budget: a number (3 or more), or auto (terminal height - 2)
    #[arg(short = 'n', long = "lines", value_name = "N|auto", default_value = "100", value_parser = parse_budget, help_heading = "Layout")]
    lines: Budget,

    /// Never open deeper than N
    #[arg(short = 'L', long = "depth", value_name = "N", help_heading = "Layout")]
    depth: Option<usize>,

    /// Display order inside a directory
    #[arg(long, value_enum, value_name = "KEY", help_heading = "Layout")]
    sort: Option<SortArg>,

    /// Smallest group that becomes a series row
    #[arg(long = "series-min", value_name = "N", default_value_t = 4,
          value_parser = clap::value_parser!(u64).range(2..), help_heading = "Layout")]
    series_min: u64,

    /// Content detection
    #[arg(
        long,
        value_enum,
        value_name = "MODE",
        default_value = "full",
        help_heading = "Content"
    )]
    detect: DetectArg,

    /// Previews
    #[arg(
        long,
        value_enum,
        value_name = "MODE",
        default_value = "short",
        help_heading = "Content"
    )]
    preview: PreviewArg,

    /// Show hidden entries (dotfiles, .git/), folded into the chaff line
    #[arg(short = 'A', long = "almost-all", help_heading = "Filtering")]
    almost_all: bool,

    /// Show everything: no chaff folding, no hidden entries, no gitignore
    #[arg(short = 'a', long = "all", help_heading = "Filtering")]
    all: bool,

    /// Do not use .gitignore rules
    #[arg(long = "no-gitignore", help_heading = "Filtering")]
    no_gitignore: bool,

    /// Set the action for all chaff classes
    #[arg(long, value_enum, value_name = "LEVEL", help_heading = "Filtering")]
    chaff: Option<ActionArg>,

    /// Show these chaff classes as normal rows
    #[arg(long, value_enum, value_name = "CLASS,…", value_delimiter = ',', action = ArgAction::Append, help_heading = "Filtering")]
    show: Vec<ClassArg>,

    /// Fold these chaff classes into the chaff line
    #[arg(long, value_enum, value_name = "CLASS,…", value_delimiter = ',', action = ArgAction::Append, help_heading = "Filtering")]
    fold: Vec<ClassArg>,

    /// Count these chaff classes only in the footer
    #[arg(long, value_enum, value_name = "CLASS,…", value_delimiter = ',', action = ArgAction::Append, help_heading = "Filtering")]
    hide: Vec<ClassArg>,

    /// Do not cross into other file systems
    #[arg(short = 'x', long = "one-file-system", help_heading = "Filtering")]
    one_file_system: bool,

    /// Follow symlinks
    #[arg(long, help_heading = "Filtering")]
    follow: bool,

    /// Output format (default: tty on a terminal, else plain)
    #[arg(long, value_enum, value_name = "FMT", help_heading = "Output")]
    format: Option<FormatArg>,

    /// Colors
    #[arg(
        long,
        value_enum,
        value_name = "WHEN",
        default_value = "auto",
        help_heading = "Output"
    )]
    color: ColorWhen,

    /// ASCII glyphs only
    #[arg(long, help_heading = "Output")]
    ascii: bool,

    /// Output width (default: the terminal width, or 120)
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u64).range(8..), help_heading = "Output")]
    width: Option<u64>,

    /// Age format
    #[arg(
        long,
        value_enum,
        value_name = "MODE",
        default_value = "relative",
        help_heading = "Output"
    )]
    time: TimeArg,

    /// Sizes in whole bytes
    #[arg(long, help_heading = "Output")]
    bytes: bool,

    /// Sizes in SI units (base 1000)
    #[arg(long, help_heading = "Output")]
    si: bool,

    /// Walk threads
    #[arg(short = 'j', long = "threads", value_name = "N",
          value_parser = clap::value_parser!(u64).range(1..), help_heading = "Help")]
    threads: Option<u64>,

    /// Print the version
    #[arg(long, help_heading = "Help")]
    version: bool,
}

/// The resolved options.
#[derive(Clone, Debug)]
pub struct Opts {
    pub paths: Vec<PathBuf>,
    pub budget: Budget,
    pub depth: Option<usize>,
    pub sort: SortKey,
    pub series_min: usize,
    pub detect: DetectMode,
    pub preview: PreviewMode,
    pub chaff: ChaffOpts,
    pub one_fs: bool,
    pub follow: bool,
    pub format: Option<Format>,
    pub color: ColorWhen,
    pub ascii: bool,
    pub width: Option<usize>,
    pub time: TimeMode,
    pub units: SizeUnits,
    pub threads: Option<usize>,
}

pub enum Parsed {
    Version,
    Run(Opts),
}

/// Parses the arguments (the first one is the program name).
pub fn parse_from<I, T>(args: I) -> Result<Parsed, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = Cli::command().try_get_matches_from(args)?;
    let cli = Cli::from_arg_matches(&matches)?;
    if cli.version {
        return Ok(Parsed::Version);
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
    let opts = Opts {
        paths: if cli.paths.is_empty() {
            vec![PathBuf::from(".")]
        } else {
            cli.paths
        },
        budget: cli.lines,
        depth: cli.depth,
        sort: match cli.sort {
            None => SortKey::Default,
            Some(SortArg::Salience) => SortKey::Salience,
            Some(SortArg::Name) => SortKey::Name,
            Some(SortArg::Size) => SortKey::Size,
            Some(SortArg::Age) => SortKey::Age,
            Some(SortArg::Type) => SortKey::Type,
        },
        series_min: cli.series_min as usize,
        detect: match cli.detect {
            DetectArg::Off => DetectMode::Off,
            DetectArg::Fast => DetectMode::Fast,
            // Task-3 adds Magika; until then `full` acts as `fast`.
            DetectArg::Full => DetectMode::Full,
        },
        preview: match cli.preview {
            PreviewArg::Off => PreviewMode::Off,
            PreviewArg::Short => PreviewMode::Short,
            PreviewArg::Full => PreviewMode::Full,
        },
        chaff: ChaffOpts {
            almost_all: cli.almost_all,
            all: cli.all,
            no_gitignore: cli.no_gitignore,
            level: cli.chaff.map(action_of),
            overrides: overrides.into_iter().map(|(_, c, a)| (c, a)).collect(),
        },
        one_fs: cli.one_file_system,
        follow: cli.follow,
        format: cli.format.map(|f| match f {
            FormatArg::Tty => Format::Tty,
            FormatArg::Plain => Format::Plain,
        }),
        color: cli.color,
        ascii: cli.ascii,
        width: cli.width.map(|w| w as usize),
        time: match cli.time {
            TimeArg::Relative => TimeMode::Relative,
            TimeArg::Iso => TimeMode::Iso,
        },
        units: if cli.bytes {
            SizeUnits::Bytes
        } else if cli.si {
            SizeUnits::Si
        } else {
            SizeUnits::Iec
        },
        threads: cli.threads.map(|t| t as usize),
    };
    Ok(Parsed::Run(opts))
}

fn action_of(a: ActionArg) -> Action {
    match a {
        ActionArg::Show => Action::Show,
        ActionArg::Dim => Action::Dim,
        ActionArg::Fold => Action::Fold,
        ActionArg::Hide => Action::Hide,
    }
}

fn class_of(c: ClassArg) -> ChaffClass {
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
