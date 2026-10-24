//! Config files (FR-66, FR-67): `~/.config/teak/config.toml` (or
//! `$XDG_CONFIG_HOME/teak/config.toml`) for the user, and the nearest
//! `.teak.toml` in the root or a parent for the project.
//!
//! Order: defaults ← user file ← project file ← flags. A TOML syntax error
//! or a bad value is a usage error (exit code 2) that names the file; an
//! unknown key is one warning, and the rest of the file still counts.

use std::path::{Path, PathBuf};

use clap::ValueEnum;
use toml::{Table, Value};

use crate::chaff::rules::{UserRule, UserRules};
use crate::chaff::{Action, ChaffClass, ChaffOpts};
use crate::cli::{self, Budget, CliArgs, ColorWhen, Opts};
use crate::detect::DetectMode;
use crate::layout::{PreviewMode, SizeUnits, TimeMode};
use crate::render::Format;
use crate::salience::Weights;
use crate::select::SortKey;

/// The name of the project file.
pub const PROJECT_FILE: &str = ".teak.toml";

/// The largest config file that teak reads.
const FILE_MAX: usize = 1 << 20;

/// A bad config file (exit code 2).
#[derive(Clone, Debug)]
pub struct ConfigError {
    pub path: PathBuf,
    /// The line of a TOML syntax error.
    pub line: Option<usize>,
    pub message: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: ", self.path.display())?;
        if let Some(line) = self.line {
            write!(f, "line {line}: ")?;
        }
        f.write_str(&self.message)
    }
}

/// The settings of one config file. `None` and empty lists set nothing.
#[derive(Clone, Debug, Default)]
pub struct Layer {
    pub path: PathBuf,
    pub budget: Option<Budget>,
    pub depth: Option<usize>,
    pub sort: Option<SortKey>,
    pub series_min: Option<usize>,
    pub detect: Option<DetectMode>,
    pub preview: Option<PreviewMode>,
    pub color: Option<ColorWhen>,
    pub width: Option<usize>,
    pub time: Option<TimeMode>,
    pub format: Option<Format>,
    /// Template directories, relative to the file's directory.
    pub templates: Vec<PathBuf>,
    /// `[chaff] show/fold/hide`, in that order.
    pub overrides: Vec<(ChaffClass, Action)>,
    pub rules: Vec<UserRule>,
    /// `[weights]`: size, recency, git, rarity, role, anomaly.
    pub weights: [Option<f32>; 6],
}

/// The user file and the project file, and the warnings from both.
#[derive(Clone, Debug, Default)]
pub struct ConfigLayers {
    pub user: Option<Layer>,
    pub project: Option<Layer>,
    /// One line each, without the `teak: ` prefix.
    pub warnings: Vec<String>,
}

/// `$XDG_CONFIG_HOME/teak/config.toml`, or `$HOME/.config/teak/config.toml`.
pub fn user_file() -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
    let base = match var("XDG_CONFIG_HOME") {
        Some(x) => PathBuf::from(x),
        None => PathBuf::from(var("HOME")?).join(".config"),
    };
    Some(base.join("teak").join("config.toml"))
}

/// The nearest `.teak.toml` in `root` (a directory) or its parents.
pub fn project_file(root: &Path) -> Option<PathBuf> {
    let abs = std::path::absolute(root).ok()?;
    let start = if abs.is_dir() {
        abs.as_path()
    } else {
        abs.parent()?
    };
    start
        .ancestors()
        .map(|d| d.join(PROJECT_FILE))
        .find(|p| p.is_file())
}

/// Loads the user file and the project file for `root`.
pub fn load(root: &Path) -> Result<ConfigLayers, ConfigError> {
    load_files(user_file().as_deref(), project_file(root).as_deref())
}

/// Loads the given files. A file that does not exist sets nothing.
pub fn load_files(
    user: Option<&Path>,
    project: Option<&Path>,
) -> Result<ConfigLayers, ConfigError> {
    let mut layers = ConfigLayers::default();
    for (path, slot) in [(user, &mut layers.user), (project, &mut layers.project)] {
        let Some(path) = path.filter(|p| p.is_file()) else {
            continue;
        };
        let bytes = crate::io::read_rule_file(path, FILE_MAX).map_err(|e| ConfigError {
            path: path.to_path_buf(),
            line: None,
            message: e.to_string(),
        })?;
        let text = String::from_utf8(bytes).map_err(|_| ConfigError {
            path: path.to_path_buf(),
            line: None,
            message: "the file is not UTF-8".to_string(),
        })?;
        *slot = Some(parse(path, &text, &mut layers.warnings)?);
    }
    Ok(layers)
}

/// Parses one config file. Unknown keys go to `warnings`.
pub fn parse(path: &Path, text: &str, warnings: &mut Vec<String>) -> Result<Layer, ConfigError> {
    let err = |message: String| ConfigError {
        path: path.to_path_buf(),
        line: None,
        message,
    };
    let table: Table = text.parse().map_err(|e: toml::de::Error| ConfigError {
        path: path.to_path_buf(),
        line: e
            .span()
            .map(|s| text[..s.start.min(text.len())].matches('\n').count() + 1),
        message: e.message().trim().to_string(),
    })?;
    let mut unknown = |key: String| {
        warnings.push(format!("{}: unknown key `{key}`", path.display()));
    };
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut layer = Layer {
        path: path.to_path_buf(),
        ..Layer::default()
    };
    for (key, value) in &table {
        match key.as_str() {
            "lines" => {
                let s = match value {
                    Value::Integer(n) => n.to_string(),
                    Value::String(s) => s.clone(),
                    _ => return Err(err(format!("`lines`: {}", type_text(value)))),
                };
                layer.budget =
                    Some(cli::parse_budget(&s).map_err(|e| err(format!("`lines`: {e}")))?);
            }
            "depth" => layer.depth = Some(int(value, key, 0).map_err(err)?),
            "series_min" => layer.series_min = Some(int(value, key, 2).map_err(err)?),
            "width" => layer.width = Some(int(value, key, 8).map_err(err)?),
            "sort" => layer.sort = Some(cli::sort_of(choice(value, key).map_err(err)?)),
            "detect" => layer.detect = Some(cli::detect_of(choice(value, key).map_err(err)?)),
            "preview" => layer.preview = Some(cli::preview_of(choice(value, key).map_err(err)?)),
            "color" => layer.color = Some(choice(value, key).map_err(err)?),
            "time" => layer.time = Some(cli::time_of(choice(value, key).map_err(err)?)),
            "format" => layer.format = Some(cli::format_of(choice(value, key).map_err(err)?)),
            "templates" => {
                for t in strings(value, key).map_err(err)? {
                    layer.templates.push(dir.join(t));
                }
            }
            "chaff" => {
                let Value::Table(chaff) = value else {
                    return Err(err(format!("`chaff`: {}", type_text(value))));
                };
                for (k, v) in chaff {
                    let name = format!("chaff.{k}");
                    let action = match k.as_str() {
                        "show" => Action::Show,
                        "fold" => Action::Fold,
                        "hide" => Action::Hide,
                        "rules" => {
                            layer.rules = rules(v).map_err(err)?;
                            continue;
                        }
                        _ => {
                            unknown(name);
                            continue;
                        }
                    };
                    for c in strings(v, &name).map_err(err)? {
                        layer
                            .overrides
                            .push((class(&c, &name).map_err(err)?, action));
                    }
                }
                // show, fold, hide: the order of the flags in the help.
                layer.overrides.sort_by_key(|(_, a)| match a {
                    Action::Show | Action::Dim => 0,
                    Action::Fold => 1,
                    Action::Hide => 2,
                });
            }
            "weights" => {
                let Value::Table(w) = value else {
                    return Err(err(format!("`weights`: {}", type_text(value))));
                };
                for (k, v) in w {
                    const NAMES: [&str; 6] =
                        ["size", "recency", "git", "rarity", "role", "anomaly"];
                    let Some(i) = NAMES.iter().position(|n| n == k) else {
                        unknown(format!("weights.{k}"));
                        continue;
                    };
                    let x = match v {
                        Value::Integer(n) => *n as f64,
                        Value::Float(f) => *f,
                        _ => return Err(err(format!("`weights.{k}`: {}", type_text(v)))),
                    };
                    if !x.is_finite() || x < 0.0 {
                        return Err(err(format!(
                            "`weights.{k}`: expected a number of 0 or more, got {x}"
                        )));
                    }
                    layer.weights[i] = Some(x as f32);
                }
            }
            _ => unknown(key.clone()),
        }
    }
    Ok(layer)
}

fn type_text(v: &Value) -> String {
    let shown = match v {
        Value::String(s) => format!(" \"{s}\""),
        Value::Integer(n) => format!(" {n}"),
        Value::Float(f) => format!(" {f}"),
        Value::Boolean(b) => format!(" {b}"),
        _ => String::new(),
    };
    format!("unexpected {}{shown}", v.type_str())
}

fn int(v: &Value, key: &str, min: i64) -> Result<usize, String> {
    match v {
        Value::Integer(n) if *n >= min => Ok(*n as usize),
        Value::Integer(n) => Err(format!("`{key}`: {n} is below {min}")),
        _ => Err(format!("`{key}`: {}", type_text(v))),
    }
}

/// A value of a clap value enum, checked as the flag checks it.
fn choice<T: ValueEnum>(v: &Value, key: &str) -> Result<T, String> {
    let Value::String(s) = v else {
        return Err(format!("`{key}`: {}", type_text(v)));
    };
    T::from_str(s, false).map_err(|_| {
        let valid: Vec<String> = T::value_variants()
            .iter()
            .filter_map(|x| x.to_possible_value().map(|p| p.get_name().to_string()))
            .collect();
        format!(
            "`{key}`: invalid value `{s}`; valid values: {}",
            valid.join(", ")
        )
    })
}

fn strings(v: &Value, key: &str) -> Result<Vec<String>, String> {
    let Value::Array(items) = v else {
        return Err(format!("`{key}`: {}, expected a list", type_text(v)));
    };
    items
        .iter()
        .map(|i| match i {
            Value::String(s) => Ok(s.clone()),
            _ => Err(format!("`{key}`: {}", type_text(i))),
        })
        .collect()
}

fn class(name: &str, key: &str) -> Result<ChaffClass, String> {
    choice::<cli::ClassArg>(&Value::String(name.to_string()), key).map(cli::class_of)
}

fn rules(v: &Value) -> Result<Vec<UserRule>, String> {
    let Value::Array(items) = v else {
        return Err(format!("`chaff.rules`: {}", type_text(v)));
    };
    let mut out = Vec::new();
    for item in items {
        let Value::Table(t) = item else {
            return Err(format!("`chaff.rules`: {}", type_text(item)));
        };
        let get = |k: &str| match t.get(k) {
            Some(Value::String(s)) => Ok(s.clone()),
            Some(other) => Err(format!("`chaff.rules.{k}`: {}", type_text(other))),
            None => Err(format!("`chaff.rules`: a rule needs `{k}`")),
        };
        if let Some(k) = t.keys().find(|k| *k != "pattern" && *k != "class") {
            return Err(format!("`chaff.rules`: unknown key `{k}`"));
        }
        out.push(UserRule {
            pattern: get("pattern")?,
            class: class(&get("class")?, "chaff.rules.class")?,
        });
    }
    UserRules::new(&out).map_err(|e| format!("`chaff.rules`: {e}"))?;
    Ok(out)
}

impl ConfigLayers {
    /// The options: defaults ← user file ← project file ← flags. A flag the
    /// user did not type does not override a config value.
    pub fn merge(self, cli: &CliArgs) -> Opts {
        let layers: Vec<&Layer> = self.user.iter().chain(self.project.iter()).collect();
        // The value of the last layer that sets it.
        macro_rules! pick {
            ($field:ident) => {
                layers.iter().rev().find_map(|l| l.$field)
            };
        }
        let mut weights = Weights::default();
        for l in &layers {
            for (slot, w) in [
                &mut weights.size,
                &mut weights.recency,
                &mut weights.git,
                &mut weights.rarity,
                &mut weights.role,
                &mut weights.anomaly,
            ]
            .into_iter()
            .zip(l.weights)
            {
                if let Some(w) = w {
                    *slot = w;
                }
            }
        }
        let mut templates: Vec<PathBuf> = layers.iter().flat_map(|l| l.templates.clone()).collect();
        templates.extend(cli.templates.iter().cloned());
        let mut overrides: Vec<(ChaffClass, Action)> =
            layers.iter().flat_map(|l| l.overrides.clone()).collect();
        overrides.extend(cli.overrides.iter().copied());
        // The project rules come first: the project file is the nearer one.
        let rules: Vec<UserRule> = layers.iter().rev().flat_map(|l| l.rules.clone()).collect();
        Opts {
            paths: if cli.paths.is_empty() {
                vec![PathBuf::from(".")]
            } else {
                cli.paths.clone()
            },
            budget: cli
                .lines
                .or_else(|| pick!(budget))
                .unwrap_or(Budget::Lines(100)),
            depth: cli.depth.or_else(|| pick!(depth)),
            sort: cli
                .sort
                .map(cli::sort_of)
                .or_else(|| pick!(sort))
                .unwrap_or(SortKey::Default),
            series_min: cli
                .series_min
                .map(|n| n as usize)
                .or_else(|| pick!(series_min))
                .unwrap_or(4),
            focus: cli.focus.clone(),
            why: cli.why.clone(),
            stats: cli.stats,
            long: cli.long,
            dupes: cli.dupes,
            detect: cli
                .detect
                .map(cli::detect_of)
                .or_else(|| pick!(detect))
                .unwrap_or(DetectMode::Full),
            preview: cli
                .preview
                .map(cli::preview_of)
                .or_else(|| pick!(preview))
                .unwrap_or(PreviewMode::Short),
            mismatches: cli.mismatches,
            no_cache: cli.no_cache,
            templates,
            templates_only: cli.templates_only,
            dump_templates: cli.dump_templates.clone(),
            chaff: ChaffOpts {
                almost_all: cli.almost_all,
                all: cli.all,
                no_gitignore: cli.no_gitignore,
                level: cli.chaff.map(cli::action_of),
                overrides,
                rules,
            },
            one_fs: cli.one_file_system,
            follow: cli.follow,
            format: if cli.json_full {
                Some(Format::Json)
            } else {
                cli.format.map(cli::format_of).or_else(|| pick!(format))
            },
            json_full: cli.json_full,
            color: cli
                .color
                .or_else(|| pick!(color))
                .unwrap_or(ColorWhen::Auto),
            ascii: cli.ascii,
            width: cli.width.map(|w| w as usize).or_else(|| pick!(width)),
            time: cli
                .time
                .map(cli::time_of)
                .or_else(|| pick!(time))
                .unwrap_or(TimeMode::Relative),
            units: if cli.bytes {
                SizeUnits::Bytes
            } else if cli.si {
                SizeUnits::Si
            } else {
                SizeUnits::Iec
            },
            no_git: cli.no_git,
            threads: cli.threads.map(|t| t as usize),
            weights,
        }
    }
}

/// Loads the config files for the first root and merges them with the
/// flags. Returns the options and the warnings.
pub fn resolve(cli: &CliArgs) -> Result<(Opts, Vec<String>), ConfigError> {
    let root = cli.paths.first().map_or(Path::new("."), PathBuf::as_path);
    let mut layers = load(root)?;
    let warnings = std::mem::take(&mut layers.warnings);
    Ok((layers.merge(cli), warnings))
}
